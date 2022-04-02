use anyhow::{anyhow, Result};
use kvstore::{DbRO, DbRW};
use serde_json::{json, Value};
use std::env;

const SQLITE_REL_PATH: &str = "kvs/kvstore.db";
const BASE_PATH_ENV_VAR: &str = "SNAP_COMMON";

const ARG_ACTION: usize = 1;
const ARG_KEY: usize = 2;
const ARG_VALUE: usize = 3;

const ACTION_GET: &str = "get";
const ACTION_SET: &str = "set";

fn do_get(sqlite_db: String, key: &str) -> Result<()> {
    let db = DbRO::open(&sqlite_db)?;
    let value: Value = db
        .get(key)?
        .ok_or_else(|| anyhow!("No value set for key '{key}'"))?;
    // If the value contains a single string, just output that
    if value.is_string() {
        print!("{}", value.as_str().unwrap());
    } else {
        print!("{}", value);
    }
    Ok(())
}

fn do_set(sqlite_db: String, key: &str, value_str: &str) -> Result<()> {
    let db = DbRW::open(&sqlite_db)?;
    let res: Result<Value, serde_json::Error> = serde_json::from_str(value_str);
    // If input was valid JSON, then set value to this;
    // otherwise treat input as a string, and generate JSON from it
    match res {
        Ok(value) => db.set(key, value)?,
        Err(_) => db.set(key, json!(value_str))?,
    }
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let action = &args[ARG_ACTION];
    let key = &args[ARG_KEY];

    let sqlite_db = format!(
        "{}/{}",
        env::var(BASE_PATH_ENV_VAR).unwrap_or_else(|_| String::from(".")),
        SQLITE_REL_PATH
    );

    match action.as_str() {
        ACTION_GET => do_get(sqlite_db, key),
        ACTION_SET => {
            let value = &args[ARG_VALUE];
            do_set(sqlite_db, key, value)
        }
        _ => Err(anyhow!("Action (1st arg) must be one of 'get', 'set'")),
    }
}
