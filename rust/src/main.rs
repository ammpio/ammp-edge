mod argsets;
mod command;

use anyhow::{anyhow, Result};
use command::{kvs_get, kvs_set};

const ACTION_KVS_GET: &str = "kvs-get";
const ACTION_KVS_SET: &str = "kvs-set";

fn main() -> Result<()> {
    let mut args = pico_args::Arguments::from_env();
    match args.subcommand()?.as_deref() {
        Some(ACTION_KVS_GET) => kvs_get(argsets::KvsGetArgs {
            key: args.free_from_str()?,
        }),
        Some(ACTION_KVS_SET) => kvs_set(argsets::KvsSetArgs {
            key: args.free_from_str()?,
            value: args.free_from_str()?,
        }),
        _ => Err(anyhow!("Subcommand must be one of 'kvs-get', 'kvs-set'")),
    }
}
