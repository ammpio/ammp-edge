use std::env;
use std::str;
use std::str::Utf8Error;

use flume::Sender;
use once_cell::sync::Lazy;
use rumqttc::{Client, Connection, Event, MqttOptions, Packet, QoS};
use thiserror::Error;

use crate::constants::topics;
use crate::constants::{defaults, envvars};
use crate::helpers;

const MAX_PACKET_SIZE: usize = 16777216; // 16 MB
const MQTT_QUEUE_CAPACITY: usize = 10;

static MQTT_BRIDGE_HOST: Lazy<String> = Lazy::new(|| {
    if let Ok(host) = env::var(envvars::MQTT_BRIDGE_HOST) {
        return host;
    }
    defaults::MQTT_BRIDGE_HOST.to_string()
});

static MQTT_BRIDGE_PORT: Lazy<u16> = Lazy::new(|| {
    if let Ok(port_str) = env::var(envvars::MQTT_BRIDGE_PORT)
        && let Ok(port) = port_str.parse::<u16>()
    {
        return port;
    }
    defaults::MQTT_BRIDGE_PORT
});

#[derive(Debug, Clone, PartialEq)]
pub struct MqttMessage {
    pub topic: String,
    pub payload: String,
}

impl MqttMessage {
    pub fn new<S: Into<String>, T: Into<String>>(topic: S, payload: T) -> MqttMessage {
        MqttMessage {
            topic: topic.into(),
            payload: payload.into(),
        }
    }
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

pub fn rand_client_id(prefix: Option<&str>) -> String {
    let randhex = helpers::rand_hex(3);

    if let Some(pref) = prefix {
        format!("{pref}-{randhex}")
    } else {
        randhex
    }
}

pub fn client_conn(client_id: &str, clean_session: bool) -> (Client, Connection) {
    let host = MQTT_BRIDGE_HOST.clone();
    let port = *MQTT_BRIDGE_PORT;
    log::info!("Establishing MQTT connection to {host}:{port} as {client_id}");

    let mut mqttoptions = MqttOptions::new(client_id, host, port);
    mqttoptions.set_clean_session(clean_session);
    mqttoptions.set_max_packet_size(MAX_PACKET_SIZE, MAX_PACKET_SIZE);

    Client::new(mqttoptions, MQTT_QUEUE_CAPACITY)
}

pub fn publish_msgs(
    messages: &[MqttMessage],
    client_prefix: Option<&str>,
    retain: bool,
) -> Result<(), MqttError> {
    let (client, mut connection) = client_conn(&rand_client_id(client_prefix), true);

    for msg_batch in messages.chunks(MQTT_QUEUE_CAPACITY) {
        let mut expected_msg_acks = msg_batch.len();
        log::debug!("Publishing batch of {} messages", msg_batch.len());

        for msg in msg_batch.iter() {
            log::debug!("Publishing to {}: {}", msg.topic, msg.payload);

            client.publish(
                msg.topic.clone(),
                QoS::AtLeastOnce,
                retain,
                msg.payload.as_bytes(),
            )?;
        }

        for notification in connection.iter() {
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
    }
    client.disconnect()?;
    Ok(())
}

pub async fn publish_msgs_async(
    messages: &[MqttMessage],
    client_prefix: Option<&str>,
    retain: bool,
) -> Result<(), MqttError> {
    let (client, mut connection) = client_conn(&rand_client_id(client_prefix), true);

    for msg_batch in messages.chunks(MQTT_QUEUE_CAPACITY) {
        let mut expected_msg_acks = msg_batch.len();
        log::debug!("Publishing batch of {} messages", msg_batch.len());

        for msg in msg_batch.iter() {
            log::debug!("Publishing to {}: {}", msg.topic, msg.payload);

            client.publish(
                msg.topic.clone(),
                QoS::AtLeastOnce,
                retain,
                msg.payload.as_bytes(),
            )?;
        }

        while expected_msg_acks > 0 {
            let notification = connection.eventloop.poll().await;
            log::debug!("Notification = {:?}", notification);
            match notification {
                Ok(Event::Incoming(Packet::PubAck(_))) => expected_msg_acks -= 1,
                Err(e) => return Err(e.into()),
                _ => (),
            }
        }
    }
    client.disconnect()?;

    // Drop the connection in a spawn_blocking to avoid runtime-in-runtime issues
    tokio::task::spawn_blocking(move || {
        drop(connection);
    })
    .await
    .map_err(|e| {
        MqttError::MqttConnection(rumqttc::ConnectionError::Io(std::io::Error::other(e)))
    })?;

    Ok(())
}

pub fn publish_log_msg(log_msg: &str) -> Result<(), MqttError> {
    publish_msgs(
        &[MqttMessage {
            topic: topics::LOG_MSG.into(),
            payload: log_msg.into(),
        }],
        "local-pub-log-msg".into(),
        false,
    )
}

pub fn sub_topics(
    topics: &[&str],
    client_prefix: Option<&str>,
    tx: Sender<MqttMessage>,
    max_messages: Option<usize>,
) -> Result<(), MqttError> {
    let (client, mut connection) = client_conn(&rand_client_id(client_prefix), true);

    for topic in topics.iter() {
        log::info!("Subscribing to {}", topic);
        client.subscribe(*topic, QoS::ExactlyOnce)?;
    }

    let mut num_messages: usize = 0;

    for notification in connection.iter() {
        log::trace!("Notification = {:?}", notification);
        match notification {
            Ok(Event::Incoming(Packet::Publish(r))) => {
                let msg = MqttMessage::new(&r.topic, str::from_utf8(&r.payload)?);
                if let Err(e) = tx.send(msg) {
                    log::error!("Failed to submit message for processing: {e}");
                }

                num_messages += 1;
                if let Some(mm) = max_messages
                    && num_messages == mm
                {
                    break;
                }
            }
            Err(e) => return Err(e.into()),
            _ => (),
        }
    }
    client.disconnect()?;
    Ok(())
}

#[cfg(test)]
mod test {
    use std::thread;
    use std::time::Duration;

    use flume::unbounded;

    use super::*;

    const CLIENT_PREFIX: &str = "test_client";
    const SAMPLE_TOPICS: [&str; 3] = ["test_topic_1", "test_topic_2", "test_topic_3"];
    static SAMPLE_MQTT_MESSAGES: Lazy<Vec<MqttMessage>> = Lazy::new(|| {
        SAMPLE_TOPICS
            .iter()
            .map(|topic| MqttMessage::new(*topic, helpers::rand_hex(6)))
            .collect()
    });

    #[test]
    fn test_rand_client_id() {
        let bare_client_id = rand_client_id(None);
        assert_eq!(bare_client_id.len(), 6);

        let prefixed_client_id = rand_client_id(Some(CLIENT_PREFIX));
        assert_eq!(prefixed_client_id.len(), CLIENT_PREFIX.len() + 1 + 6);
    }

    #[test]
    fn test_publist_and_receive_msgs() {
        assert!(!SAMPLE_MQTT_MESSAGES.is_empty());

        let (tx, rx) = unbounded();

        let publisher_thread = thread::spawn(move || {
            thread::sleep(Duration::from_millis(200));
            publish_msgs(&SAMPLE_MQTT_MESSAGES, Some("sub-test-publisher"), false).unwrap();
        });

        sub_topics(
            &SAMPLE_TOPICS,
            Some("sub-test"),
            tx,
            Some(SAMPLE_MQTT_MESSAGES.len()),
        )
        .unwrap();

        for msg in &*SAMPLE_MQTT_MESSAGES {
            assert_eq!(rx.recv().unwrap(), *msg);
        }
        publisher_thread.join().unwrap();
    }
}
