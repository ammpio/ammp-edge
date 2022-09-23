use std::env;
use std::time::Duration;

use anyhow::Result;
use backoff::{retry_notify, Error, ExponentialBackoff};
use sys_info::boottime;

use crate::envvars::SNAP_REVISION;
use crate::helpers::get_ssh_fingerprint;
use crate::interfaces::mqtt::{self, MqttMessage};

fn construct_meta_msg() -> Vec<MqttMessage> {
    let mut msgs = vec![
        MqttMessage {
            topic: "u/meta/snap_rev".into(),
            payload: env::var(SNAP_REVISION).unwrap_or_else(|_| "N/A".into()),
        },
        MqttMessage {
            topic: "u/meta/boot_time".into(),
            payload: boottime().unwrap().tv_sec.to_string(),
        }
    ];
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

    let publish_msgs = || {
        mqtt::publish_msgs(&messages, Some(true), Some("local-pub-meta".into()))
            .map_err(Error::transient)
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
