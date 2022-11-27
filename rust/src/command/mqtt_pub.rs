use std::env;
use std::time::Duration;

use anyhow::Result;
use sysinfo::{System, SystemExt};

use crate::constants::{envvars, topics};
use crate::helpers;
use crate::interfaces::mqtt;
use crate::interfaces::mqtt::MqttMessage;

const PUBLISH_TIMEOUT: Duration = Duration::from_secs(30);

fn construct_meta_msg() -> Vec<MqttMessage> {
    let mut msgs = vec![
        MqttMessage::new(
            topics::META_BOOT_TIME,
            System::new().boot_time().to_string(),
        ),
        MqttMessage::new(topics::META_START_TIME, helpers::now_epoch().to_string()),
    ];

    if let Ok(snap_revision) = env::var(envvars::SNAP_REVISION) {
        msgs.push(MqttMessage::new(topics::META_SNAP_REV, snap_revision));
    }
    if let Ok(arch) = helpers::get_node_arch() {
        msgs.push(MqttMessage::new(topics::META_ARCH, arch));
    }
    if let Ok(ssh_fingerprint) = helpers::get_ssh_fingerprint() {
        msgs.push(MqttMessage::new(
            topics::META_SSH_FINGERPRINT,
            ssh_fingerprint,
        ));
    }
    msgs
}

fn construct_clean_msg(original_msg: &[MqttMessage]) -> Vec<MqttMessage> {
    original_msg
        .iter()
        .map(|m| MqttMessage::new(&m.topic, ""))
        .collect()
}

pub fn mqtt_pub_meta() -> Result<()> {
    let messages = construct_meta_msg();
    let clean_messages = construct_clean_msg(&messages);
    log::info!("Publishing metadata: {:?}", messages);

    let publish_msgs = || {
        mqtt::publish_msgs(&clean_messages, Some("local-pub-meta"), true)?;
        mqtt::publish_msgs(&messages, Some("local-pub-meta"), false)
            .map_err(backoff::Error::transient)
    };

    match helpers::backoff_retry(publish_msgs, Some(PUBLISH_TIMEOUT)) {
        Ok(()) => log::info!("Successfully published"),
        Err(e) => log::error!("Error while publishing to MQTT: {e}"),
    }
    Ok(())
}
