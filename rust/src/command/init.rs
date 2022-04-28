use anyhow::Result;

use crate::helpers::now_iso;
use crate::interfaces::get_legacy_config;
use crate::interfaces::keys;
use crate::interfaces::KVStore;
use crate::node_mgmt::{activate, generate_node_id};

pub fn init() -> Result<()> {
    if let Ok(Some(node_id)) = KVStore::get::<_, String>(keys::NODE_ID) {
        log::info!("Node ID: {node_id}");
        return Ok(());
    }

    match get_legacy_config() {
        Ok(Some(lconf)) => {
            log::info!("Legacy config found: {:?}; migrating...", lconf);
            return KVStore::set_many(vec![
                (keys::NODE_ID, lconf.node_id),
                (keys::ACCESS_KEY, lconf.access_key),
            ]);
        }
        _ => log::info!("Legacy config not found"),
    }

    let node_id = generate_node_id();
    log::info!("Node ID: {}. Initializing...", node_id);

    let access_key = activate(&node_id)?;
    KVStore::set_many(
        [
            (keys::NODE_ID, &node_id),
            (keys::ACCESS_KEY, &access_key),
            (keys::ACTIVATED, &now_iso()),
        ]
        .to_vec(),
    )?;
    log::info!("Activation successfully completed");

    Ok(())
}
