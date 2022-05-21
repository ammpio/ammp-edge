#![feature(explicit_generic_args_with_impl_trait)]

mod argsets;
mod command;
mod constants;
mod helpers;
mod interfaces;
mod node_mgmt;

use anyhow::{anyhow, Result};
use env_logger::Env;

use helpers::load_dotenv;
use constants::envvars;

pub const CMD_INIT: &str = "init";
pub const CMD_KVS_GET: &str = "kvs-get";
pub const CMD_KVS_SET: &str = "kvs-set";

const DEFAULT_LOG_LEVEL: &str = "info";

fn main() -> Result<()> {
    load_dotenv();
    env_logger::Builder::from_env(Env::default().filter_or(envvars::LOG_LEVEL, DEFAULT_LOG_LEVEL))
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
            "Subcommand must be one of '{CMD_INIT}', '{CMD_KVS_GET}', '{CMD_KVS_SET}'"
        )),
    }
}
