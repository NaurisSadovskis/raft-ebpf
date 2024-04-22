use crate::helpers;
use crate::state;
use crate::values;
use log::info;
use raft_main_common::NodeState;

pub fn follower(state: &state::AppState) {
    let jitter = helpers::get_leader_communication_jitter();

    if state.leader_last_seen() > values::LEADER_COMMUNICATION_TIMEOUT_MS + jitter {
        info!(
            "[follower] No communication received from the leader in {} ms; becoming a candidate",
            values::LEADER_COMMUNICATION_TIMEOUT_MS + jitter
        );
        state.become_candidate();
    }
}

pub fn follower_loop(state: &state::AppState) {
    loop {
        if state.get_current_state() != NodeState::Follower {
            continue;
        }

        follower(state)
    }
}
