mod argsets;
mod command;
mod helpers;
mod interfaces;
mod node_mgmt;

use anyhow::{anyhow, Result};
use dotenv::dotenv;
use env_logger::Env;

const CMD_INIT: &str = "init";
const CMD_KVS_GET: &str = "kvs-get";
const CMD_KVS_SET: &str = "kvs-set";

const LOG_LEVEL_ENV_VAR: &str = "LOGGING_LEVEL";
const DEFAULT_LOG_LEVEL: &str = "INFO";

fn main() -> Result<()> {
    let _ = dotenv();
    env_logger::Builder::from_env(Env::default().filter_or(LOG_LEVEL_ENV_VAR, DEFAULT_LOG_LEVEL))
        .init();

    let mut args = pico_args::Arguments::from_env();
    match args.subcommand()?.as_deref() {
        Some(CMD_INIT) => command::init(),
        Some(CMD_KVS_GET) => command::kvs_get(argsets::KvsGetArgs {
            key: args.free_from_str()?,
        }),
        Some(CMD_KVS_SET) => command::kvs_set(argsets::KvsSetArgs {
            key: args.free_from_str()?,
            value: args.free_from_str()?,
        }),
        _ => Err(anyhow!(
            "Subcommand must be one of 'init', 'kvs-get', 'kvs-set'"
        )),
    }
}
