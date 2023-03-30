use std::str::FromStr;

use kvstore::{KVDb, KVStoreError};

use crate::constants::keys;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use typify::import_types;

import_types!(schema = "../resources/schema/config.schema.json", derives = [PartialEq]);

pub type Config = AmmpEdgeConfiguration;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("could not parse config JSON: {0}")]
    ParseJson(#[from] serde_json::Error),
}

impl FromStr for Config {
    type Err = ConfigError;
    fn from_str(config_raw: &str) -> Result<Self, Self::Err> {
        serde_json::from_str::<Config>(config_raw).map_err(Into::into)
    }
}

pub fn set(kvs: KVDb, config: &Config) -> Result<(), KVStoreError> {
    kvs.set(keys::CONFIG, config)
}
