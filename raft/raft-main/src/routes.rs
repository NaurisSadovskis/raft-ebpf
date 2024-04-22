use crate::helpers::ip_string_to_u32;
use crate::state;
use axum::extract;
use axum::extract::State;
use axum::response::Json;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::net::Ipv4Addr;

#[derive(Debug, Deserialize)]
pub struct IPPayload {
    ip: String,
}

#[derive(Debug, Serialize)]
pub struct FollowerState {
    ip: String,
    ip_raw: u32,
    latency_ns: u64,
    latency_ms: f64,
    last_seen_epoch: u64, // epoch
}

// add_follower allows dynamically adding a new follower via POST request.
pub async fn add_follower(
    State(state): State<state::AppState>,
    payload: extract::Json<IPPayload>,
) -> Json<Value> {
    let ip_addr_str = payload.ip.to_string();
    let ip_address: u32 = match ip_string_to_u32(&payload.ip) {
        Ok(ip_address) => ip_address,
        Err(_) => {
            let error_msg = format!("add_follower: invalid IP address: {}", ip_addr_str);
            warn!("{}", error_msg);
            return Json(json!({ "error": error_msg}));
        }
    };

    let follower_data = state.followers.clone();
    let mut data = follower_data.lock().unwrap();

    // Second item is "last_seen" value.
    match data.insert(ip_address, 0, 0) {
        Ok(()) => Json(json!({ "errors": "none" })),
        Err(err) => Json(json!({ "errors": err.to_string() })),
    }
}

// delete_follower deletes a follder if it exists in a map.
pub async fn delete_follower(
    State(state): State<state::AppState>,
    payload: extract::Json<IPPayload>,
) -> Json<Value> {
    let ip_addr_str = payload.ip.to_string();
    let ip_address: u32 = match ip_string_to_u32(&payload.ip) {
        Ok(ip_address) => ip_address,
        Err(_) => {
            let error_msg = format!("delete_follower: invalid IP address: {}", ip_addr_str);
            warn!("{}", error_msg);
            return Json(json!({ "error": error_msg}));
        }
    };

    let follower_data = state.followers.clone();
    let mut data = follower_data.lock().unwrap();

    // TODO: Term number needs to be inserted somewhere, so it can be eventually compared?
    match data.remove(&ip_address) {
        Ok(()) => {
            info!(
                "delete_follower: successfully removed IP address: {}",
                ip_addr_str
            )
        }
        Err(err) => warn!("delete_follower: cannot delete a follower: {}", err),
    }

    Json(json!({ "errors": "none" }))
}

// list_followers return IP addresses in the eBPF followers map.
pub async fn list_followers(State(state): State<state::AppState>) -> Json<Value> {
    let mut response: Vec<FollowerState> = Vec::new();

    let heartbeat_latency_data = state.heartbeat_latency.clone();
    let heartbeat_data = heartbeat_latency_data.lock().unwrap();

    for entry in heartbeat_data.keys() {
        let ip = entry.unwrap();
        let ip_string = Ipv4Addr::from(ip).to_string();

        let latency_ns: u64 = match heartbeat_data.get(&ip, 0) {
            Ok(latency_ns) => latency_ns,
            Err(_err) => todo!(),
        };

        let latency_ms: f64 = latency_ns as f64 / 1_000_000.00; // TODO: fix precision

        let follower_data = state.followers.clone();
        let followers = follower_data.lock().unwrap();

        let last_seen_boot_time: u64 = match followers.get(&ip, 0) {
            Ok(last_seen_boot_time) => last_seen_boot_time,
            Err(_err) => todo!(),
        };

        response.push(FollowerState {
            ip_raw: ip,
            ip: ip_string,
            latency_ns,
            latency_ms,
            last_seen_epoch: last_seen_boot_time, // TODO
        })
    }
    Json(json!({ "data": response }))
}
