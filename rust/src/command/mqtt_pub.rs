use anyhow::Result;

use crate::interfaces::mqtt::{self, MqttMessage};

pub fn mqtt_pub_meta() -> Result<()> {
    let messages = vec!(
        MqttMessage {
            topic: "u/meta/snap1".into(),
            payload: "abc".into(),
        },
        MqttMessage {
            topic: "u/meta/snap2".into(),
            payload: "abc1".into(),
        },
        MqttMessage {
            topic: "u/meta/snap3".into(),
            payload: "abc2".into(),
        },
    );
    mqtt::publish_msgs(messages, Some(true))
}
