use std::env;

use anyhow::Result;
use sys_info::boottime;

use crate::envvars::SNAP_REVISION;
use crate::interfaces::mqtt::{self, MqttMessage};

pub fn mqtt_pub_meta() -> Result<()> {
    let messages = vec!(
        MqttMessage {
            topic: "u/meta/snap_rev".into(),
            payload: env::var(SNAP_REVISION).unwrap_or_else(|_| "N/A".into()),
        },
        MqttMessage {
            topic: "u/meta/boottime".into(),
            payload: boottime().unwrap().tv_sec.to_string(),
        },
    );
    mqtt::publish_msgs(messages, Some(true))
}
