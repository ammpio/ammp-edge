use anyhow::Result;
use kvstore::{DbRO, DbRW};
use serde_json::Value;
use std::env;

const SQLITE_REL_PATH: &str = "kvs/kvstore.db";
const BASE_PATH_ENV_VAR: &str = "SNAP_COMMON";

const ACTION_GET: &str = "get";
const ACTION_SET: &str = "set";

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let action = &args[1];
    let key = &args[2];

    let sqlite_db = format!(
        "{}/{}",
        env::var(BASE_PATH_ENV_VAR).unwrap_or_else(|_| String::from(".")),
        SQLITE_REL_PATH
    );

    match action.as_str() {
        ACTION_GET => {
            let db = DbRO::open(&sqlite_db)?;
            let value = db.get_raw(key).expect("Error reading KV store");
            match value {
                Some(bytes) => {
                    let res: Result<Value, serde_json::Error> = serde_json::from_slice(&bytes);
                    if let Ok(parsed) = res {
                        if parsed.is_string() {
                            print!("{}", parsed.as_str().unwrap());
                            return Ok(());
                        }
                    }
                    print!("{}", String::from_utf8(bytes).unwrap())
                }
                None => eprintln!("No value set for key '{key}'"),
            }
        }
        ACTION_SET => {
            let value = &args[3];
            let db = DbRW::open(&sqlite_db)?;
            db.set_raw(key, value)?;
        }
        _ => panic!("Action must be one of 'get', 'set'"),
    }

    Ok(())
}
