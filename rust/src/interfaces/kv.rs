use anyhow::Result;
use kvstore::Db;

use crate::helpers::base_path;

pub struct Store;
pub struct Cache;

pub struct KV<StoreType>(StoreType);

pub type KVStore = KV<Store>;
#[allow(dead_code)]
pub type KVCache = KV<Cache>;

impl<StoreType> KV<StoreType>
where
    StoreType: DbPath,
{
    pub fn new() -> Result<Db> {
        Db::open(StoreType::sqlite_db_path())
    }
}

pub trait DbPath {
    fn sqlite_db_path() -> String;
}

impl DbPath for Store {
    fn sqlite_db_path() -> String {
        format!("{}/kvs-db/kvstore.db", base_path::data_dir())
    }
}

impl DbPath for Cache {
    fn sqlite_db_path() -> String {
        format!("{}/ae-kvcache.db", base_path::tmp_dir())
    }
}
