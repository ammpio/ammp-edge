use std::env;
use std::time::Duration;

use anyhow::Result;
use sysinfo::{System, SystemExt};

use crate::constants::topics;
use crate::envvars::SNAP_REVISION;
use crate::helpers::{backoff_retry, get_node_arch, get_ssh_fingerprint, now_epoch};
use crate::interfaces::mqtt::{publish_msgs, MqttMessage};

const PUBLISH_TIMEOUT: Duration = Duration::from_secs(30);

fn construct_meta_msg() -> Vec<MqttMessage> {
    let mut msgs = vec![
        MqttMessage {
            topic: topics::META_BOOT_TIME.to_string(),
            payload: System::new().boot_time().to_string(),
        },
        MqttMessage {
            topic: topics::META_START_TIME.to_string(),
            payload: now_epoch().to_string(),
        },
    ];

    if let Ok(snap_revision) = env::var(SNAP_REVISION) {
        msgs.push(MqttMessage {
            topic: topics::META_SNAP_REV.to_string(),
            payload: snap_revision,
        })
    }
    if let Ok(arch) = get_node_arch() {
        msgs.push(MqttMessage {
            topic: topics::META_ARCH.to_string(),
            payload: arch,
        })
    }
    if let Ok(ssh_fingerprint) = get_ssh_fingerprint() {
        msgs.push(MqttMessage {
            topic: topics::META_SSH_FINGERPRINT.to_string(),
            payload: ssh_fingerprint,
        });
    }
    msgs
}

fn construct_clean_msg(original_msg: &[MqttMessage]) -> Vec<MqttMessage> {
    original_msg
        .iter()
        .map(|m| MqttMessage {
            topic: m.topic.clone(),
            payload: "".to_string(),
        })
        .collect()
}

pub fn mqtt_pub_meta() -> Result<()> {
    let messages = construct_meta_msg();
    let clean_messages = construct_clean_msg(&messages);
    log::info!("Publishing metadata: {:?}", messages);

    let publish_msgs = || {
        publish_msgs(&clean_messages, Some("local-pub-meta".into()), true)?;
        publish_msgs(&messages, Some("local-pub-meta".into()), false).map_err(backoff::Error::transient)
    };

    match backoff_retry(publish_msgs, Some(PUBLISH_TIMEOUT)) {
        Ok(()) => log::info!("Successfully published"),
        Err(e) => log::error!("Error while publishing to MQTT: {e}"),
    }
    Ok(())
}
