use aya_bpf::{
    maps::{HashMap, Array},
    macros::map,
};
use raft_main_common::{LeaderNode, CurrentNode};


#[map]
pub static FOLLOWERS: HashMap<u32, u64> = HashMap::with_max_entries(1024, 0);
#[map]
pub static HEARTBEAT_LATENCY: HashMap<u32, u64> = HashMap::with_max_entries(1024, 0);
#[map]
pub static CURRENT_NODE: Array<CurrentNode> = Array::with_max_entries(1, 0);
#[map]
pub static LEADER_NODE: Array<LeaderNode> = Array::with_max_entries(1, 0);
#[map]
pub static VOTE_TERMS: HashMap<u64, bool> = HashMap::with_max_entries(8192, 0);
#[map]
pub static VOTE_RESULTS: HashMap<u32, u64> = HashMap::with_max_entries(1024, 0);