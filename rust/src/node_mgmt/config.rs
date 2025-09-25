use kvstore::{KVDb, KVStoreError};
use thiserror::Error;

use crate::constants::keys;

pub use derived_models::config::{AmmpEdgeConfiguration as Config, Device, ReadingType};

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error(transparent)]
    KvStore(#[from] KVStoreError),
    #[error("could not parse config JSON: {0}")]
    ParseJson(#[from] serde_json::Error),
    #[error("no config set")]
    NoConfigSet,
}

pub fn config_from_str(config_raw: &str) -> Result<Config, ConfigError> {
    serde_json::from_str::<Config>(config_raw).map_err(Into::into)
}

pub fn get(kvs: KVDb) -> Result<Config, ConfigError> {
    kvs.get(keys::CONFIG)?.ok_or(ConfigError::NoConfigSet)
}

pub fn set(kvs: KVDb, config: &Config) -> Result<(), ConfigError> {
    kvs.set(keys::CONFIG, config).map_err(Into::into)
}
