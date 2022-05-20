use std::path::PathBuf;

use once_cell::sync::Lazy;

use crate::helpers::base_path;

pub static SQLITE_STORE: Lazy<PathBuf> = Lazy::new(||
    base_path::DATA_DIR.join("kvs-db/kvstore.db")
);

pub static SQLITE_CACHE: Lazy<PathBuf> = Lazy::new(||
    base_path::TEMP_DIR.join("ae-kvcache.db")
);
