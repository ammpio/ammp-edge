use ae::argsets;
use ae::command;
use ae::constants;
use ae::helpers;

use anyhow::{anyhow, Result};
use env_logger::Env;

use constants::{defaults, envvars};
use helpers::load_dotenv;

const CMD_INIT: &str = "init";
const CMD_KVS_GET: &str = "kvs-get";
const CMD_KVS_SET: &str = "kvs-set";
const CMD_MQTT_PUB_META: &str = "mqtt-pub-meta";
const CMD_MQTT_SUB_CFG_CMD: &str = "mqtt-sub-cfg-cmd";
const CMD_READ_SMA_HYCON_CSV: &str = "read-sma-hycon-csv";
const CMD_WAIT_FOR_TIME_SOURCE: &str = "wait-for-time-source";

fn main() -> Result<()> {
    load_dotenv();
    env_logger::Builder::from_env(
        Env::default().filter_or(envvars::LOG_LEVEL, defaults::LOG_LEVEL),
    )
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
        Some(CMD_MQTT_PUB_META) => command::mqtt_pub_meta(),
        Some(CMD_MQTT_SUB_CFG_CMD) => command::mqtt_sub_cfg_cmd(),
        Some(CMD_READ_SMA_HYCON_CSV) => command::read_sma_hycon_csv(),
        Some(CMD_WAIT_FOR_TIME_SOURCE) => command::wait_for_time_source(),
        _ => Err(anyhow!(
            r#"
            Subcommand must be one of '{CMD_INIT}', '{CMD_KVS_GET}', '{CMD_KVS_SET}', '{CMD_MQTT_PUB_META}', '{CMD_MQTT_SUB_CFG_CMD}',
            '{CMD_WAIT_FOR_TIME_SOURCE}', '{CMD_READ_SMA_HYCON_CSV}'
            "#
        )),
    }
}
