use std::path::PathBuf;

use lazy_static::lazy_static;

use crate::helpers::base_path;

lazy_static! {
    pub static ref SQLITE_STORE: PathBuf = base_path::DATA_DIR.join("kvs-db/kvstore.db");
    pub static ref SQLITE_CACHE: PathBuf = base_path::TEMP_DIR.join("ae-kvcache.db");
}
