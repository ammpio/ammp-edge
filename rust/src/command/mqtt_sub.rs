use anyhow::Result;
use kvstore::KVDb;

use crate::constants::topics;
use crate::helpers::backoff_retry;
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
            .map_err(backoff::Error::permanent)?;
        let kvs = KVDb::new(kvpath::SQLITE_STORE.as_path()).map_err(backoff::Error::transient)?;
        node_mgmt::config::set(kvs, config).map_err(backoff::Error::transient)
    };

    match backoff_retry(set_config) {
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
