use crate::helpers::base_path;

pub fn sqlite_store() -> String {
    format!("{}/kvs-db/kvstore.db", base_path::data_dir())
}

#[allow(dead_code)]
pub fn sqlite_cache() -> String {
    format!("{}/ae-kvcache.db", base_path::tmp_dir())
}
