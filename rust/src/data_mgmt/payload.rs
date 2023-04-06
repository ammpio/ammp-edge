#[cfg(test)]
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use typify::import_types;

import_types!(
    schema = "../resources/schema/data.schema.json",
    derives = [PartialEq]
);

#[derive(Error, Debug)]
pub enum DataPayloadError {
    #[cfg(test)]
    #[error("could not parse data payload JSON: {0}")]
    ParseJson(#[from] serde_json::Error),
}

#[cfg(test)]
impl FromStr for DataPayload {
    type Err = DataPayloadError;
    fn from_str(data_payload: &str) -> Result<Self, Self::Err> {
        serde_json::from_str::<DataPayload>(data_payload).map_err(Into::into)
    }
}
