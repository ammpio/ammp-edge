use anyhow::Result;
use kvstore::{DbRO, DbRW};
use serde::{Serialize, de::DeserializeOwned};
use std::env;

const SQLITE_REL_PATH: &str = "kvs-db/kvstore.db";
const BASE_PATH_ENV_VAR: &str = "SNAP_COMMON";

pub fn set<K: AsRef<str>, V: Serialize>(key: K, value: V) -> Result<()> {
    let db = DbRW::open(&sqlite_db_path())?;
    db.set(&key, &value)
}

pub fn set_many<K: AsRef<str>, V: Serialize>(pairs: Vec<(K, V)>) -> Result<()> {
    let db = DbRW::open(&sqlite_db_path())?;
    for (key, value) in pairs {
        db.set(key, value)?;
    }
    Ok(())
}

pub fn get<K: AsRef<str>, V: DeserializeOwned>(key: K) -> Result<Option<V>> {
    let db = DbRO::open(&sqlite_db_path())?;
    db.get(&key)
}

#[allow(dead_code)]
pub fn get_many<K: AsRef<str>, V: DeserializeOwned>(keys: Vec<K>) -> Result<Vec<Option<V>>> {
    let db = DbRO::open(&sqlite_db_path())?;
    let mut res: Vec<Option<V>> = vec![];
    for key in keys {
        res.push(db.get(key)?);
    }
    Ok(res)
}

fn sqlite_db_path() -> String {
    format!(
        "{}/{}",
        env::var(BASE_PATH_ENV_VAR).unwrap_or_else(|_| String::from(".")),
        SQLITE_REL_PATH
    )
}
