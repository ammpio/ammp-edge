use anyhow::Result;

use crate::node_mgmt::{activate, generate_node_id};
use crate::interfaces::kvstore;
use crate::interfaces::keys;


pub fn init() -> Result<()> {
    let node_id = generate_node_id();
    log::info!("Node ID is {}", node_id);

    let access_key = activate(&node_id)?;
    kvstore::set_many([(keys::NODE_ID, &node_id), (keys::ACCESS_KEY, &access_key)].to_vec())?;
    log::info!("Activation successfully completed");

    Ok(())
}
