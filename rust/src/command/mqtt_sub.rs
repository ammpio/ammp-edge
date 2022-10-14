use anyhow::Result;

use crate::interfaces::mqtt::{sub_topics, MqttMessage};

fn process_msg(msg: MqttMessage) {
    log::debug!("Received {:?}", msg);
}

pub fn mqtt_sub_cfg() -> Result<()> {
    sub_topics(
        &["d/config".into(), "d/command".into()],
        Some("local-sub-cfg".into()),
        process_msg,
    )
}
