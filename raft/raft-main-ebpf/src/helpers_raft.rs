use aya_bpf::programs::XdpContext;
use network_types::{
    eth::EthHdr,
    ip::Ipv4Hdr,
    udp::UdpHdr,
};
use aya_bpf::helpers::bpf_ktime_get_ns;
use raft_main_common::{CurrentNode, NodeState, LeaderNode};
use crate::helpers_xdp;
use crate::maps;

// Calculate if Raft term number (u64) is present in the payload.
#[inline(always)]
pub fn is_term_in_payload(ctx: &XdpContext) -> bool {
    // If there is data after 8 bytes (u64 size), it is not u64 term number.
    if helpers_xdp::ptr_exists::<[u8; 1]>(ctx, EthHdr::LEN + Ipv4Hdr::LEN + UdpHdr::LEN + 8) {
        return false;
    }

    // If size matches that of u64, it _must_ be the term number.
    if helpers_xdp::ptr_exists::<[u8;8]>(ctx, EthHdr::LEN + Ipv4Hdr::LEN + UdpHdr::LEN) {
        return true 
    }

    return false
}


#[inline(always)]
pub fn parse_term_in_payload(ctx: &XdpContext) -> Result<u64, ()> {
    let term_bytes: *mut [u8; 8] = match helpers_xdp::ptr_at(&ctx, EthHdr::LEN + Ipv4Hdr::LEN + UdpHdr::LEN) {
        Ok(x) => {
            x as *mut [u8; 8]
        }
        Err(_) => {
            return Err(());
        }
    };

    return unsafe 
     { 
        Ok(
        (u64::from((*term_bytes)[0]) << 56) + 
        (u64::from((*term_bytes)[1]) << 48) + 
        (u64::from((*term_bytes)[2]) << 40) + 
        (u64::from((*term_bytes)[3]) << 32) + 
        (u64::from((*term_bytes)[4]) << 24) + 
        (u64::from((*term_bytes)[5]) << 16) + 
        (u64::from((*term_bytes)[6]) << 8) + 
        u64::from((*term_bytes)[7])
    )
    }
}

// Get current node state.
pub fn get_current_node_state() -> Result<NodeState, ()> {
    let current_node: CurrentNode = match maps::CURRENT_NODE.get(0) {
        Some(value) => *value,
        None => { 
            return Ok(NodeState::Follower);
        }
    };
    Ok(current_node.state)
}

// Get current node term.
pub fn current_node_term() -> Result<u64, ()> {
    let current_node: CurrentNode = match maps::CURRENT_NODE.get(0) {
            Some(value) => *value,
            None => { 
                return Err(());
            }
    };

    Ok(current_node.term)
}

// Check if already voted for a given term.
pub fn voted_for_term(term: u64) -> Result<bool, ()> {
    unsafe {
        let voted: bool = match maps::VOTE_TERMS.get(&term) {
            Some(value) => *value,
            None => {
                false
            }
        };
        return Ok(voted)
    }
}

// Become follower.
pub fn become_follower() -> Result<(), ()> {
    unsafe {
        let current_node: *mut CurrentNode = match maps::CURRENT_NODE.get_ptr_mut(0) {
                Some(value) => value,
                None => { return Err(());}
        };
        (*current_node).state = NodeState::Follower;
        (*current_node).vote.in_progress = false; 
        (*current_node).vote.started_ts = 0; 
        (*current_node).vote.ended_ts = 0; 
        (*current_node).vote.election_timeout = 0; 
    }
    Ok(())
}

// Update leader metadata.
pub fn update_leader_metadata(source_addr: u32, term: u64) -> Result<(), ()> {
    unsafe {
        let leader_node: *mut LeaderNode = match maps::LEADER_NODE.get_ptr_mut(0) {
            Some(value) => value,
            None => { return Err(());}
        };

        (*leader_node).last_seen = bpf_ktime_get_ns();
        (*leader_node).source_addr_raw = source_addr;
        (*leader_node).term_id = term;
    }
    Ok(())
}

// Update node term.
pub fn update_node_term(incoming_term: u64) -> Result<(), ()>  {
    unsafe {
        let current_node: *mut CurrentNode = match maps::CURRENT_NODE.get_ptr_mut(0) {
                Some(value) => value,
                None => { return Err(());}
        };
        (*current_node).term = incoming_term
    }
    Ok(())
}