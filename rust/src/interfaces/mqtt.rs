use std::env;
use std::str::{from_utf8, Utf8Error};

use getrandom::getrandom;
use once_cell::sync::Lazy;
use rumqttc::{Client, Connection, Event, MqttOptions, Packet, QoS};
use thiserror::Error;

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

#[derive(Debug)]
pub struct MqttMessage {
    pub topic: String,
    pub payload: String,
}

#[derive(Error, Debug)]
pub enum MqttError {
    #[error(transparent)]
    Utf8(#[from] Utf8Error),
    #[error(transparent)]
    MqttClient(#[from] rumqttc::ClientError),
    #[error(transparent)]
    MqttConnection(#[from] rumqttc::ConnectionError),
}

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

#[allow(dead_code)]
pub fn publish(
    mut client: Client,
    msg: MqttMessage,
    retain: Option<bool>,
    qos: Option<QoS>,
) -> Result<(), MqttError> {
    log::debug!("Publishing to {}: {}", msg.topic, msg.payload);

    client.publish(
        msg.topic,
        qos.unwrap_or(QoS::AtLeastOnce),
        retain.unwrap_or(false),
        msg.payload.as_bytes(),
    )?;
    Ok(())
}

pub fn publish_msgs(
    messages: &Vec<MqttMessage>,
    retain: Option<bool>,
    client_prefix: Option<String>,
) -> Result<(), MqttError> {
    let (mut client, mut connection) = client_conn(get_rand_client_id(client_prefix), None);

    let mut expected_msg_acks = messages.len();

    for msg in messages.iter() {
        log::debug!("Publishing to {}: {}", msg.topic, msg.payload);

        client.publish(
            msg.topic.clone(),
            QoS::AtLeastOnce,
            retain.unwrap_or(false),
            msg.payload.as_bytes(),
        )?;
    }

    for (_, notification) in connection.iter().enumerate() {
        log::debug!("Notification = {:?}", notification);
        match notification {
            Ok(Event::Incoming(Packet::PubAck(_))) => expected_msg_acks -= 1,
            Err(e) => return Err(e.into()),
            _ => (),
        }
        if expected_msg_acks == 0 {
            break;
        }
    }
    client.disconnect()?;
    Ok(())
}

pub fn sub_topics<F>(
    topics: &[String],
    client_prefix: Option<String>,
    func: F,
) -> Result<(), MqttError>
where
    F: Fn(MqttMessage),
{
    let (mut client, mut connection) = client_conn(get_rand_client_id(client_prefix), None);

    for topic in topics.iter() {
        log::info!("Subscribing to {}", topic);
        client.subscribe(topic, QoS::ExactlyOnce)?;
    }

    for (_, notification) in connection.iter().enumerate() {
        log::trace!("Notification = {:?}", notification);
        match notification {
            Ok(Event::Incoming(Packet::Publish(r))) => {
                let msg = MqttMessage {
                    topic: r.topic,
                    payload: from_utf8(&r.payload)?.into(),
                };
                func(msg);
            }
            Err(e) => return Err(e.into()),
            _ => (),
        }
    }
    client.disconnect()?;
    Ok(())
}
