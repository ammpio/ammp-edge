use crate::argsets::{KvsGetArgs, KvsSetArgs};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};

use crate::interfaces::kvstore;

pub fn kvs_set(args: KvsSetArgs) -> Result<()> {
    let res: Result<Value, serde_json::Error> = serde_json::from_str(&args.value);
    // If input was valid JSON, then set value to this;
    // otherwise treat input as a string, and generate JSON from it
    match res {
        Ok(value) => kvstore::set(&args.key, value)?,
        Err(_) => kvstore::set(&args.key, json!(args.value))?,
    }
    Ok(())
}

pub fn kvs_get(args: KvsGetArgs) -> Result<()> {
    let value: Value = kvstore::get(&args.key)?
        .ok_or_else(|| anyhow!("No value set for key '{}'", &args.key))?;
    // If the value contains a single string, just output that
    if value.is_string() {
        print!("{}", value.as_str().unwrap());
    } else {
        print!("{}", value);
    }
    Ok(())
}
