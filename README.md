# rust-tuya-mqtt
![Build status](https://github.com/EmilSodergren/rust-tuya-mqtt/workflows/Mean%20Bean%20CI/badge.svg)

Rust program that enables controlling of Tuya/Smart Life devices via MQTT. It uses the rust-tuyapi library to talk to the devices.

## Prerequisit
You need to know the key and id of the Tuya device. According to me the easiest way to find these is explained at: [Step by Step for adding Tuya-bulbs](https://community.openhab.org/t/step-by-step-guide-for-adding-tuya-bulbs-wi-fi-smart-led-smart-life-app-to-oh2-using-tuya-mqtt-js-by-agentk/59371).

The topic should look like 'tuya/ver3.[1|3]/\<tuya-id>/\<tuya-key>/\<tuya-ip>/command'. Command can be ON/OFF or 1/0.

