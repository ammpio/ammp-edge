use std::{error::Error, fmt};

use kvstore::{KVDb, KVStoreError};

use crate::constants::keys;

pub type Config = serde_json::Value;

#[derive(Debug)]
pub enum ConfigError {
    JsonParse(serde_json::Error),
}

impl Error for ConfigError {}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ConfigError::JsonParse(ref err) => write!(f, "Cannot parse config as JSON: {:?}", err),
        }
    }
}

impl From<serde_json::Error> for ConfigError {
    fn from(err: serde_json::Error) -> ConfigError {
        ConfigError::JsonParse(err)
    }
}

pub fn from_string(config_raw: &str) -> Result<Config, ConfigError> {
    serde_json::from_str::<Config>(config_raw).map_err(Into::into)
}

pub fn set(kvs: KVDb, config: &Config) -> Result<(), KVStoreError> {
    kvs.set(keys::CONFIG, config)
}
