#![no_std]

#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct LeaderNode {
    pub last_seen: u64,
    pub source_addr_raw: u32,
    pub term_id: u64,
}

#[derive(Copy, Clone, Debug)]
#[repr(C)]

pub struct CurrentNode {
    pub state: NodeState,
    pub term: u64,
    pub peers: [u32; 2],
    pub vote: Vote,
}

#[derive(Copy, Clone, Debug, PartialEq, Default)]
#[repr(C)]
pub enum NodeState {
    #[default]
    Follower,
    Candidate,
    Leader,
}

#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(C)]
pub struct Vote {
    pub in_progress: bool,
    pub started_ts: u64,
    pub ended_ts: u64,
    pub election_timeout: u64,
}

#[cfg(feature = "user")]
unsafe impl aya::Pod for LeaderNode {}

#[cfg(feature = "user")]
unsafe impl aya::Pod for CurrentNode {}

// ports
pub const VOTE_REQUEST_PORT: u16 = 28000;
pub const VOTE_RESPONSE_PORT_NO: u16 = 29000;
pub const VOTE_RESPONSE_PORT_YES: u16 = 29001;
pub const HEARTBEAT_REQUEST_PORT: u16 = 27001;
pub const HEARTBEAT_RESPONSE_PORT: u16 = 27000;
