use crate::helpers::ip_string_to_u32;
use crate::helpers::{self, get_current_clock_ns};
use crate::values;
use aya::{maps::Array, maps::HashMap, maps::MapData};
use local_ip_address::local_ip;
use log::info;
use raft_main_common::{
    CurrentNode, LeaderNode, NodeState, Vote, HEARTBEAT_REQUEST_PORT, VOTE_REQUEST_PORT,
};
use rayon::prelude::*;
use std::env;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex, RwLock};

pub struct AppState {
    pub followers: Arc<Mutex<HashMap<MapData, u32, u64>>>,
    pub heartbeat_latency: Arc<Mutex<HashMap<MapData, u32, u64>>>,
    pub voting_results: Arc<RwLock<HashMap<MapData, u32, u64>>>,
    pub leader_node: Arc<RwLock<Array<MapData, LeaderNode>>>,
    pub current_node: Arc<RwLock<Array<MapData, CurrentNode>>>,
    pub udp_socket: Arc<Mutex<UdpSocket>>,
}

// Clone here makes a copy of the Arc pointer.
// All clones point to the same internal data.
impl Clone for AppState {
    fn clone(&self) -> Self {
        AppState {
            followers: Arc::clone(&self.followers),
            heartbeat_latency: Arc::clone(&self.heartbeat_latency),
            voting_results: Arc::clone(&self.voting_results),
            current_node: Arc::clone(&self.current_node),
            leader_node: Arc::clone(&self.leader_node),
            udp_socket: Arc::clone(&self.udp_socket),
        }
    }
}

impl AppState {
    // TODO: This function is very ugly.
    pub fn initialise_node(&self) {
        let mut peer_ip_addresses: Vec<u32> = Vec::new();
        let mut peer_ip_addresses_array: [u32; 2] = [0; 2]; // eBPF does not support vectors.

        match env::var("PEERS") {
            Ok(peers) => {
                let local_ip = local_ip().unwrap(); // Get local IP address
                let local_ip_u32 = ip_string_to_u32(&local_ip.to_string()).unwrap_or_default();

                let ip_list_from_env: Vec<&str> = peers.split(',').collect();

                info!(
                    "Found {} IPs in the PEERS environment variable.",
                    ip_list_from_env.len()
                );

                for ip in ip_list_from_env {
                    let ip_address: u32 = ip_string_to_u32(ip).unwrap_or_default();

                    // Skip adding current IP address to peer list.
                    // This allows passing the same IP address list to all Raft nodes.
                    if local_ip_u32 == ip_address {
                        continue;
                    }

                    peer_ip_addresses.push(ip_address);
                }
            }
            Err(e) => println!("Couldn't read PEERS from environment variable({})", e),
        }

        for (index, element) in peer_ip_addresses.iter().enumerate() {
            peer_ip_addresses_array[index] = *element;
        }

        info!("Added {} IPs to the hosts...", peer_ip_addresses.len());

        // Initialise node
        let mut current_node = self.current_node.write().unwrap();
        match current_node.set(
            0,
            CurrentNode {
                state: NodeState::Follower,
                peers: peer_ip_addresses_array,
                term: 0,
                vote: Vote {
                    in_progress: false,
                    started_ts: 0,
                    ended_ts: 0,
                    election_timeout: 0,
                },
            },
            0,
        ) {
            Ok(_) => {}
            Err(_err) => todo!(),
        };

        // Inser dummy leader data with current timestamp.
        let mut leader = self.leader_node.write().unwrap();
        match leader.set(
            0,
            LeaderNode {
                last_seen: get_current_clock_ns(),
                source_addr_raw: 0,
                term_id: 0,
            },
            0,
        ) {
            Ok(_) => {}
            Err(_err) => todo!(),
        };
    }

    // Get current node data.
    fn get_current_node(&self) -> CurrentNode {
        let current_node = self.current_node.read().unwrap();

        let node: CurrentNode = match current_node.get(&0, 0) {
            Ok(x) => x,
            Err(_err) => todo!(),
        };

        node
    }

    // Update current node values.
    fn update_current_node(&self, node: CurrentNode) {
        let mut node_data = self.current_node.write().unwrap();

        match node_data.set(0, node, 0) {
            Ok(x) => x,
            Err(_err) => todo!(),
        };
    }

    // Insert heartbeat timestamp into FOLLOWERS map when sending HEARTBEAT_REQUEST_PORT.
    fn insert_heartbeat_timestamp(&self, ip: u32, ts: u64) {
        let follower_data = self.followers.clone();
        let mut followers = follower_data.lock().unwrap();

        match followers.insert(ip, ts, 0) {
            Ok(()) => {}
            Err(_err) => todo!(),
        }
    }

    // Get current node state.
    pub fn get_current_state(&self) -> NodeState {
        let node = self.get_current_node();
        node.state
    }

    // Check when leader has last communicated.
    // Data is populated by eBPF program when receiving heartbeat requests on HEARTBEAT_REQUEST_PORT.
    pub fn leader_last_seen(&self) -> u64 {
        let leader = self.leader_node.read().unwrap();

        let current_leader: LeaderNode = match leader.get(&0, 0) {
            Ok(x) => x,
            Err(_err) => todo!(),
        };

        helpers::get_current_clock_ms() - (current_leader.last_seen / 1_000_000)
    }

    // When simulating crash, update leader last seen to current timestamp to avoid
    // becoming the first node detecting absence of leader and winning the election.
    pub fn update_leader_last_seen_time(&self) {
        let mut leader: std::sync::RwLockWriteGuard<'_, Array<MapData, LeaderNode>> =
            self.leader_node.write().unwrap();

        match leader.set(
            0,
            LeaderNode {
                last_seen: get_current_clock_ns(),
                term_id: 0,
                source_addr_raw: 0,
            },
            0,
        ) {
            Ok(x) => x,
            Err(_err) => todo!(),
        };
    }

    // Get curent node term.
    pub fn current_term_id(&self) -> u64 {
        self.get_current_node().term
    }

    // Get current term number, represented in bytes.
    fn current_term_id_bytes(&self) -> [u8; 8] {
        let term_id = self.current_term_id();

        // u64 needs 8 bytes
        let mut buffer = [0; 8]; // initialise to 0
        buffer.copy_from_slice(&term_id.to_be_bytes());

        buffer
    }

    // Increment term number.
    pub fn increment_term_number(&self) {
        let mut node = self.get_current_node();
        node.term += 1;
        self.update_current_node(node);
    }

    // Transition to follower state.
    pub fn become_follower(&self) {
        let mut node = self.get_current_node();
        node.state = NodeState::Follower;
        self.update_current_node(node);
    }

    // Transition to candidate state.
    pub fn become_candidate(&self) {
        let mut node = self.get_current_node();
        node.state = NodeState::Candidate;
        node.vote.in_progress = false;
        self.update_current_node(node);
    }

    // Transition to leader state.
    pub fn become_leader(&self) {
        let mut node = self.get_current_node();
        node.state = NodeState::Leader;
        self.update_current_node(node);
    }

    // Start vote (update metadata).
    pub fn start_vote(&self) {
        let mut node = self.get_current_node();
        node.vote.in_progress = true;
        node.vote.started_ts = get_current_clock_ns();

        node.vote.election_timeout =
            values::ELECTION_TIMEOUT_NS + helpers::get_election_timeout_jitter_ns();

        self.update_current_node(node);
    }

    // Get current election timeout.
    pub fn get_election_timeout(&self) -> u64 {
        let node = self.get_current_node();
        node.vote.election_timeout
    }

    // Check if election has timed out.
    pub fn election_timed_out(&self) -> bool {
        let node = self.get_current_node();
        let time_elapsed = get_current_clock_ns() - node.vote.started_ts;
        if node.vote.in_progress && (time_elapsed > node.vote.election_timeout) {
            return true;
        }
        false
    }

    // Stops vote and records its ending time.
    pub fn stop_vote(&self) {
        let mut node = self.get_current_node();
        node.vote.in_progress = false;
        node.vote.ended_ts = get_current_clock_ns();
        self.update_current_node(node);
    }

    // Aborts vote and resets voting fields.
    pub fn abort_vote(&self) {
        let mut node = self.get_current_node();
        node.vote.in_progress = false;
        node.vote.started_ts = 0;
        self.update_current_node(node);
    }

    // Resets vote data after a successful election.
    pub fn reset_vote_data(&self) {
        let mut node = self.get_current_node();
        node.vote.in_progress = false;
        node.vote.ended_ts = 0;
        node.vote.started_ts = 0;
        node.vote.election_timeout = 0;
        self.update_current_node(node);
    }

    // Reset vote result map items after each election.
    pub fn reset_vote_results(&self) -> Result<(), aya::maps::MapError> {
        let mut vote_results = self.voting_results.write().unwrap();

        for ip in vote_results.keys() {
            let source_ip = ip.unwrap();

            match vote_results.insert(source_ip, 0, 0) {
                Ok(()) => return Ok(()),
                Err(err) => return Err(err),
            }
        }
        Ok(())
    }

    // Calculates vote duration.
    pub fn get_vote_duration(&self) -> u64 {
        let node = self.get_current_node();
        node.vote.ended_ts - node.vote.started_ts
    }

    // Returns vote state.
    pub fn vote_in_progress(&self) -> bool {
        self.get_current_node().vote.in_progress
    }

    // Get Raft peer IPs.
    pub fn get_raft_peers(&self) -> [u32; 2] {
        let node = self.get_current_node();
        node.peers
    }

    // Send heartbeat RPCs.
    pub fn send_heartbeat_rpcs(&self) {
        let buffer = self.current_term_id_bytes();
        let udp_socket_data = self.udp_socket.clone();
        let socket = udp_socket_data.lock().unwrap();

        self.get_raft_peers().par_iter().for_each(|&ip| {
            if ip == 0 {
                return;
            }
            let dest_socket = SocketAddr::new(Ipv4Addr::from(ip).into(), HEARTBEAT_REQUEST_PORT);

            self.insert_heartbeat_timestamp(ip, helpers::get_current_clock_ns());
            socket
                .send_to(&buffer, dest_socket)
                .expect("Failed to send packet");
        });
    }

    // Send vote request RPCs.
    pub fn send_request_vote_rpcs(&self) {
        let buffer = self.current_term_id_bytes();
        let udp_socket_data = self.udp_socket.clone();
        let socket = udp_socket_data.lock().unwrap();

        self.get_raft_peers().par_iter().for_each(|&ip| {
            if ip == 0 {
                return;
            }
            let dest_socket = SocketAddr::new(Ipv4Addr::from(ip).into(), VOTE_REQUEST_PORT);

            socket
                .send_to(&buffer, dest_socket)
                .expect("Failed to send packet");
        });
    }

    // Get current yes votes from peers.
    pub fn get_current_yes_votes_from_peers(&self) -> u64 {
        let vote_results = self.voting_results.read().unwrap();
        let mut total_positive_votes_for: u64 = 0;

        for vote in vote_results.keys() {
            let voter = vote.unwrap();

            let vote: u64 = match vote_results.get(&voter, 0) {
                Ok(x) => x,
                Err(_err) => todo!(),
            };

            total_positive_votes_for += vote;
        }

        total_positive_votes_for
    }

    // Quorum checks.
    pub fn quorum_reached(&self) -> bool {
        let mut positive_votes = self.get_current_yes_votes_from_peers();
        positive_votes += 1; // Candidate votes for itself.

        if positive_votes >= values::QUORUM {
            return true;
        }

        false
    }
}
