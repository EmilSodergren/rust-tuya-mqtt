use crate::error::ErrorKind;
use anyhow::{anyhow, Context, Error, Result};
use log::{debug, error, info, warn};
use pretty_env_logger::env_logger::WriteStyle;
use rumqttc::{qos, Client, Event, MqttOptions, Packet, Publish};
use rust_tuyapi::tuyadevice::TuyaDevice;
use rust_tuyapi::{payload, TuyaType};
use serde::Deserialize;
use std::fs::File;
use std::io::BufReader;
use std::io::Write;
use std::net::IpAddr;
use std::str::FromStr;
use std::thread;

mod error;

// RETRIES will be exponential: 10ms 100ms 1000ms 10_000ms
const RETRIES: usize = 4;

#[derive(Deserialize, Debug)]
struct Config {
    #[serde(default = "default_mqtt_id")]
    mqtt_id: String,
    host: String,
    port: u16,
    topic: String,
    mqtt_user: String,
    mqtt_pass: String,
    qos: u8,
}

fn default_mqtt_id() -> String {
    String::from("rust-tuya-mqtt")
}

#[derive(Debug, PartialEq)]
struct Topic {
    tuya_ver: String,
    tuya_id: String,
    tuya_key: String,
    ip: IpAddr,
}

impl FromStr for Topic {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let content: Vec<&str> = s.split('/').collect();
        if content.len() < 5 {
            return Err(ErrorKind::TopicTooShort.into());
        };
        if content[0] != "tuya" {
            return Err(ErrorKind::NotATuyaTopic.into());
        };

        Ok(Topic {
            tuya_ver: content[1].to_string(),
            tuya_id: content[2].to_string(),
            tuya_key: content[3].to_string(),
            ip: content[4].parse()?,
        })
    }
}

fn handle_publish(publish: Publish) -> Result<()> {
    info!("Received packet {:?}", &publish);
    let topic: Topic = publish
        .topic
        .parse()
        .context(format!("Trying to parse {}", &publish.topic))?;
    debug!("{:?}", topic);
    let mqtt_state =
        std::str::from_utf8(&publish.payload).context("Mqtt payload is not valid utf8")?;
    let tuya_payload = payload(&topic.tuya_id, TuyaType::Socket, &mqtt_state)
        .context("Could not get Payload from MQTT message")?;
    let tuya_device = TuyaDevice::create(&topic.tuya_ver, Some(&topic.tuya_key), topic.ip)
        .context("Could not create TuyaDevice")?;
    let pkid = publish.pkid as u32;
    set(&tuya_device, pkid, &tuya_payload)
}

// This function sends a command to a device. It will detect a number of errors that might occur
// due to bad behaving devices and resend the command until it get a valid response.
fn set(dev: &TuyaDevice, pkid: u32, payload: &str) -> Result<()> {
    use retry::{delay::Exponential, retry, Error};
    use rust_tuyapi::error::ErrorKind::{BadTcpRead, TcpError};
    match retry(Exponential::from_millis(10).take(RETRIES), || {
        match dev.set(payload, pkid) {
            Ok(()) => Ok(()),
            Err(BadTcpRead) => {
                warn!(
                    "The device did not return a valid response message ({}), trying again",
                    pkid
                );
                Err(anyhow!(BadTcpRead))
            }
            Err(TcpError(e)) => {
                warn!("The communication failed with: ({}), trying again", e);
                Err(anyhow!(TcpError(e)))
            }
            Err(e) => Err(anyhow!(e)),
        }
    }) {
        Ok(()) => Ok(()),
        Err(e) => match e {
            Error::Operation {
                error,
                total_delay: _,
                tries: _,
            } => Err(error),
            Error::Internal(s) => Err(anyhow!(s)),
        },
    }
}

fn handle_notification(event: Event) {
    let res = match event {
        Event::Incoming(packet) => match packet {
            Packet::Publish(publish) => {
                handle_publish(publish).context("Handling publish notification.")
            }
            _ => Err(anyhow!("EMIL")),
        },
        Event::Outgoing(_) => Err(anyhow!("packet")),
    };
    match res {
        Ok(_) => {}
        Err(err) => error!("{:?}", err),
    };
}

fn initialize_logger() {
    if let Some(s) = std::env::var("TUYA_LOG").map_or(std::env::var("RUST_LOG").ok(), |s| {
        Some(format!("rust_tuyapi={},rust_tuya_mqtt={}", s, s))
    }) {
        pretty_env_logger::formatted_builder()
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
            .write_style(WriteStyle::Always)
            .init()
    }
}

fn main() -> anyhow::Result<()> {
    initialize_logger();
    info!("Reading config file");
    let file_reader = BufReader::new(File::open("config.json")?);
    let config: Config = serde_json::from_reader(file_reader)?;
    debug!("Read {:#?}", config);

    let mut options = MqttOptions::new(config.mqtt_id, config.host, config.port);
    options.set_keep_alive(10);
    if !config.mqtt_user.is_empty() {
        options.set_credentials(config.mqtt_user, config.mqtt_pass);
    }

    let (mut client, mut connection) = Client::new(options, 10);
    let mut client2 = client.clone();
    ctrlc::set_handler(move || client2.cancel().unwrap())?;
    client.subscribe(
        format!("{}{}", config.topic, "#"),
        qos(config.qos).map_err(Error::msg)?,
    )?;
    for n in connection.iter() {
        let n = n.map_err(Error::msg)?;
        thread::spawn(|| handle_notification(n));
    }
    info!("Bye!");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn topic_from_string() {
        assert!(Topic::from_str("ayut/ver3.3/adf").is_err());
        assert!(Topic::from_str("tuya/ver3.3/adf/18356").is_err());
        let topic = Topic::from_str(
            "tuya/ver3.3/545c7250ecf8bc58a8fd/6597042c66252228/192.168.170.7/command",
        )
        .unwrap();
        assert_eq!(
            topic,
            Topic {
                tuya_ver: "ver3.3".to_string(),
                tuya_id: "545c7250ecf8bc58a8fd".to_string(),
                tuya_key: "6597042c66252228".to_string(),
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
        assert!(config.mqtt_id == "test_mqtt_d")
    }
}
