use thiserror::Error;

use crate::{
    constants::topics,
    interfaces::mqtt::{self, MqttMessage},
};

use super::{models::DeviceReading, payload::payloads_from_device_readings};

#[derive(Error, Debug)]
pub enum PublishError {
    #[error("MQTT error: {0}")]
    MqttError(#[from] mqtt::MqttError),
}

pub fn publish_readings(readings: Vec<DeviceReading>) -> anyhow::Result<(), PublishError> {
    let messages = construct_payloads(readings);
    log::trace!("Publishing messages: {:?}", &messages);

    // mqtt::publish_msgs(&vec![MqttMessage::new("hello", "there")], Some("local-pub-data"), false)?;
    mqtt::publish_msgs(&messages, Some("local-pub-data"), false)?;

    Ok(())
}

fn construct_payloads(readings: Vec<DeviceReading>) -> Vec<MqttMessage> {
    payloads_from_device_readings(readings)
        .into_iter()
        .map(|p| MqttMessage::new(topics::DATA, serde_json::to_string(&p).unwrap()))
        .collect()
}
