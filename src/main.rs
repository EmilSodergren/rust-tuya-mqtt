use crate::socket::payload;
use anyhow::{anyhow, Context, Error, Result};
use env_logger::Builder;
use log::{debug, error, info, trace, warn};
use rumqttc::{qos, Client, Event, MqttOptions, Packet, Publish};
use rust_tuyapi::tuyadevice::TuyaDevice;
use rust_tuyapi::Result as TuyaResult;
use rust_tuyapi::{Payload, Truncate};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Display;
use std::fs::File;
use std::io::{BufReader, Write};
use std::net::IpAddr;
use std::sync::Arc;
use std::thread;

mod socket;

// RETRIES will be exponential: (skipped 10ms) 100ms 1000ms 10_000ms
const SKIP: usize = 1;
const RETRIES: usize = 3;

pub enum TuyaType {
    Socket,
}

type DeviceMap = HashMap<String, DeviceInfo>;

#[derive(Deserialize, Serialize, Debug)]
struct Config {
    #[serde(default = "default_mqtt_id")]
    mqtt_id: String,
    host: String,
    #[serde(default = "default_port")]
    port: u16,
    #[serde(default = "default_topic")]
    topic: String,
    mqtt_user: String,
    mqtt_pass: String,
    qos: u8,
}

fn default_topic() -> String {
    String::from("tuya/")
}

fn default_mqtt_id() -> String {
    String::from("rust-tuya-mqtt")
}

fn default_port() -> u16 {
    1883_u16
}

#[derive(Deserialize, Debug, Serialize, PartialEq, Clone)]
struct DeviceInfo {
    name: String,
    version: String,
    id: String,
    key: String,
    ip: IpAddr,
}

impl Display for DeviceInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Topic: Ip: {}, Id: {}, Key: {}, Version: {}",
            self.ip, self.id, self.key, self.version
        )
    }
}

impl Truncate for DeviceInfo {
    /// Take the last 5 characters and prefix them with "..."
    fn truncate(&self) -> DeviceInfo {
        DeviceInfo {
            name: self.name.clone(),
            version: self.version.clone(),
            id: String::from("...") + Self::truncate_str(&self.id),
            key: String::from("...") + Self::truncate_str(&self.key),
            ip: self.ip,
        }
    }
}

impl DeviceInfo {
    fn from_str_and_devices(s: &str, devs: &DeviceMap) -> Result<Self> {
        let content: Vec<&str> = s.split('/').collect();
        // First try to match the topic name against entries in the DeviceMap
        if content.len() > 2 {
            if let Some(topic) = devs.get(content[content.len() - 2]) {
                return Ok(topic.clone());
            }
        }
        // Then try to parse the topic as a device
        if content.len() > 5 {
            return Ok(DeviceInfo {
                name: "".to_string(),
                version: content[1].to_string(),
                id: content[2].to_string(),
                key: content[3].to_string(),
                ip: content[4].parse()?,
            });
        }
        Err(anyhow!("Not a valid Topic"))
    }
}

fn handle_publish(publish: Publish, devices: &DeviceMap, full_display: bool) -> Result<()> {
    let topic: DeviceInfo = DeviceInfo::from_str_and_devices(&publish.topic, devices)
        .context(format!("Trying to parse {}", &publish.topic))?;

    if full_display {
        debug!("{}", topic);
    } else {
        debug!("{}", topic.truncate());
    }
    let mqtt_state =
        std::str::from_utf8(&publish.payload).context("Mqtt payload is not valid utf8")?;
    let tuya_payload = payload(&topic.id, TuyaType::Socket, &mqtt_state);
    let tuya_device = TuyaDevice::create(&topic.version, Some(&topic.key), topic.ip)
        .context("Could not create TuyaDevice")?;
    let pkid = publish.pkid as u32;
    set(&tuya_device, pkid, tuya_payload)
}

#[inline]
fn print_warnings_on_failure(pkid: u32, res: &TuyaResult<()>) {
    use rust_tuyapi::error::ErrorKind::{BadTcpRead, TcpError};
    if let Err(ref e) = res {
        match e {
            BadTcpRead => {
                warn!(
                    "The device did not return a valid response message ({}), trying again",
                    pkid
                );
            }
            TcpError(e) => {
                warn!("The communication failed with: ({}), trying again", e);
            }
            _ => (),
        }
    }
}

// This function sends a command to a device. It will detect a number of errors that might occur
// due to bad behaving devices and resend the command until it get a valid response.
fn set(dev: &TuyaDevice, pkid: u32, payload: Payload) -> Result<()> {
    use retry::{delay::Exponential, retry, Error};
    match retry(
        Exponential::from_millis(10).skip(SKIP).take(RETRIES),
        || {
            let r = dev.set(payload.clone(), pkid);
            print_warnings_on_failure(pkid, &r);
            r
        },
    ) {
        Ok(()) => Ok(()),
        Err(e) => match e {
            Error::Operation {
                error,
                total_delay: _,
                tries: _,
            } => Err(anyhow!(error)),
            Error::Internal(s) => Err(anyhow!(s)),
        },
    }
}

fn handle_notification(event: Event, devices: &DeviceMap, full_display: bool) -> Result<()> {
    match event {
        Event::Incoming(packet) => match packet {
            Packet::Publish(publish) => handle_publish(publish, devices, full_display)
                .context("Handling publish notification."),
            _ => {
                trace!("Unhandled incoming packet {:?}", packet);
                Ok(())
            }
        },
        Event::Outgoing(packet) => {
            trace!("Unhandled incoming packet {:?}", packet);
            Ok(())
        }
    }
}

fn initialize_logger() {
    use std::env::var;
    // Make it possible to declare TUYA_LOG as an easier logger filter
    if let Some(s) = var("TUYA_LOG").map_or(var("RUST_LOG").ok(), |s| {
        Some(format!("rust_tuyapi={},rust_tuya_mqtt={}", s, s))
    }) {
        Builder::new()
            .format(|buf, record| {
                writeln!(
                    buf,
                    "{} [{}] - {}",
                    chrono::Local::now().format("%Y-%m-%dT%H:%M:%S"),
                    record.level(),
                    record.args()
                )
            })
            .parse_filters(&s)
            .init()
    }
}

fn read_devices(file: File) -> Result<Arc<DeviceMap>> {
    let mut map: DeviceMap = HashMap::new();
    info!("Reading devices from file");
    let file_reader = BufReader::new(file);
    let devices: Vec<DeviceInfo> = serde_json::from_reader(file_reader)?;
    for device in devices.iter() {
        map.insert(device.name.clone(), device.clone());
    }
    debug!("Read Devices {:#?}", devices);
    Ok(Arc::new(map))
}

fn main() -> anyhow::Result<()> {
    initialize_logger();
    let full_display = std::env::var("TUYA_FULL_DISPLAY").map_or_else(|_| false, |_| true);
    info!(
        "Full display {} - declare env variable TUYA_FULL_DISPLAY to set",
        full_display
    );
    info!("Reading config file");
    let config: Config = serde_json::from_reader(BufReader::new(File::open("config.json")?))?;
    debug!("Read {:#?}", config);

    // Read the devices.json configuration file if it exist, set empty device map otherwise
    let device_map =
        File::open("devices.json").map_or(Ok(Arc::new(DeviceMap::new())), read_devices)?;

    let mut options = MqttOptions::new(config.mqtt_id, config.host, config.port);
    options.set_keep_alive(10);
    if !config.mqtt_user.is_empty() {
        options.set_credentials(config.mqtt_user, config.mqtt_pass);
    }

    let (mut client, mut connection) = Client::new(options, 10);
    let mut client2 = client.clone();
    ctrlc::set_handler(move || {
        info!("Stopping the client");
        client2.cancel().unwrap();
    })?;
    client.subscribe(
        format!("{}#", config.topic),
        qos(config.qos).map_err(Error::msg)?,
    )?;
    for n in connection.iter() {
        match n {
            Ok(n) => {
                // Give each thread an Arc of the devices
                let map = device_map.clone();
                thread::spawn(move || match handle_notification(n, &map, full_display) {
                    Ok(_) => (),
                    Err(e) => error!("{}", e),
                });
            }
            Err(e) => {
                error!("{}", e);
                thread::sleep(std::time::Duration::from_secs(5));
            }
        };
    }
    info!("Bye!");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn topic_from_string() {
        assert!(DeviceInfo::from_str_and_devices("ayut/ver3.3/adf", &DeviceMap::new()).is_err());
        assert!(
            DeviceInfo::from_str_and_devices("tuya/ver3.3/adf/18356", &DeviceMap::new()).is_err()
        );
        let topic = DeviceInfo::from_str_and_devices(
            "tuya/ver3.3/545c7250ecf8bc58a8fd/6597042c66252228/192.168.170.7/command",
            &DeviceMap::new(),
        )
        .unwrap();
        assert_eq!(
            topic,
            DeviceInfo {
                name: "".to_string(),
                version: "ver3.3".to_string(),
                id: "545c7250ecf8bc58a8fd".to_string(),
                key: "6597042c66252228".to_string(),
                ip: "192.168.170.7".parse().unwrap(),
            },
        );
    }
    #[test]
    fn config_without_mqttid() {
        let config_json = r#"
{
    "host": "192.168.1.1",
    "port": 1883,
    "topic": "tuya/",
    "mqtt_user": "",
    "mqtt_pass": "",
    "qos": 2
}
        "#;
        let config: Config = serde_json::from_str(config_json).unwrap();
        assert!(!config.mqtt_id.is_empty())
    }
    #[test]
    fn config_with_mqttid() {
        let config_json = r#"
{
    "mqtt_id": "test_mqtt_id",
    "host": "192.168.1.1",
    "port": 1883,
    "topic": "tuya/",
    "mqtt_user": "",
    "mqtt_pass": "",
    "qos": 2
}
        "#;
        let config: Config = serde_json::from_str(config_json).unwrap();
        assert!(config.mqtt_id == "test_mqtt_id")
    }
}
