use anyhow::Result;
use crate::node_funcs::node_id::generate_node_id;

pub fn init() -> Result<()> {
    println!("Node ID is {}", generate_node_id());
    Ok(())
}