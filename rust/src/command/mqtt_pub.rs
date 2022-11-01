use std::env;
use std::time::Duration;

use anyhow::Result;
use sysinfo::{System, SystemExt};

use crate::envvars::SNAP_REVISION;
use crate::helpers::{backoff_retry, get_node_arch, get_ssh_fingerprint, now_epoch};
use crate::interfaces::mqtt::{publish_msgs, MqttMessage};

const PUBLISH_TIMEOUT: Duration = Duration::from_secs(30);

fn construct_meta_msg() -> Vec<MqttMessage> {
    let mut msgs = vec![
        MqttMessage {
            topic: "u/meta/boot_time".into(),
            payload: System::new().boot_time().to_string(),
        },
        MqttMessage {
            topic: "u/meta/start_time".into(),
            payload: now_epoch().to_string(),
        },
    ];

    if let Ok(snap_revision) = env::var(SNAP_REVISION) {
        msgs.push(MqttMessage {
            topic: "u/meta/snap_rev".into(),
            payload: snap_revision,
        })
    }
    if let Ok(arch) = get_node_arch() {
        msgs.push(MqttMessage {
            topic: "u/meta/arch".into(),
            payload: arch,
        })
    }
    if let Ok(ssh_fingerprint) = get_ssh_fingerprint() {
        msgs.push(MqttMessage {
            topic: "u/meta/ssh_fingerprint".into(),
            payload: ssh_fingerprint,
        });
    }
    msgs
}

pub fn mqtt_pub_meta() -> Result<()> {
    let messages = construct_meta_msg();
    log::info!("Publishing metadata: {:?}", messages);

    let publish_msgs = || {
        publish_msgs(&messages, Some("local-pub-meta".into())).map_err(backoff::Error::transient)
    };

    match backoff_retry(publish_msgs, Some(PUBLISH_TIMEOUT)) {
        Ok(()) => log::info!("Successfully published"),
        Err(e) => log::error!("Error while publishing to MQTT: {e}"),
    }
    Ok(())
}
