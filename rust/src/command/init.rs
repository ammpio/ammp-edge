use anyhow::Result;

use crate::interfaces::keys;
use crate::interfaces::kvstore;
use crate::node_mgmt::{activate, generate_node_id};

pub fn init() -> Result<()> {
    if let Ok(Some(node_id)) = kvstore::get::<_, String>(keys::NODE_ID) {
        log::info!("Node ID: {node_id}");
        return Ok(());
    }

    let node_id = generate_node_id();
    log::info!("Node ID is {}", node_id);

    let access_key = activate(&node_id)?;
    kvstore::set_many([(keys::NODE_ID, &node_id), (keys::ACCESS_KEY, &access_key)].to_vec())?;
    log::info!("Activation successfully completed");

    Ok(())
}
