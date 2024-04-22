use crate::helpers;
use crate::state;
use crate::values;
use local_ip_address::local_ip;
use log::info;
use raft_main_common::NodeState;

pub fn candidate(state: &state::AppState) {
    let local_ip = local_ip().unwrap();

    if state.vote_in_progress() && !state.election_timed_out() && state.quorum_reached() {
        state.stop_vote();
        info!(
            "[candidate] Quorum reached after {},{},{}, becoming leader with term: {}",
            local_ip,
            helpers::get_current_clock_ns(),
            state.get_vote_duration(),
            state.current_term_id()
        );
        state.reset_vote_results().unwrap_or_default();
        state.become_leader();
        return;
    }

    if !state.vote_in_progress() {
        state.reset_vote_data();
        state.increment_term_number();
        state.send_request_vote_rpcs();
        state.start_vote();
        info!("[candidate] no vote in progress, starting vote at {} with term: {} and election timeout: {}", helpers::get_current_clock_ns(), state.current_term_id(), state.get_election_timeout());
        return;
    }

    if state.election_timed_out() {
        info!(
            "[candidate] election timed out, aborting with {} out of {} needed votes for term {}",
            state.get_current_yes_votes_from_peers(),
            values::QUORUM,
            state.current_term_id()
        );
        state.reset_vote_data();
        state.abort_vote();
        state.reset_vote_results().unwrap_or_default();
    }
}

pub fn candidate_loop(state: &state::AppState) {
    loop {
        if state.get_current_state() != NodeState::Candidate {
            continue;
        }

        candidate(state)
    }
}
