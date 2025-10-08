use thiserror::Error;

use crate::{
    constants::topics,
    interfaces::mqtt::{self, MqttMessage, MqttPublisher},
};

use super::payload::DataPayload;

#[derive(Error, Debug)]
pub enum PublishError {
    #[error("MQTT error: {0}")]
    MqttError(#[from] Box<mqtt::MqttError>),
}

pub fn publish_readings(data_payloads: &[DataPayload]) -> anyhow::Result<(), PublishError> {
    let messages = construct_mqtt_messages(data_payloads);
    log::trace!("Publishing messages: {:?}", &messages);

    mqtt::publish_msgs(&messages, Some("local-pub-data"), false).map_err(Box::new)?;

    Ok(())
}

pub async fn publish_readings_with_publisher(
    publisher: &mut MqttPublisher,
    data_payloads: &[DataPayload],
) -> anyhow::Result<(), PublishError> {
    let messages = construct_mqtt_messages(data_payloads);

    log::info!("Publishing {} payloads to MQTT", messages.len());

    publisher
        .publish_msgs(&messages, false)
        .await
        .map_err(Box::new)?;

    Ok(())
}

fn construct_mqtt_messages(data_payloads: &[DataPayload]) -> Vec<MqttMessage> {
    data_payloads
        .iter()
        .map(|p| MqttMessage::new(topics::DATA, serde_json::to_string(&p).unwrap()))
        .collect()
}
