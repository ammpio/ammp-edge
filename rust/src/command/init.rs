use std::path::Path;

use anyhow::Result;
use kvstore::KVDb;

use crate::constants::keys;
use crate::helpers::{base_path, now_iso};
use crate::interfaces;
use crate::interfaces::kvpath;
use crate::node_mgmt;

const LEGACY_CONFIG_FILENAME: &str = "config.db";

fn is_already_initialized(kvs: &KVDb) -> Result<bool> {
    if let Some(node_id) = kvs.get::<String>(keys::NODE_ID)? {
        log::info!("Node ID: {node_id}");
        Ok(true)
    } else {
        Ok(false)
    }
}

fn can_import_legacy_config(legacy_config_path: impl AsRef<Path>, kvs: &KVDb) -> Result<bool> {
    match interfaces::legacy_config(legacy_config_path) {
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
    kvs.set(keys::NODE_ID, node_id)?;
    kvs.set(keys::ACCESS_KEY, access_key)?;
    kvs.set(keys::ACTIVATED, now_iso())?;
    log::info!("Activation successfully completed");
    Ok(())
}

pub fn init() -> Result<()> {
    let kvs = KVDb::new(kvpath::SQLITE_STORE.as_path())?;

    if is_already_initialized(&kvs)? {
        return Ok(());
    }

    let legacy_config_path = base_path::DATA_DIR.join(LEGACY_CONFIG_FILENAME);
    if can_import_legacy_config(legacy_config_path, &kvs)? {
        return Ok(());
    }

    do_fresh_initialization(&kvs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use once_cell::sync::Lazy;

    use rusqlite::{Connection, params};
    use serde_json::{Value, json};

    const IN_MEMORY: &str = ":memory:";
    const SAMPLE_NODE_ID: &str = "abcdef123456";
    const SAMPLE_ACCESS_KEY: &str = "secret";
    static SAMPLE_CONFIG: Lazy<Value> = Lazy::new(|| {
        json!({
            "devices": {"blah": "blah"},
            "readings": ["a", "b"],
            "timestamp": "2000-01-01T00:00:00Z"
        })
    });

    fn create_kvs_initialized(path: impl AsRef<Path>) -> Result<KVDb> {
        let kvs = KVDb::new(path)?;
        kvs.set(keys::NODE_ID, SAMPLE_NODE_ID)?;
        kvs.set(keys::ACCESS_KEY, SAMPLE_ACCESS_KEY)?;
        Ok(kvs)
    }

    fn create_legacy_configdb(path: impl AsRef<Path>) -> Result<()> {
        let conn = Connection::open(path.as_ref())?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS 'nodeconfig' (
            node_id TEXT PRIMARY KEY NOT NULL,
            access_key TEXT,
            config TEXT
            )",
            [],
        )?;

        let mut stmt = conn.prepare(
            "INSERT INTO 'nodeconfig' (node_id, access_key, config) values (?1, ?2, ?3)",
        )?;
        let res = stmt.execute(params![
            SAMPLE_NODE_ID.to_string(),
            SAMPLE_ACCESS_KEY.to_string(),
            SAMPLE_CONFIG.to_string()
        ])?;
        log::debug!("Inserted: {:?} row(s)", res);
        Ok(())
    }

    #[test]
    fn with_initialized_kvs() -> Result<()> {
        let initialized_kvs = create_kvs_initialized(IN_MEMORY)?;
        assert!(is_already_initialized(&initialized_kvs)?);
        Ok(())
    }

    #[test]
    fn without_initialized_kvs() -> Result<()> {
        let blank_kvs = KVDb::new(IN_MEMORY)?;
        assert!(!is_already_initialized(&blank_kvs)?);
        Ok(())
    }

    #[test]
    fn with_legacy_config() -> Result<()> {
        let blank_kvs = KVDb::new(IN_MEMORY)?;
        let tempdir = tempfile::tempdir()?;
        let legacy_configdb_path = tempdir.path().join(LEGACY_CONFIG_FILENAME);
        create_legacy_configdb(&legacy_configdb_path)?;
        assert!(can_import_legacy_config(&legacy_configdb_path, &blank_kvs)?);
        assert_eq!(
            blank_kvs.get::<String>(keys::NODE_ID)?.unwrap(),
            SAMPLE_NODE_ID
        );
        assert_eq!(
            blank_kvs.get::<String>(keys::ACCESS_KEY)?.unwrap(),
            SAMPLE_ACCESS_KEY
        );
        assert_eq!(
            blank_kvs.get::<Value>(keys::CONFIG)?.unwrap(),
            *SAMPLE_CONFIG
        );
        Ok(())
    }

    #[test]
    fn without_legacy_config() -> Result<()> {
        let blank_kvs = KVDb::new(IN_MEMORY)?;
        let tempdir = tempfile::tempdir()?;
        let legacy_configdb_path = tempdir.path().join(LEGACY_CONFIG_FILENAME);
        assert!(!can_import_legacy_config(legacy_configdb_path, &blank_kvs)?);
        Ok(())
    }

    #[test]
    fn fresh_initialization() -> Result<()> {
        let blank_kvs = KVDb::new(IN_MEMORY)?;
        // TODO
        blank_kvs.set(keys::NODE_ID, SAMPLE_NODE_ID)?;
        assert_eq!(
            blank_kvs.get::<String>(keys::NODE_ID)?.unwrap(),
            SAMPLE_NODE_ID
        );
        Ok(())
    }
}
