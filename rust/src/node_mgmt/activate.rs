use anyhow::Result;
use kvstore::KVDb;

use crate::interfaces::http_api;

pub fn activate(kvs: &KVDb, node_id: &str) -> Result<String> {
    let api_root = http_api::get_api_root(kvs);
    let access_key = http_api::activate(&api_root, node_id)?;
    Ok(access_key)
}
