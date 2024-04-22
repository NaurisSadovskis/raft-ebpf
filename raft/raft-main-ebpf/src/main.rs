#![no_std]
#![no_main]

use aya_bpf::{
    bindings::xdp_action,
    macros::xdp,
    programs::XdpContext,
    helpers::bpf_ktime_get_ns,
};
use aya_log_ebpf::{debug, warn, info};
use network_types::{
    eth::{EthHdr, EtherType},
    ip::{IpProto, Ipv4Hdr},
    tcp::TcpHdr,
    udp::UdpHdr,
};

use raft_main_common::{
    NodeState, 
    VOTE_REQUEST_PORT, 
    VOTE_RESPONSE_PORT_NO, 
    VOTE_RESPONSE_PORT_YES, 
    HEARTBEAT_REQUEST_PORT, 
    HEARTBEAT_RESPONSE_PORT
};

mod helpers_raft;
mod helpers_xdp;
mod maps;

#[xdp]
pub fn raft_main(ctx: XdpContext) -> u32 {
    match try_raft_main(ctx) {
        Ok(ret) => ret,
        Err(_) => xdp_action::XDP_ABORTED,
    }
}

fn try_raft_main(ctx: XdpContext) -> Result<u32, ()> {
    let ethhdr: *mut EthHdr = helpers_xdp::ptr_at(&ctx, 0)?;
    match unsafe { (*ethhdr).ether_type } {
        EtherType::Ipv4 => {}
        _ => return Ok(xdp_action::XDP_PASS),
    }

    let ipv4hdr: *mut Ipv4Hdr = helpers_xdp::ptr_at(&ctx, EthHdr::LEN)?;
    let udphdr: *mut UdpHdr = helpers_xdp::ptr_at(&ctx, EthHdr::LEN + Ipv4Hdr::LEN)?;

    let source_addr = u32::from_be(unsafe { (*ipv4hdr).src_addr });

    let protocol: IpProto = match unsafe { (*ipv4hdr).proto } {
        IpProto::Tcp => IpProto::Tcp,
        IpProto::Udp => IpProto::Udp,
        _ => return Err(()),
    };

    let dest_port = match unsafe { (*ipv4hdr).proto } {
        IpProto::Tcp => {
            let tcphdr: *const TcpHdr =
                helpers_xdp::ptr_at(&ctx, EthHdr::LEN + Ipv4Hdr::LEN)?;
            u16::from_be(unsafe { (*tcphdr).dest })
        }
        IpProto::Udp => {
            let udphdr: *const UdpHdr =
                helpers_xdp::ptr_at(&ctx, EthHdr::LEN + Ipv4Hdr::LEN)?;
            u16::from_be(unsafe { (*udphdr).dest })
        }
        _ => return Err(()),
    };

    // Log prefix
    let execution_id = unsafe{bpf_ktime_get_ns()};

    match (protocol, dest_port) {
        (IpProto::Udp, VOTE_REQUEST_PORT) => {

            // Drop vote requests if currently in a Leader state.
            if helpers_raft::get_current_node_state().unwrap() == NodeState::Leader {
                debug!(&ctx, "[XDP] [{}] [->] Received vote request with term from '{}', but I'm a leader; dropping.", execution_id, source_addr);
                return Ok(xdp_action::XDP_DROP);
            }

            let incoming_term_number: u64 = match helpers_raft::parse_term_in_payload(&ctx) {
                Ok(x) => x,
                Err(_) => return Ok(xdp_action::XDP_DROP)
            };

            // Drop vote requests for terms already voted for.
            if helpers_raft::voted_for_term(incoming_term_number).unwrap_or_default() {
                debug!(&ctx, "[XDP] [{}] [->] I already voted for term '{}' from '{}'; dropping.", execution_id, incoming_term_number, source_addr);
                return Ok(xdp_action::XDP_DROP)
            } 


            let current_node_term = helpers_raft::current_node_term().unwrap_or_default();
            // Default response
            let mut vote_response: u16 = VOTE_RESPONSE_PORT_NO;

            if incoming_term_number > current_node_term {
                debug!(&ctx, "[XDP] [{}] [->] Received vote from '{}' with higher term number than mine ({} vs {}). Voting YES.", execution_id, source_addr, incoming_term_number, current_node_term);
                vote_response = VOTE_RESPONSE_PORT_YES;
            } else {
                debug!(&ctx, "[XDP] [{}] [->] Received vote from '{}' with lower term number than mine ({} vs {}). Voting NO.", execution_id, source_addr, incoming_term_number, current_node_term);
                vote_response = VOTE_RESPONSE_PORT_NO;
            }

            // Record voting for incoming term.
            match maps::VOTE_TERMS.insert(&incoming_term_number, &true, 0) {
                    Ok(()) => {},
                    Err(_) => todo!()
                }

            let decision = match vote_response {
                VOTE_RESPONSE_PORT_YES => "YES",
                VOTE_RESPONSE_PORT_NO => "NO",
                _     => "unknown",
            };


            debug!(&ctx, "[XDP] [{}] [->] Final vote: '{}' to '{}' (incoming {} vs my term {})", execution_id, decision, source_addr, incoming_term_number, current_node_term);

            unsafe {
                let src_addr = (*ipv4hdr).src_addr;
                let dst_addr = (*ipv4hdr).dst_addr;
                let src_mac =  (*ethhdr).src_addr;
                let dst_mac =  (*ethhdr).dst_addr;
                (*ipv4hdr).dst_addr = src_addr; 
                (*ethhdr).dst_addr = src_mac;
                (*udphdr).dest = u16::to_be(vote_response); 
                (*ipv4hdr).src_addr = dst_addr; 
                (*ethhdr).src_addr = dst_mac;
            }

            return Ok(xdp_action::XDP_TX);
        },

        // Vote response port.
        (IpProto::Udp, VOTE_RESPONSE_PORT_YES) => {
            if helpers_raft::get_current_node_state().unwrap() == NodeState::Candidate {
                let current_node_term = helpers_raft::current_node_term().unwrap_or_default();

                match maps::VOTE_RESULTS.insert(&source_addr, &1, 0) {
                    Ok(()) => {
                        debug!(&ctx, "[XDP] [{}] [<-] Received 'NO' from {} for term {}", execution_id, source_addr, current_node_term);
                    },
                    Err(_) => todo!()
                }
            }

            return Ok(xdp_action::XDP_DROP);
        },

        // Vote response port.
        (IpProto::Udp, VOTE_RESPONSE_PORT_NO) => {
            if helpers_raft::get_current_node_state().unwrap() == NodeState::Candidate {
                let current_node_term = helpers_raft::current_node_term().unwrap_or_default();

                match maps::VOTE_RESULTS.insert(&source_addr, &0, 0) {
                    Ok(()) => {
                        debug!(&ctx, "[XDP] [{}] [<-] Received 'NO' from {} for term {}", execution_id, source_addr, current_node_term);
                    },
                    Err(_) => todo!()
                }
            }

            return Ok(xdp_action::XDP_DROP);
        },


        // Heartbeat request packets handled by nodes receiving heartbeat packets from the leader.
        (IpProto::Udp, HEARTBEAT_REQUEST_PORT) => {
            if !helpers_raft::is_term_in_payload(&ctx) {
                warn!(&ctx, "[XDP] [{}]: Received a healthcheck packet, but Raft term is not present. Ignorning.", dest_port);
                return Ok(xdp_action::XDP_PASS);
            };

            let incoming_term_number: u64 = match helpers_raft::parse_term_in_payload(&ctx) {
                Ok(x) => x,
                Err(_) => {
                    warn!(&ctx, "[XDP] [{}]: Unable to parse Raft term number, ignoring.", dest_port);
                    return Ok(xdp_action::XDP_PASS)
                }
            };

            // Transition to follower state, if current state is candidate.
            if helpers_raft::get_current_node_state().unwrap() != NodeState::Follower {
                match helpers_raft::become_follower() {
                    Ok(_) => info!(&ctx, "[XDP] [{}] Received a new heartbeat from leader '{}' with term '{}', transitioned to Follower state.", execution_id, source_addr, incoming_term_number),
                    Err(_) => return Ok(xdp_action::XDP_DROP)
                };
            }

            let current_node_term = helpers_raft::current_node_term().unwrap_or_default();

            match helpers_raft::update_node_term(incoming_term_number) {
                Ok(_) => info!(&ctx, "[XDP] [{}] Updated node term to {} (mine was {}).", execution_id, incoming_term_number, current_node_term),
                Err(_) => return Ok(xdp_action::XDP_DROP)
            };

            match helpers_raft::update_leader_metadata(source_addr, incoming_term_number) {
                Ok(_) => debug!(&ctx, "[XDP] [{}] Updated leader metadata.", execution_id),
                Err(_) => return Ok(xdp_action::XDP_DROP)
            };
            

            // Send heartbeat response.
            unsafe {
                let src_addr = (*ipv4hdr).src_addr;
                let dst_addr = (*ipv4hdr).dst_addr;
                let src_mac =  (*ethhdr).src_addr;
                let dst_mac =  (*ethhdr).dst_addr;
                (*ipv4hdr).dst_addr = src_addr; 
                (*ethhdr).dst_addr = src_mac;
                (*udphdr).dest = u16::to_be(HEARTBEAT_RESPONSE_PORT);
                (*ipv4hdr).src_addr = dst_addr; 
                (*ethhdr).src_addr = dst_mac;
            }

            return Ok(xdp_action::XDP_TX)
        },

        // Heartbeat response packets handled by the leader.
        (IpProto::Udp, HEARTBEAT_RESPONSE_PORT) => {
            unsafe {
                if maps::FOLLOWERS.get(&source_addr).is_some() {
                    let heartbeat_request_timestamp: u64 = match maps::FOLLOWERS.get(&source_addr) {
                        Some(value) => *value,
                        None => {
                            0
                        }
                    };

                    let heartbeat_request_latency: u64 = bpf_ktime_get_ns() - heartbeat_request_timestamp;

                    match maps::HEARTBEAT_LATENCY.insert(&source_addr, &heartbeat_request_latency, 0) {
                        Ok(()) => {
                            debug!(&ctx, "[XDP] Received a heartbeat from {},{},{}", source_addr, bpf_ktime_get_ns(), heartbeat_request_latency);
                        },
                        Err(_) => todo!()
                    }
                    
                }
            }

            return Ok(xdp_action::XDP_DROP) // Drop heartbeat response packet.
        },
        (_, _) => {
            debug!(&ctx, "Other traffic which is ignored...");
        },
    }

    Ok(xdp_action::XDP_PASS) // Allow unmatching traffic to pass.
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe { core::hint::unreachable_unchecked() }
}
