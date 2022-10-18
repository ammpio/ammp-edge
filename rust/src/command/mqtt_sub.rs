use std::time::Duration;

use anyhow::Result;
use backoff::{retry_notify, Error, ExponentialBackoff};
use kvstore::KVDb;

use crate::constants::topics;
use crate::interfaces::kvpath;
use crate::interfaces::mqtt::{sub_topics, MqttMessage};
use crate::node_mgmt;

fn try_set_config(config_payload: String) {
    // An invalid config would lead to a permanent error that is not retried
    // A databse connection or write error is transient and would lead toa retry
    // TODO: Set maximum number of retries, once https://github.com/ihrwein/backoff/pull/60 is merged 
    let set_config = || {
        let config = node_mgmt::config::from_string(&config_payload)
            .map_err(Into::into)
            .map_err(Error::permanent)?;
        let kvs = KVDb::new(kvpath::SQLITE_STORE.as_path()).map_err(Error::transient)?;
        node_mgmt::config::set(kvs, config).map_err(Error::transient)
    };

    let notify = |err, dur: Duration| {
        log::error!("Temporary error setting config, after {:.1}s: {}", dur.as_secs_f32(), err);
    };

    match retry_notify(ExponentialBackoff::default(), set_config, notify) {
        Ok(()) => log::info!("Successfully set new config"),
        Err(err) => log::error!("Permanent error setting config: {:?}", err),
    }
}

fn process_msg(msg: MqttMessage) {
    log::debug!("Received {} on {}", msg.payload, msg.topic);
    match msg.topic.as_str() {
        topics::CONFIG => try_set_config(msg.payload),
        topics::COMMAND => todo!("Set config"),
        _ => log::info!("Message received on unrecognized topic {}", msg.topic),
    }
}

pub fn mqtt_sub_cfg() -> Result<()> {
    sub_topics(
        &[topics::CONFIG.into(), topics::COMMAND.into()],
        Some("local-sub-cfg".into()),
        process_msg,
    )
}
