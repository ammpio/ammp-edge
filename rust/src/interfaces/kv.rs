use anyhow::Result;
use kvstore::{DbRO, DbRW};
use serde::{de::DeserializeOwned, Serialize};

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
    pub fn set<K: AsRef<str>, V: Serialize>(key: K, value: V) -> Result<()> {
        let db = DbRW::open(StoreType::sqlite_db_path())?;
        db.set(&key, &value)
    }

    pub fn set_many<K: AsRef<str>, V: Serialize>(pairs: Vec<(K, V)>) -> Result<()> {
        let db = DbRW::open(StoreType::sqlite_db_path())?;
        for (key, value) in pairs {
            db.set(key, value)?;
        }
        Ok(())
    }

    pub fn get<K: AsRef<str>, V: DeserializeOwned>(key: K) -> Result<Option<V>> {
        let db = DbRO::open(StoreType::sqlite_db_path())?;
        db.get(&key)
    }

    #[allow(dead_code)]
    pub fn get_many<K: AsRef<str>, V: DeserializeOwned>(keys: Vec<K>) -> Result<Vec<Option<V>>> {
        let db = DbRO::open(StoreType::sqlite_db_path())?;
        let mut res: Vec<Option<V>> = vec![];
        for key in keys {
            res.push(db.get(key)?);
        }
        Ok(res)
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
