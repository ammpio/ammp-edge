use std::thread::sleep;
use std::time::Duration;

use kvstore::KVDb;

use crate::constants::defaults::DB_WRITE_TIMEOUT;
use crate::constants::topics;
use crate::helpers::{backoff_retry, run_command};
use crate::interfaces::kvpath;
use crate::interfaces::mqtt::{publish_msgs, sub_topics, MqttMessage};
use crate::node_mgmt;

fn try_set_config(config_payload: String) {
    if let Ok(config) = node_mgmt::config::from_string(&config_payload) {
        // A databse connection or write error is transient and would lead to a retry
        let set_config = || {
            let kvs = KVDb::new(kvpath::SQLITE_STORE.as_path())?;
            node_mgmt::config::set(kvs, config.clone())?;
            Ok(())
        };

        match backoff_retry(set_config, Some(DB_WRITE_TIMEOUT)) {
            Ok(()) => log::info!("Successfully set new config"),
            Err(e) => log::error!("Permanent error setting config: {:?}", e),
        }
    } else {
        log::error!(
            "Could not parse received payload as valid config: {:?}",
            &config_payload
        );
    }
}

fn run_commands(command_payload: String) {
    match serde_json::from_str::<Vec<String>>(&command_payload) {
        Ok(commands) => {
            for cmd in commands {
                let response = run_command(cmd);
                if let Err(e) = publish_msgs(
                    &vec![MqttMessage {
                        topic: topics::COMMAND_RESPONSE.into(),
                        payload: response,
                    }],
                    Some("local-pub-cmd-resp".into()),
                ) {
                    log::error!("Could not publish command response; error: {e}");
                }
                sleep(Duration::from_secs(5));
            }
        }
        Err(e) => log::error!("Could not parse payload as JSON list; error: {e}"),
    }
}

fn process_msg(msg: MqttMessage) {
    log::info!("Received {} on {}", msg.payload, msg.topic);
    match msg.topic.as_str() {
        topics::CONFIG => try_set_config(msg.payload),
        topics::COMMAND => run_commands(msg.payload),
        _ => log::warn!("Message received on unrecognized topic {}", msg.topic),
    }
}

pub fn mqtt_sub_cfg_cmd() -> anyhow::Result<()> {
    let sub_loop = || {
        sub_topics(
            &[topics::CONFIG.into(), topics::COMMAND.into()],
            Some("local-sub-cfg".into()),
            process_msg,
        )
        .map_err(backoff::Error::transient)
    };
    backoff_retry(sub_loop, None)?;
    Ok(())
}
