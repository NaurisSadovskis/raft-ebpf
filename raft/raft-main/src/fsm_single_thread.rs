use crate::fsm_candidate;
use crate::fsm_follower;
use crate::fsm_leader;
use crate::state;
use raft_main_common::NodeState;
use std::sync::{Arc, Mutex};

pub fn shared_loop(state: &state::AppState) {
    let counter = Arc::new(Mutex::new(0)); // counter to simulate a failure after LEADER_HEARTBEAT_CYCLES_BEFORE_CRASH.

    loop {
        if state.current_term_id() == 100 {
            std::process::exit(0)
        }

        match state.get_current_state() {
            NodeState::Leader => fsm_leader::leader(state, counter.clone()),
            NodeState::Candidate => fsm_candidate::candidate(state),
            NodeState::Follower => fsm_follower::follower(state),
        }
    }
}
