use anyhow::Result;
use kvstore::KVDb;

use crate::{constants::keys, interfaces::kvpath};

use super::payload::DeviceData;

/// Save device readings to the cache, merging with existing readings if they share the same timestamp
pub fn save_last_readings(readings: Vec<DeviceData>, timestamp: i64) -> Result<()> {
    let cache = KVDb::new(kvpath::SQLITE_CACHE.as_path())?;

    // Get existing cached data
    let cached_timestamp: Option<i64> = cache.get(keys::LAST_READINGS_TS)?;
    let mut cached_readings: Vec<DeviceData> = cache.get(keys::LAST_READINGS)?.unwrap_or_default();

    // Extract device IDs from readings for per-device timestamp tracking
    let device_ids: Vec<String> = readings
        .iter()
        .filter_map(|r| r.d.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    // Merge logic based on timestamp
    let final_readings = if cached_timestamp == Some(timestamp) {
        // Same timestamp: combine the readings arrays
        log::debug!(
            "Merging {} new readings with {} cached readings for timestamp {}",
            readings.len(),
            cached_readings.len(),
            timestamp
        );
        cached_readings.extend(readings);
        cached_readings
    } else {
        // Different timestamp: replace with new readings
        log::debug!(
            "Replacing cached readings (ts: {:?}) with {} new readings for timestamp {}",
            cached_timestamp,
            readings.len(),
            timestamp
        );
        readings
    };

    // Save to cache
    cache.set(keys::LAST_READINGS, &final_readings)?;
    cache.set(keys::LAST_READINGS_TS, timestamp)?;

    // Save per-device timestamps
    for device_id in device_ids {
        let key = format!("{}/{}", keys::LAST_READING_TS_FOR_DEV_PFX, device_id);
        cache.set(&key, timestamp)?;
    }

    log::debug!(
        "Saved {} total readings to cache with timestamp {}",
        final_readings.len(),
        timestamp
    );

    Ok(())
}
