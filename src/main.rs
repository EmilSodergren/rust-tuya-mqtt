use serde::Deserialize;
use std::fs::File;

#[derive(Deserialize, Debug)]
struct Config {
    host: String,
    port: u8,
    topic: String,
    mqtt_user: String,
    mqtt_pass: String,
    qos: u8,
}

fn main() {
    let mut file = File('config.json')
}
