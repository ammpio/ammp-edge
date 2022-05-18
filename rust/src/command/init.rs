use std::path::Path;

use anyhow::Result;
use kvstore::KVDb;

use crate::constants::keys;
use crate::helpers::{base_path, now_iso};
use crate::interfaces::{get_legacy_config, kvpath};
use crate::node_mgmt;


fn is_already_initialized(kvs: &KVDb) -> Result<bool> {
    if let Some(node_id) = kvs.get::<String>(keys::NODE_ID)? {
        log::info!("Node ID: {node_id}");
        Ok(true)
    } else {
        Ok(false)
    }
}

fn can_import_legacy_config(legacy_config_path: impl AsRef<Path>, kvs: &KVDb) -> Result<bool> {
    match get_legacy_config(legacy_config_path) {
        Ok(Some(legacy_conf)) => {
            log::info!("Legacy config found: {:?}; migrating...", legacy_conf);
            kvs.set(keys::NODE_ID, legacy_conf.node_id)?;
            kvs.set(keys::ACCESS_KEY, legacy_conf.access_key)?;
            kvs.set(keys::CONFIG, legacy_conf.config)?;
            Ok(true)
        }
        _ => {
            log::info!("Legacy config not found");
            Ok(false)
        }
    }
}

fn do_fresh_initialization(kvs: &KVDb) -> Result<()> {
    let node_id = node_mgmt::generate_node_id();
    log::info!("Node ID: {}. Initializing...", node_id);

    let access_key = node_mgmt::activate(kvs, &node_id)?;
    kvs.set(keys::NODE_ID, &node_id)?;
    kvs.set(keys::ACCESS_KEY, &access_key)?;
    kvs.set(keys::ACTIVATED, &now_iso())?;
    log::info!("Activation successfully completed");
    Ok(())
}

pub fn init() -> Result<()> {
    let kvs = KVDb::new(kvpath::SQLITE_STORE.as_path())?;

    if is_already_initialized(&kvs)? {
        return Ok(());
    }

    let legacy_config_path = base_path::DATA_DIR.join("config.db");
    if can_import_legacy_config(legacy_config_path, &kvs)? {
        return Ok(());
    }

    do_fresh_initialization(&kvs)
}
