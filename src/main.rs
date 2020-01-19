use crossbeam_channel::{bounded, select};
use failure::Error;
use log::info;
use rumqtt::{MqttClient, MqttOptions, QoS, SecurityOptions};
use serde::Deserialize;
use std::fs::File;
use std::io::BufReader;

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

fn main() -> Result<()> {
    env_logger::init();
    info!("Reading config file");
    let file_reader = BufReader::new(File::open("config.json")?);
    let config: Config = serde_json::from_reader(file_reader)?;

    let mut options = MqttOptions::new("testEmil", config.host, config.port);
    if !config.mqtt_user.is_empty() {
        options = options.set_security_opts(SecurityOptions::UsernamePassword(
            config.mqtt_user,
            config.mqtt_pass,
        ));
    }
    let (mut client, notifications) = MqttClient::start(options)?;

    let (done_tx, done_rx) = bounded(1);

    ctrlc::set_handler(move || {
        let _ = done_tx.send(());
    })?;
    client.subscribe(
        format!("{}{}", config.topic, "#"),
        QoS::from_u8(config.qos)?,
    )?;

    loop {
        select! {
            recv(notifications) -> not => {
                println!("{:?}", not);
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
