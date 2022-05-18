use std::path::Path;

use anyhow::Result;
use kvstore::KVDb;

use crate::helpers::base_path;
use crate::helpers::now_iso;
use crate::interfaces::get_legacy_config;
use crate::interfaces::keys;
use crate::interfaces::kvpath;
use crate::node_mgmt::{activate, generate_node_id};


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

fn initialize(kvs: &KVDb, node_id: &str) -> Result<()> {
    log::info!("Node ID: {}. Initializing...", node_id);

    let access_key = activate(kvs, node_id)?;
    kvs.set(keys::NODE_ID, &node_id)?;
    kvs.set(keys::ACCESS_KEY, &access_key)?;
    kvs.set(keys::ACTIVATED, &now_iso())?;
    log::info!("Activation successfully completed");
    Ok(())
}

pub fn init() -> Result<()> {
    let kvs = KVDb::new(kvpath::sqlite_store())?;

    if is_already_initialized(&kvs)? {
        return Ok(());
    }

    let legacy_config_path = &format!("{}/config.db", base_path::data_dir());
    if can_import_legacy_config(legacy_config_path, &kvs)? {
        return Ok(());
    }

    let node_id = generate_node_id();
    initialize(&kvs, &node_id)
}
