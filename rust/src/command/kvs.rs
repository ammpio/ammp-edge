use crate::argsets::{KvsGetArgs, KvsSetArgs};
use anyhow::{Result, anyhow};
use kvstore::KVDb;
use serde_json::{Value, json};

use crate::interfaces::kvpath;

pub fn kvs_set(args: KvsSetArgs) -> Result<()> {
    let kvs = KVDb::new(kvpath::SQLITE_STORE.as_path())?;
    let res: Result<Value, serde_json::Error> = serde_json::from_str(&args.value);
    // If input was valid JSON, then set value to this;
    // otherwise treat input as a string, and generate JSON from it
    match res {
        Ok(value) => kvs.set(&args.key, value)?,
        Err(_) => kvs.set(&args.key, json!(args.value))?,
    }
    Ok(())
}

pub fn kvs_get(args: KvsGetArgs) -> Result<()> {
    let kvs = KVDb::new(kvpath::SQLITE_STORE.as_path())?;
    let value: Value = kvs
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
