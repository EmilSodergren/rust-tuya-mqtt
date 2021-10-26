use crate::TuyaType;
use rust_tuyapi::Payload;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::time::Duration;

#[derive(Serialize, Deserialize, Debug)]
struct GetPayload {
    dev_id: String,
    gw_id: String,
}

pub fn now_as_u32() -> Option<u32> {
    use std::time::SystemTime;
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .as_ref()
        .map(|dur| Duration::as_secs(dur))
        .ok()
        .and_then(|x| Some(x as u32))
}

// Convenience method to create a valid Tuya style payload from a device ID and a state received
// from mqtt.
// Calling:
//
// payload("abcde", TuyaType::Socket, "on");
//
// will render:
//
// {
//   dev_id: abcde,
//   gw_id: abcde,
//   uid: "",
//   t: 132478194, <-- current time
//   dps: {
//     1: true
//   }
// }
//
pub fn payload(device_id: &str, tt: TuyaType, state: &str) -> Payload {
    Payload::new(
        device_id.to_string(),
        Some(device_id.to_string()),
        None,
        now_as_u32(),
        None,
        Some(dps(tt, state)),
    )
}

pub fn _get_payload(device_id: &str) -> Payload {
    Payload::new(
        device_id.to_string(),
        Some(device_id.to_string()),
        None,
        now_as_u32(),
        None,
        Some(HashMap::new()),
    )
}

fn dps(tt: TuyaType, state: &str) -> HashMap<String, serde_json::Value> {
    match tt {
        TuyaType::Socket => socket_dps(state),
    }
}

fn socket_dps(state: &str) -> HashMap<String, serde_json::Value> {
    let mut map = HashMap::new();
    if state.eq_ignore_ascii_case("on") || state.eq_ignore_ascii_case("1") {
        map.insert("1".to_string(), json!(true));
    } else {
        map.insert("1".to_string(), json!(false));
    }
    map
}
