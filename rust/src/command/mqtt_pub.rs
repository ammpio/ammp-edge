use std::env;
use std::thread::sleep;
use std::time::Duration;

use anyhow::Result;
use backoff::{retry_notify, Error, ExponentialBackoff};
use sysinfo::{System, SystemExt};

use crate::envvars::{SNAP_ARCH, SNAP_REVISION};
use crate::helpers::{get_ssh_fingerprint, now_epoch};
use crate::interfaces::mqtt::{publish_msgs, MqttMessage};

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
    if let Ok(arch) = env::var(SNAP_ARCH) {
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
    sleep(Duration::from_secs(2));
    let res = publish_msgs(&messages, Some(false), Some("local-pub-meta".into()));
    if let Err(e) = res {
        log::error!(
            "Error while publishing to MQTT: {e}\nMessages: {:?}",
            messages
        );
    }
    // The command will log and ignore errors, and always return a success exit code.
    // This is a temporary workaround since snapd will try to run this by itself, without Mosquitto running
    // See https://forum.snapcraft.io/t/bug-refreshing-snap-with-new-service-doesnt-respect-dependencies/31890
    Ok(())
}

#[allow(dead_code)]
pub fn mqtt_pub_meta_persistent() -> Result<()> {
    let messages = construct_meta_msg();

    let publish_msgs = || {
        publish_msgs(&messages, Some(true), Some("local-pub-meta".into())).map_err(Error::transient)
    };

    let notify = |err, dur: Duration| {
        log::error!(
            "MQTT publish error after {:.1}s: {}",
            dur.as_secs_f32(),
            err
        );
    };

    retry_notify(ExponentialBackoff::default(), publish_msgs, notify).unwrap();
    Ok(())
}
