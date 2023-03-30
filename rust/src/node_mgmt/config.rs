use kvstore::{KVDb, KVStoreError};

use crate::constants::keys;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use typify::import_types;

import_types!(schema = "../resources/schema/config.schema.json");

pub type Config = AmmpEdgeConfiguration;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("could not parse config JSON: {0}")]
    ParseJson(#[from] serde_json::Error),
}

pub fn from_str(config_raw: &str) -> Result<Config, ConfigError> {
    serde_json::from_str::<Config>(config_raw).map_err(Into::into)
}

pub fn set(kvs: KVDb, config: &Config) -> Result<(), KVStoreError> {
    kvs.set(keys::CONFIG, config)
}
