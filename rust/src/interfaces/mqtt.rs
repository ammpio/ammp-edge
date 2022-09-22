use std::env;

use anyhow::Result;
use getrandom::getrandom;
use once_cell::sync::Lazy;
use rumqttc::{Client, Connection, Event, MqttOptions, Packet, QoS};

use crate::constants::{defaults, envvars};

static MQTT_BRIDGE_HOST: Lazy<String> = Lazy::new(|| {
    if let Ok(host) = env::var(envvars::MQTT_BRIDGE_HOST) {
        return host;
    }
    defaults::MQTT_BRIDGE_HOST.to_string()
});

static MQTT_BRIDGE_PORT: Lazy<u16> = Lazy::new(|| {
    if let Ok(port_str) = env::var(envvars::MQTT_BRIDGE_PORT) &&
    let Ok(port) = port_str.parse::<u16>() {
        return port;
    }
    defaults::MQTT_BRIDGE_PORT
});

pub fn get_rand_client_id(prefix: Option<String>) -> String {
    const RAND_ID_BYTES: usize = 3;
    let mut rand = [0u8; RAND_ID_BYTES];
    getrandom(&mut rand).unwrap();
    let randhex = hex::encode(rand);

    if let Some(pref) = prefix {
        format!("{pref}-{randhex}")
    } else {
        randhex
    }
}

pub fn client_conn(client_id: String, clean_session: Option<bool>) -> (Client, Connection) {
    let host = MQTT_BRIDGE_HOST.clone();
    let port = *MQTT_BRIDGE_PORT;
    log::info!("Establishing MQTT connection to {host}:{port} as {client_id}");

    let mut mqttoptions = MqttOptions::new(client_id, host, port);
    mqttoptions.set_clean_session(clean_session.unwrap_or(true));

    Client::new(mqttoptions, 10)
}

pub fn publish(
    mut client: Client,
    topic: String,
    payload: Vec<u8>,
    retain: Option<bool>,
    qos: Option<QoS>,
) -> Result<()> {
    if let Ok(payload_str) = String::from_utf8(payload.clone()) {
        log::debug!("Publishing to {topic}: {payload_str}");
    } else {
        log::debug!("Publishing to {topic}");
    }
    client
        .publish(
            topic,
            qos.unwrap_or(QoS::AtLeastOnce),
            retain.unwrap_or(false),
            payload,
        )
        .map_err(Into::into)
}

pub fn publish_one(
    topic: String,
    payload: Vec<u8>,
    retain: Option<bool>,
    qos: Option<QoS>,
) -> Result<()> {
    let (client, mut connection) = client_conn(get_rand_client_id(None), None);
    publish(client, topic, payload, retain, qos)?;

    for (_, notification) in connection.iter().enumerate() {
        log::debug!("Notification = {:?}", notification);
        match notification {
            Ok(Event::Incoming(Packet::PubAck(_))) => break,
            Err(e) => return Err(e.into()),
            _ => continue,
        }
    }
    Ok(())
}
