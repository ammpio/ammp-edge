use anyhow::Result;

use crate::helpers::now_iso;
use crate::interfaces::{get_legacy_config};
use crate::interfaces::keys;
use crate::interfaces::kvstore;
use crate::node_mgmt::{activate, generate_node_id};

pub fn init() -> Result<()> {
    if let Ok(Some(node_id)) = kvstore::get::<_, String>(keys::NODE_ID) {
        log::info!("Node ID: {node_id}");
        return Ok(());
    }

    match get_legacy_config() {
        Ok(Some(config)) => {
            log::info!("Legacy config found: {:?}; migrating...", config);
            // TODO: Migrate config
            return Ok(());
        },
        _ => log::info!("Legacy config not found"),
    }

    let node_id = generate_node_id();
    log::info!("Node ID is {}", node_id);

    let access_key = activate(&node_id)?;
    kvstore::set_many(
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
