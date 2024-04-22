use crate::state;
use crate::values;
use log::info;
use raft_main_common::NodeState;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;

pub fn leader(state: &state::AppState, c: Arc<Mutex<i32>>) {
    state.send_heartbeat_rpcs();
    let mut current_cyle_counter = c.lock().unwrap();
    *current_cyle_counter += 1;

    if *current_cyle_counter > values::LEADER_HEARTBEAT_CYCLES_BEFORE_CRASH {
        info!("[leader] Simulating crash and becoming a follower");
        // sleeping to avoid being the first node to detect leader absence.
        sleep(Duration::from_millis(
            values::LEADER_COMMUNICATION_TIMEOUT_MS + 1,
        ));

        *current_cyle_counter = 0; // reset cycle counter.
        state.update_leader_last_seen_time(); // hack; see fn comment.
        state.become_follower();
    } else {
        sleep(Duration::from_millis(values::LEADER_HEARTBEAT_FREQUENCY_MS));
    }
}

pub fn leader_loop(state: &state::AppState) {
    let c = Arc::new(Mutex::new(0));

    loop {
        if state.get_current_state() != NodeState::Leader {
            continue;
        }

        leader(state, c.clone())
    }
}
