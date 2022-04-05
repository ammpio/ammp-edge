use crate::argsets::{KvsGetArgs, KvsSetArgs};
use anyhow::{anyhow, Result};
use kvstore::{DbRO, DbRW};
use serde_json::{json, Value};
use std::env;

const SQLITE_REL_PATH: &str = "kvs/kvstore.db";
const BASE_PATH_ENV_VAR: &str = "SNAP_COMMON";

pub fn kvs_set(args: KvsSetArgs) -> Result<()> {
    let db = DbRW::open(&sqlite_db_path())?;
    let res: Result<Value, serde_json::Error> = serde_json::from_str(&args.value);
    // If input was valid JSON, then set value to this;
    // otherwise treat input as a string, and generate JSON from it
    match res {
        Ok(value) => db.set(&args.key, value)?,
        Err(_) => db.set(&args.key, json!(args.value))?,
    }
    Ok(())
}

pub fn kvs_get(args: KvsGetArgs) -> Result<()> {
    let db = DbRO::open(&sqlite_db_path())?;
    let value: Value = db
        .get(&args.key)?
        .ok_or_else(|| anyhow!("No value set for key '{}'", &args.key))?;
    // If the value contains a single string, just output that
    if value.is_string() {
        print!("{}", value.as_str().unwrap());
    } else {
        print!("{}", value);
    }
    Ok(())
}

fn sqlite_db_path() -> String {
    format!(
        "{}/{}",
        env::var(BASE_PATH_ENV_VAR).unwrap_or_else(|_| String::from(".")),
        SQLITE_REL_PATH
    )
}
