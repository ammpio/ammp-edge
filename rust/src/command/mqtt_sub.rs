use std::thread;
use std::time::Duration;

use flume;
use kvstore::KVDb;

use crate::constants::defaults::DB_WRITE_TIMEOUT;
use crate::constants::topics;
use crate::helpers;
use crate::interfaces::{kvpath, mqtt, mqtt::MqttMessage};
use crate::node_mgmt;

fn try_set_config(config_payload: &str) {
    match node_mgmt::config::from_str(config_payload) {
        Ok(config) => {
            // A databse connection or write error is transient and would lead to a retry
            let set_config = || {
                let kvs = KVDb::new(kvpath::SQLITE_STORE.as_path())?;
                node_mgmt::config::set(kvs, &config)?;
                Ok(())
            };

            match helpers::backoff_retry(set_config, Some(DB_WRITE_TIMEOUT)) {
                Ok(()) => log::info!("Successfully set new config"),
                Err(e) => log::error!("Permanent error setting config: {:?}", e),
            }
        }
        Err(e) => {
            log::error!("Error \"{e}\" while trying to parse received payload:\n{config_payload}");
        }
    }
}

fn run_commands(command_payload: &str) {
    match serde_json::from_str::<Vec<String>>(command_payload) {
        Ok(commands) => {
            for cmd in commands {
                let response = helpers::run_command(&cmd);
                if let Err(e) = mqtt::publish_msgs(
                    &vec![MqttMessage::new(topics::COMMAND_RESPONSE, response)],
                    Some("local-pub-cmd-resp"),
                    false,
                ) {
                    log::error!("Could not publish command response; error: {e}");
                }
                thread::sleep(Duration::from_secs(5));
            }
        }
        Err(e) => log::error!("Could not parse payload as JSON list; error: {e}"),
    }
}

fn process_msg(msg: &MqttMessage) {
    log::info!("Received {} on {}", msg.payload, msg.topic);
    match msg.topic.as_str() {
        topics::CONFIG => try_set_config(&msg.payload),
        topics::COMMAND => run_commands(&msg.payload),
        _ => log::warn!("Message received on unrecognized topic {}", msg.topic),
    }
}

pub fn mqtt_sub_cfg_cmd() -> anyhow::Result<()> {
    let sub_loop = || {
        let (tx, rx) = flume::unbounded();

        thread::spawn(move || {
            for msg in rx.iter() {
                process_msg(&msg);
            }
        });

        mqtt::sub_topics(
            &[topics::CONFIG, topics::COMMAND],
            Some("local-sub-cfg"),
            tx,
            None,
        )
        .map_err(backoff::Error::transient)
    };
    helpers::backoff_retry(sub_loop, None)?;
    Ok(())
}
