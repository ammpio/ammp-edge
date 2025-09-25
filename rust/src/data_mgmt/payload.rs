use itertools::Itertools;
use thiserror::Error;

use super::models::DeviceReading;

pub use derived_models::data::{DataPayload, DeviceData, DeviceDataExtraValue, Metadata};

pub fn blank_metadata() -> Metadata {
    Metadata::default()
}

#[derive(Error, Debug)]
pub enum DataPayloadError {
    #[cfg(test)]
    #[error("could not parse data payload JSON: {0}")]
    ParseJson(#[from] serde_json::Error),
}

pub fn payloads_from_device_readings(
    device_readings: Vec<DeviceReading>,
    metadata: Option<Metadata>,
) -> Vec<DataPayload> {
    let mut payloads = Vec::new();
    for (timestamp, dev_rdgs) in &device_readings
        .into_iter()
        .chunk_by(|r| r.record.get_timestamp())
    {
        // Any records that are not explicitly timestamped will be ignored
        if let Some(ts) = timestamp {
            payloads.push(DataPayload {
                t: ts.timestamp(),
                r: dev_rdgs.map(device_data_from_device_reading).collect(),
                m: metadata.clone(),
            });
        }
    }
    payloads
}

fn device_data_from_device_reading(dev_rdg: DeviceReading) -> DeviceData {
    DeviceData {
        d: Some(dev_rdg.device.key),
        vid: dev_rdg.device.vendor_id,
        extra: dev_rdg.record.all_fields_as_device_data_extra(),
    }
}
