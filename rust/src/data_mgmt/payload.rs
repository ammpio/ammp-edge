use itertools::Itertools;
use kvstore::KVDb;

use super::models::DeviceReading;
use crate::{constants::keys, interfaces::kvpath};

pub use derived_models::data::{
    DataPayload, DeviceData, DeviceDataExtraValue, Metadata, StatusReading,
};

pub fn blank_metadata() -> Metadata {
    Metadata::default()
}

pub fn payloads_from_device_readings(
    device_readings: Vec<DeviceReading>,
    metadata: Option<Metadata>,
) -> Vec<DataPayload> {
    let cache = match KVDb::new(kvpath::SQLITE_CACHE.as_path()) {
        Ok(cache) => Some(cache),
        Err(e) => {
            log::error!("Failed to create cache: {}", e);
            None
        }
    };
    let mut payloads = Vec::new();
    for (timestamp, dev_rdgs) in &device_readings
        .into_iter()
        .chunk_by(|r| r.record.get_timestamp())
    {
        // Any records that are not explicitly timestamped will be ignored
        if let Some(ts) = timestamp {
            payloads.push(DataPayload {
                t: ts.timestamp(),
                r: dev_rdgs
                    .map(|dev_rdg| device_data_from_device_reading(dev_rdg, &cache))
                    .collect(),
                m: metadata.clone(),
            });
        }
    }
    payloads
}

fn device_data_from_device_reading(dev_rdg: DeviceReading, cache: &Option<KVDb>) -> DeviceData {
    let mut status_readings = dev_rdg.record.status_readings().to_vec();
    if let Some(cache) = cache {
        status_readings = filter_status_readings(&dev_rdg.device.key, status_readings, cache);
    }

    log::debug!(
        "Status readings for {}: {:?}",
        dev_rdg.device.key,
        status_readings
    );

    DeviceData {
        d: Some(dev_rdg.device.key),
        vid: dev_rdg.device.vendor_id,
        s: status_readings,
        extra: dev_rdg.record.all_fields_as_device_data_extra(),
    }
}

// Filter out status readings where the level is the same as the last recorded one
fn filter_status_readings(
    device_key: &str,
    status_readings: Vec<StatusReading>,
    cache: &KVDb,
) -> Vec<StatusReading> {
    status_readings
        .into_iter()
        .filter(|s| {
            let cached_level = get_last_cached_status_level(device_key, &s.c, cache);
            log::trace!(
                "Cached level for {}/{}: {:?}",
                device_key,
                s.c,
                cached_level
            );
            // Only keep readings where the level is different from the cached level
            Some(s.l) != cached_level
        })
        .collect()
}

// Get last status level from cache
fn get_last_cached_status_level(device_id: &str, content: &str, cache: &KVDb) -> Option<u8> {
    let key = format!(
        "{}/{}/{}",
        keys::LAST_STATUS_INFO_LEVEL_PFX,
        device_id,
        content
    );
    match cache.get(&key) {
        Ok(level) => level,
        Err(e) => {
            log::warn!(
                "Could not get cached status level for {}/{}: {}",
                device_id,
                content,
                e
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_mgmt::models::{DeviceRef, Record};

    #[test]
    fn test_filter_status_readings_mixed() {
        let temp_cache = tempfile::NamedTempFile::new().unwrap();
        let cache = KVDb::new(temp_cache.path()).unwrap();

        // Cache some status levels
        cache
            .set(
                format!("{}/device1/alarm1", keys::LAST_STATUS_INFO_LEVEL_PFX),
                1u8,
            )
            .unwrap();
        cache
            .set(
                format!("{}/device1/alarm2", keys::LAST_STATUS_INFO_LEVEL_PFX),
                0u8,
            )
            .unwrap();

        let status_readings = vec![
            StatusReading {
                c: "alarm1".to_string(),
                l: 1, // Same as cache - should be filtered out
            },
            StatusReading {
                c: "alarm2".to_string(),
                l: 2, // Different from cache - should pass
            },
            StatusReading {
                c: "alarm3".to_string(),
                l: 3, // Not in cache - should pass
            },
        ];

        let filtered = filter_status_readings("device1", status_readings, &cache);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].c, "alarm2");
        assert_eq!(filtered[1].c, "alarm3");
    }

    #[test]
    fn test_device_data_from_device_reading_with_cache() {
        let temp_cache = tempfile::NamedTempFile::new().unwrap();
        let cache = KVDb::new(temp_cache.path()).unwrap();

        // Cache alarm1 with level 1 (matching what we'll send)
        cache
            .set(
                format!("{}/test_device/alarm1", keys::LAST_STATUS_INFO_LEVEL_PFX),
                1u8,
            )
            .unwrap();

        let device = DeviceRef::new("test_device".to_string(), "vendor-123".to_string());

        let mut record = Record::new();
        record.add_status_reading(StatusReading {
            c: "alarm1".to_string(),
            l: 1, // Same as cache - should be filtered
        });
        record.add_status_reading(StatusReading {
            c: "alarm2".to_string(),
            l: 2, // Not in cache - should pass
        });

        let reading = DeviceReading { device, record };

        // With cache, only alarm2 should be included
        let device_data = device_data_from_device_reading(reading, &Some(cache));
        assert_eq!(device_data.s.len(), 1);
        assert_eq!(device_data.s[0].c, "alarm2");
    }
}
