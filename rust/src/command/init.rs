use anyhow::Result;
use kvstore::KVDb;

use crate::helpers::now_iso;
use crate::interfaces::get_legacy_config;
use crate::interfaces::keys;
use crate::interfaces::kvpath;
use crate::node_mgmt::{activate, generate_node_id};

pub fn init() -> Result<()> {
    let kvs = KVDb::new(kvpath::sqlite_store())?;

    if let Ok(Some(node_id)) = kvs.get::<String>(keys::NODE_ID) {
        log::info!("Node ID: {node_id}");
        return Ok(());
    }

    match get_legacy_config() {
        Ok(Some(lconf)) => {
            log::info!("Legacy config found: {:?}; migrating...", lconf);
            kvs.set(keys::NODE_ID, lconf.node_id)?;
            kvs.set(keys::ACCESS_KEY, lconf.access_key)?;
            kvs.set(keys::CONFIG, lconf.config)?;
            return Ok(());
        }
        _ => log::info!("Legacy config not found"),
    }

    let node_id = generate_node_id();
    log::info!("Node ID: {}. Initializing...", node_id);

    let access_key = activate(&kvs, &node_id)?;
    kvs.set(keys::NODE_ID, &node_id)?;
    kvs.set(keys::ACCESS_KEY, &access_key)?;
    kvs.set(keys::ACTIVATED, &now_iso())?;
    log::info!("Activation successfully completed");

    Ok(())
}
