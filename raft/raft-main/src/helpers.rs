use crate::values;
use nix::time::clock_gettime;
use rand::{thread_rng, Rng};
use std::net::Ipv4Addr;
use std::time::Duration;

pub fn get_election_timeout_jitter_ns() -> u64 {
    let mut rng = thread_rng();
    rng.gen_range(values::ELECTION_TIMEOUT_JITTER_MIN_NS..values::ELECTION_TIMEOUT_JITTER_MAX_NS)
}

pub fn get_leader_communication_jitter() -> u64 {
    let mut rng = thread_rng();
    rng.gen_range(
        values::LEADER_COMMUNICATION_JITTER_MIN_MS..values::LEADER_COMMUNICATION_JITTER_MAX_MS,
    )
}

pub fn get_current_clock_ms() -> u64 {
    Duration::from(clock_gettime(nix::time::ClockId::CLOCK_MONOTONIC).unwrap()).as_millis() as u64
}

pub fn get_current_clock_ns() -> u64 {
    Duration::from(clock_gettime(nix::time::ClockId::CLOCK_MONOTONIC).unwrap()).as_nanos() as u64
}

// C onvert dot-delimited IP address to u32 representation.
pub fn ip_string_to_u32(ip_str: &str) -> Result<u32, ()> {
    let parts: Vec<&str> = ip_str.split('.').collect();

    if parts.len() != 4 {
        return Err(());
    }

    let mut ip_vec = Vec::new();

    for part in parts {
        if let Ok(num) = part.parse::<u8>() {
            ip_vec.push(num);
        } else {
            return Err(());
        }
    }

    match Ipv4Addr::new(ip_vec[0], ip_vec[1], ip_vec[2], ip_vec[3]).try_into() {
        Ok(ip_addr) => Ok(ip_addr),
        Err(_) => Err(()),
    }
}
