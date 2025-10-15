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
        s: dev_rdg.record.status_readings().to_vec(),
        extra: dev_rdg.record.all_fields_as_device_data_extra(),
    }
}

/// Filter status info in existing payloads to remove readings where the level
/// matches the last cached level.
pub fn filter_status_info_in_payloads(mut payloads: Vec<DataPayload>) -> Vec<DataPayload> {
    let cache = match KVDb::new(kvpath::SQLITE_CACHE.as_path()) {
        Ok(cache) => cache,
        Err(e) => {
            log::error!("Failed to create cache for status info filtering: {}", e);
            return payloads;
        }
    };

    for payload in &mut payloads {
        for device_data in &mut payload.r {
            if let Some(device_key) = &device_data.d {
                let status_readings = std::mem::take(&mut device_data.s);
                device_data.s = filter_status_readings(device_key, status_readings, &cache);

                if !device_data.s.is_empty() {
                    log::info!(
                        "New status readings for {}: {:?}",
                        device_key,
                        device_data.s
                    );
                }
            }
        }
    }

    payloads
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
            log::debug!(
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
    fn test_filter_status_info_in_payloads() {
        use std::collections::BTreeMap;

        // Create payload with status readings
        let payloads = vec![DataPayload {
            t: 1234567890,
            r: vec![DeviceData {
                d: Some("test_device".to_string()),
                vid: "vendor-123".to_string(),
                s: vec![
                    StatusReading {
                        c: "alarm1".to_string(),
                        l: 1,
                    },
                    StatusReading {
                        c: "alarm2".to_string(),
                        l: 2,
                    },
                ],
                extra: BTreeMap::new(),
            }],
            m: None,
        }];

        // Filter the payloads - this test verifies the function works
        // (actual filtering logic is tested via test_filter_status_readings_mixed)
        let filtered = filter_status_info_in_payloads(payloads);

        // Should preserve structure even if cache is unavailable
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].r.len(), 1);
    }
}
