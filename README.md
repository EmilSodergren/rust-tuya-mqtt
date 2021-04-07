# rust-tuya-mqtt
[![Build Status](https://github.com/EmilSodergren/rust-tuya-mqtt/actions/workflows/mean_bean_ci.yml/badge.svg)](https://github.com/EmilSodergren/rust-tuya-mqtt/actions/workflows/mean_bean_ci.yml)

Rust program that enables controlling of Tuya/Smart Life devices via MQTT. It uses the rust-tuyapi library to talk to the devices.
1. Get the latest release from the Releases page and `tar xfz <tar-file>` into a folder.
1. Rename `config.json.sample -> config.json` and update to the correct address to your MQTT broker.
1. Run the binary.

## Prerequisit
You need to know the key and id of the Tuya device. According to me the easiest way to find these is explained at: [Step by Step for adding Tuya-bulbs](https://community.openhab.org/t/step-by-step-guide-for-adding-tuya-bulbs-wi-fi-smart-led-smart-life-app-to-oh2-using-tuya-mqtt-js-by-agentk/59371).

## Logging
The program uses [env_logger](https://docs.rs/env_logger) and can be configured to log at different levels with the `RUST_LOG=level` variable. It is also possible to turn on logging only for the rust-tuya components with `TUYA_LOG=level`. By default the id and key will be scrambled in the log output. To get the full id and key information set `TUYA_FULL_DISPLAY=true`.

## Start rust-tuya-mqtt when the computer starts
Running on a system that launches services with systemd this is a possible way of launching setting up the application as a service. This example-service runs alongside an openhab server, thus the Wants=, After=, and User= lines. The WorkingDirectory is where `rust-tuya-mqtt` looks for it configuration files and may be different from the actual binary.

Content of rust-tuya-mqtt.service:
```
#!/bin/sh -
[Unit]
Description=rust-tuya-mqtt
Wants=openhab.service
After=openhab.service

[Service]
ExecStart=/etc/openhab/scripts/rust-tuya-mqtt
Restart=always
RestartSec=10
User=openhab
Group=openhab
Environment=TUYA_LOG=debug
WorkingDirectory=/etc/openhab/scripts/
StandardOutput=append:/var/log/rust-tuya-mqtt.log
StandardError=append:/var/log/rust-tuya-mqtt.log

[Install]
WantedBy=multi-user.target
```
This service can be placed in the `/usr/lib/systemd/system/` folder and when enabling it with `sudo systemctl enable rust-tuya-mqtt.service` it will be launched at system start.

## Configuring
The program is configured with a file called config.json.

### config.json
This is the config.json file. It should be placed in the working directory.
```
{
    "mqtt_id": "test-tuya", <-- optional, default is "rust-tuya-mqtt"
    "host": "192.168.1.14",
    "port": 1883,
    "topic": "tuya/",       <-- optional, default is "tuya/"
    "mqtt_user": "",        <-- provide user and pass to...
    "mqtt_pass": "",        <-- ...login to secure tuya broker
    "qos": 0                <-- valid values are 0, 1 or 2
}
```
## Topics
There are two ways to design mqtt topics for communication with `rust-tuya-mqtt`. Either the topic can contain all the information needed to identify and communicate with the tuya compatble device. I this case the topic looks like this:

`tuya/ver3.[1|3]/<tuya-id>/<tuya-key>/<tuya-ip>/[command|state]`.

The other way to configure is to keep the device specific configuration in a file called `devices.json` and be placed in the working directory.

### devices.json
The devices.json is an optional configuration file that may contain information about the devices. This is a possible way of providing the necessary information for two devices:
```
[
    {
        "name": "my_awesome_device",
        "id": "<tuya_device_id>",
        "ip": "192.168.1.5",
        "key": "<tuya_device_key>",
        "version": "3.3"
    },
    {
        "name": "my_other_awesome_device",
        "id": "<tuya_device_id>",
        "ip": "192.168.1.6",
        "key": "<tuya_device_key>",
        "version": "3.1"
    }
]
```

The topic may look like `tuya/my_awesome_device/[command|state]`. The two methods of configuring, by topic or in configuration file, can be mixed.
