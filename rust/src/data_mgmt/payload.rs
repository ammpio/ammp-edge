use itertools::Itertools;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use typify::import_types;

use super::models::DeviceReading;

import_types!(
    schema = "../resources/schema/data.schema.json",
    derives = [PartialEq]
);

const BLANK_METADATA: Metadata = Metadata { config_id: None, reading_duration: None, snap_rev: None };

#[derive(Error, Debug)]
pub enum DataPayloadError {
    #[cfg(test)]
    #[error("could not parse data payload JSON: {0}")]
    ParseJson(#[from] serde_json::Error),
}

pub fn payloads_from_device_readings(device_readings: Vec<DeviceReading>) -> Vec<DataPayload> {
    let mut payloads = Vec::new();
    for (timestamp, dev_rdgs) in &device_readings
        .into_iter()
        .group_by(|r| r.record.get_timestamp())
    {
        // Any records that are not explicitly timestamped will be ignored
        if let Some(ts) = timestamp {
            payloads.push(DataPayload {
                t: ts.timestamp(),
                r: dev_rdgs.map(device_data_from_device_reading).collect(),
                m: BLANK_METADATA,
            });
        }
    }
    payloads
}

fn device_data_from_device_reading(dev_rdg: DeviceReading) -> DeviceData {
    DeviceData {
        d: None,
        vid: dev_rdg.device.vendor_id,
        extra: dev_rdg.record.all_fields_as_device_data_extra(),
    }
}
