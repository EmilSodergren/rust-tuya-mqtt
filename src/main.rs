use crate::error::ErrorKind;
use atomic_counter::{AtomicCounter, RelaxedCounter};
use crossbeam_channel::{bounded, select};
use failure::Error;
use log::{debug, info, warn};
use rumqtt::{MqttClient, MqttOptions, Notification, Publish, QoS, SecurityOptions};
use rust_tuyapi::mesparse::TuyaDevice;
use rust_tuyapi::{payload, TuyaType};
use serde::Deserialize;
use std::fs::File;
use std::io::BufReader;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;
use std::thread;

mod error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Deserialize, Debug)]
struct Config {
    host: String,
    port: u16,
    topic: String,
    mqtt_user: String,
    mqtt_pass: String,
    qos: u8,
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

fn handle_publish(publish: Publish, counter: Arc<RelaxedCounter>) {
    info!("Received packet {:?}", &publish);
    let topic: Topic = publish
        .topic_name
        .parse()
        .expect("Could not parse the package");
    debug!("{:?}", topic);
    let mqtt_state = std::str::from_utf8(&publish.payload).expect("Payload is not valid utf8");
    let tuya_payload = payload(&topic.tuya_id, TuyaType::Socket, &mqtt_state)
        .expect("Could not get Payload from MQTT message");
    let tuya_device = TuyaDevice::create(
        &topic.tuya_ver,
        Some(&topic.tuya_key),
        Some(SocketAddr::new(topic.ip, 6668)),
    )
    .expect("Could not create TuyaDevice");
    tuya_device
        .set(&tuya_payload, counter.inc() as u32)
        .expect("Could not set value to TuyaDevice");
}

fn handle_notification(notification: Notification, counter: Arc<RelaxedCounter>) {
    match notification {
        Notification::Publish(publish) => handle_publish(publish, counter),
        _ => warn!("Received unhandled notification {:?}", notification),
    };
}

fn main() -> Result<()> {
    pretty_env_logger::init();
    info!("Reading config file");
    let file_reader = BufReader::new(File::open("config.json")?);
    let config: Config = serde_json::from_reader(file_reader)?;
    debug!("Read {:#?}", config);

    let mut options =
        MqttOptions::new("rust-tuya-mqtt", config.host, config.port).set_keep_alive(10);
    options = if !config.mqtt_user.is_empty() {
        options.set_security_opts(SecurityOptions::UsernamePassword(
            config.mqtt_user,
            config.mqtt_pass,
        ))
    } else {
        options
    };

    let (mut client, notifications) = MqttClient::start(options)?;

    let (done_tx, done_rx) = bounded(1);

    ctrlc::set_handler(move || {
        let _ = done_tx.send(());
    })?;
    client.subscribe(
        format!("{}{}", config.topic, "#"),
        QoS::from_u8(config.qos)?,
    )?;
    let counter = Arc::new(RelaxedCounter::new(0));
    loop {
        select! {
            recv(notifications) -> n => {
                let n = n?;
                let a_counter = Arc::clone(&counter);
                thread::spawn(|| handle_notification(n, a_counter));
            },
            recv(done_rx) -> _ => {
                info!("Received termination, shutting down the mqtt connection");
                client.shutdown()?;
                break
            },
        }
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
}
