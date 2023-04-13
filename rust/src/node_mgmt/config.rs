use std::str::FromStr;

use kvstore::{KVDb, KVStoreError};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use typify::import_types;

use crate::constants::keys;

import_types!(
    schema = "resources/schema/config.schema.json",
    derives = [Clone, Eq, PartialEq]
);

pub type Config = AmmpEdgeConfiguration;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error(transparent)]
    KvStore(#[from] KVStoreError),
    #[error("could not parse config JSON: {0}")]
    ParseJson(#[from] serde_json::Error),
    #[error("no config set")]
    NoConfigSet,
}

impl FromStr for Config {
    type Err = ConfigError;
    fn from_str(config_raw: &str) -> Result<Self, Self::Err> {
        serde_json::from_str::<Config>(config_raw).map_err(Into::into)
    }
}

pub fn get(kvs: KVDb) -> Result<Config, ConfigError> {
    kvs.get(keys::CONFIG)?.ok_or(ConfigError::NoConfigSet)
}

pub fn set(kvs: KVDb, config: &Config) -> Result<(), ConfigError> {
    kvs.set(keys::CONFIG, config).map_err(Into::into)
}
