use thiserror::Error;

use crate::{
    constants::topics,
    interfaces::mqtt::{self, MqttMessage, MqttPublisher},
};

use super::{
    models::DeviceReading,
    payload::{Metadata, payloads_from_device_readings},
};

#[derive(Error, Debug)]
pub enum PublishError {
    #[error("MQTT error: {0}")]
    MqttError(#[from] Box<mqtt::MqttError>),
}

pub fn publish_readings(
    readings: Vec<DeviceReading>,
    metadata: Option<Metadata>,
) -> anyhow::Result<(), PublishError> {
    let messages = construct_payloads(readings, metadata);
    log::trace!("Publishing messages: {:?}", &messages);

    mqtt::publish_msgs(&messages, Some("local-pub-data"), false).map_err(Box::new)?;

    Ok(())
}

pub async fn publish_readings_with_publisher(
    publisher: &mut MqttPublisher,
    readings: Vec<DeviceReading>,
    metadata: Option<Metadata>,
) -> anyhow::Result<(), PublishError> {
    let messages = construct_payloads(readings, metadata);

    log::info!("Publishing {} payloads to MQTT", messages.len());

    publisher
        .publish_msgs(&messages, false)
        .await
        .map_err(Box::new)?;

    Ok(())
}

fn construct_payloads(
    readings: Vec<DeviceReading>,
    metadata: Option<Metadata>,
) -> Vec<MqttMessage> {
    payloads_from_device_readings(readings, metadata)
        .into_iter()
        .map(|p| MqttMessage::new(topics::DATA, serde_json::to_string(&p).unwrap()))
        .collect()
}
