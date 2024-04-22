use anyhow::Context;
use axum::{
    routing::{get, post},
    Router,
};
use aya::maps::MapData;
use aya::programs::{Xdp, XdpFlags};
use aya::{include_bytes_aligned, maps::Array, maps::HashMap, Bpf};
use aya_log::BpfLogger;
use clap::Parser;
use log::{debug, info, warn};
use nix::sys::socket::{setsockopt, sockopt::SndBuf};
use raft_main_common::{CurrentNode, LeaderNode};
use std::net::{SocketAddr, UdpSocket};
use std::os::unix::io::AsRawFd;
use std::sync::{Arc, Mutex, RwLock};
use tokio::signal;

mod fsm_candidate;
mod fsm_follower;
mod fsm_leader;
mod fsm_single_thread;
mod helpers;
mod routes;
mod state;
mod values;

#[derive(Debug, Parser)]
struct Opt {
    #[clap(short, long, default_value = "eth0")]
    iface: String,
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let opt = Opt::parse();

    env_logger::init();

    // Bump the memlock rlimit. This is needed for older kernels that don't use the
    // new memcg based accounting, see https://lwn.net/Articles/837122/
    let rlim = libc::rlimit {
        rlim_cur: libc::RLIM_INFINITY,
        rlim_max: libc::RLIM_INFINITY,
    };
    let ret = unsafe { libc::setrlimit(libc::RLIMIT_MEMLOCK, &rlim) };
    if ret != 0 {
        debug!("remove limit on locked memory failed, ret is: {}", ret);
    }

    // This will include your eBPF object file as raw bytes at compile-time and load it at
    // runtime. This approach is recommended for most real-world use cases. If you would
    // like to specify the eBPF program at runtime rather than at compile-time, you can
    // reach for `Bpf::load_file` instead.
    #[cfg(debug_assertions)]
    let mut bpf = Bpf::load(include_bytes_aligned!(
        "../../target/bpfel-unknown-none/debug/raft-main"
    ))?;
    #[cfg(not(debug_assertions))]
    let mut bpf = Bpf::load(include_bytes_aligned!(
        "../../target/bpfel-unknown-none/release/raft-main"
    ))?;
    if let Err(e) = BpfLogger::init(&mut bpf) {
        // This can happen if you remove all log statements from your eBPF program.
        warn!("failed to initialize eBPF logger: {}", e);
    }
    let program: &mut Xdp = bpf.program_mut("raft_main").unwrap().try_into()?;
    program.load()?;
    program.attach(&opt.iface, XdpFlags::SKB_MODE) // TODO: This somehow affects the checksums????
        .context("failed to attach the XDP program with default flags - try changing XdpFlags::default() to XdpFlags::SKB_MODE")?;

    // Shared maps.
    let followers: HashMap<_, u32, u64> = HashMap::try_from(bpf.take_map("FOLLOWERS").unwrap())?;
    let heartbeat_latency: HashMap<_, u32, u64> =
        HashMap::try_from(bpf.take_map("HEARTBEAT_LATENCY").unwrap())?;
    let voting_results: HashMap<_, u32, u64> =
        HashMap::try_from(bpf.take_map("VOTE_RESULTS").unwrap())?;
    let current_node: Array<MapData, CurrentNode> =
        Array::try_from(bpf.take_map("CURRENT_NODE").unwrap())?;
    let leader_node: Array<MapData, LeaderNode> =
        Array::try_from(bpf.take_map("LEADER_NODE").unwrap())?;

    // Create a UDP socket to be shared across multiple threads.
    let udp_socket = UdpSocket::bind("0.0.0.0:0").expect("Failed to create socket");
    let fd = udp_socket.as_raw_fd();
    setsockopt(fd, SndBuf, &4096).expect("Failed to set send buffer size");

    // State object holds all relevant data for a single Raft node.
    let state = state::AppState {
        followers: Arc::new(Mutex::new(followers)),
        heartbeat_latency: Arc::new(Mutex::new(heartbeat_latency)),
        voting_results: Arc::new(RwLock::new(voting_results)),
        current_node: Arc::new(RwLock::new(current_node)),
        leader_node: Arc::new(RwLock::new(leader_node)),
        udp_socket: Arc::new(Mutex::new(udp_socket)),
    };

    // Initialise the (follower) node with term ID 0.
    state.initialise_node();

    // let leader_state = state.clone();
    // std::thread::spawn(move || fsm_leader::leader_loop(&leader_state));

    // let follower_state = state.clone();
    // std::thread::spawn(move || fsm_follower::follower_loop(&follower_state));

    // let candidate_state = state.clone();
    // std::thread::spawn(move || fsm_candidate::candidate_loop(&candidate_state));

    // Using single thread as opposed to multiple threads ^ for performance.
    let shared_state = state.clone();
    std::thread::spawn(move || fsm_single_thread::shared_loop(&shared_state));

    let app = Router::new()
        .route("/followers/list", get(routes::list_followers))
        .route("/followers/add", post(routes::add_follower))
        .route("/followers/delete", post(routes::delete_follower))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8888));
    info!("Listening on...{}", addr);
    let server = axum::Server::bind(&addr).serve(app.into_make_service());

    tokio::select! {
        _ = server => {
            info!("Server is running...");
        },
        _ = signal::ctrl_c() => {
            info!("Ctrl-C received. Exiting...");
        }
    }
    Ok(())
}
