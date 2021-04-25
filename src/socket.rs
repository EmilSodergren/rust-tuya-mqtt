use crate::TuyaType;
use rust_tuyapi::{Payload, PayloadStruct};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::time::SystemTime;

#[derive(Serialize, Deserialize, Debug)]
struct GetPayload {
    dev_id: String,
    gw_id: String,
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
    Payload::Struct(PayloadStruct {
        dev_id: device_id.to_string(),
        gw_id: Some(device_id.to_string()),
        uid: None,
        t: Some(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs() as u32,
        ),
        dps: dps(tt, state),
    })
}

pub fn _get_payload(device_id: &str) -> anyhow::Result<String> {
    Ok(serde_json::to_string(&GetPayload {
        dev_id: device_id.to_string(),
        gw_id: device_id.to_string(),
    })?)
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
