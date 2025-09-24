use anyhow::Result;
use kvstore::KVDb;
use tokio::time::{interval, Duration};

use crate::{
    interfaces::kvpath,
    node_mgmt,
};

/// Start the continuous reading cycle for ModbusTCP devices
pub async fn start_readings() -> Result<()> {
    log::info!("Starting ModbusTCP reading cycle...");

    // Load configuration from key-value store
    let kvs = KVDb::new(kvpath::SQLITE_STORE.as_path())?;
    let config = node_mgmt::config::get(kvs)?;

    // Extract timing parameters from config
    // Note: These fields may not exist in current schema - will be added in Phase 4
    let read_interval = extract_read_interval(&config);
    let read_roundtime = extract_read_roundtime(&config);

    log::info!(
        "Reading cycle configured - interval: {}s, roundtime: {}",
        read_interval, read_roundtime
    );

    // Create interval timer
    let mut interval_timer = if read_roundtime {
        create_aligned_interval(read_interval).await
    } else {
        interval(Duration::from_secs(read_interval as u64))
    };

    // Main reading loop
    loop {
        interval_timer.tick().await;

        log::debug!("Starting reading cycle iteration");

        match execute_reading_cycle(&config).await {
            Ok(reading_count) => {
                log::info!("Completed reading cycle: {} readings", reading_count);
            }
            Err(e) => {
                log::error!("Reading cycle error: {}", e);
            }
        }
    }
}

/// Execute one iteration of the reading cycle
async fn execute_reading_cycle(_config: &node_mgmt::Config) -> Result<usize> {
    // TODO: Phase 2 - Implement device filtering and reading
    // TODO: Phase 3 - Implement ModbusTCP reading
    // TODO: Phase 4 - Implement data processing
    // TODO: Phase 5 - Implement MQTT publishing

    log::debug!("Reading cycle iteration (placeholder)");

    // For now, just return 0 readings as a placeholder
    Ok(0)
}

/// Extract read_interval from config, with default fallback
fn extract_read_interval(_config: &node_mgmt::Config) -> u32 {
    // TODO: Phase 4 - Extract from config schema
    // For now, use a sensible default
    60 // Default: 60 seconds
}

/// Extract read_roundtime from config, with default fallback
fn extract_read_roundtime(_config: &node_mgmt::Config) -> bool {
    // TODO: Phase 4 - Extract from config schema
    // For now, use a sensible default
    false // Default: no round time alignment
}

/// Create an interval timer aligned to round timestamps
async fn create_aligned_interval(interval_secs: u32) -> tokio::time::Interval {
    use std::time::{SystemTime, UNIX_EPOCH};

    // Calculate next aligned time
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs();

    let next_aligned = now + (interval_secs as u64) - (now % (interval_secs as u64));
    let delay_until_aligned = Duration::from_secs(next_aligned - now);

    log::debug!("Aligning reading cycle to round timestamps, delay: {:?}", delay_until_aligned);

    // Sleep until aligned time, then create regular interval
    tokio::time::sleep(delay_until_aligned).await;
    interval(Duration::from_secs(interval_secs as u64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_defaults() {
        // Create a minimal config for testing using serde_json::Value
        let config_json = serde_json::json!({
            "devices": {},
            "readings": {}
        });

        // Parse it into a Config - this will be updated when we have proper schema
        let config: node_mgmt::Config = serde_json::from_value(config_json).unwrap();

        assert_eq!(extract_read_interval(&config), 60);
        assert_eq!(extract_read_roundtime(&config), false);
    }
}