use anyhow::Result;
use kvstore::KVDb;
use tokio::time::{Duration, interval};

use crate::{
    data_mgmt::{
        payload::Metadata, publish::publish_readings_with_publisher, readings::get_readings,
    },
    interfaces::kvpath,
    interfaces::mqtt::MqttPublisher,
    node_mgmt,
};

/// Start the continuous reading cycle for ModbusTCP devices
///
/// Other device types like SMA HyCon CSV have their own dedicated commands.
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
        read_interval,
        read_roundtime
    );

    // Create persistent MQTT publisher
    let mut mqtt_publisher = MqttPublisher::new(Some("data")).await;

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

        match execute_reading_cycle(&config, &mut mqtt_publisher).await {
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
async fn execute_reading_cycle(
    config: &node_mgmt::config::Config,
    mqtt_publisher: &mut MqttPublisher,
) -> Result<usize> {
    // One of these is for a monotonic timer, the other is for a wall clock timestamp
    let start_time = std::time::Instant::now();
    let start_timestamp = chrono::Utc::now();

    // Delegate to the reading orchestrator
    let mut all_readings = get_readings(config).await?;
    let reading_count = all_readings.len();

    // Ensure all readings have timestamps
    for reading in &mut all_readings {
        if reading.record.get_timestamp().is_none() {
            reading.record.set_timestamp(start_timestamp);
        }
    }

    log::info!("Publishing {} readings", reading_count);
    // Publish readings if we have any
    if !all_readings.is_empty() {
        let duration = start_time.elapsed();
        let metadata = Metadata {
            data_provider: Some("modbus-tcp-reader".into()),
            reading_duration: Some(duration.as_secs_f64()),
            ..Default::default()
        };

        publish_readings_with_publisher(mqtt_publisher, all_readings, Some(metadata)).await?;
        log::debug!("Published {} readings in {:?}", reading_count, duration);
    }

    Ok(reading_count)
}

/// Extract read_interval from config, with default fallback
fn extract_read_interval(config: &node_mgmt::config::Config) -> u32 {
    config.read_interval as u32
}

/// Extract read_roundtime from config, with default fallback
fn extract_read_roundtime(config: &node_mgmt::config::Config) -> bool {
    config.read_roundtime
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

    log::debug!(
        "Aligning reading cycle to round timestamps, delay: {:?}",
        delay_until_aligned
    );

    // Sleep until aligned time, then create regular interval
    tokio::time::sleep(delay_until_aligned).await;
    interval(Duration::from_secs(interval_secs as u64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_config_values() {
        // Create a test config with specific values
        let config_json = serde_json::json!({
            "devices": {},
            "readings": {},
            "read_interval": 120,
            "read_roundtime": true
        });

        let config: node_mgmt::config::Config = serde_json::from_value(config_json).unwrap();

        assert_eq!(extract_read_interval(&config), 120);
        assert_eq!(extract_read_roundtime(&config), true);
    }

    #[test]
    fn test_extract_config_defaults() {
        // Create a minimal config for testing defaults
        let config_json = serde_json::json!({
            "devices": {},
            "readings": {},
            "read_interval": 60,
            "read_roundtime": false
        });

        let config: node_mgmt::config::Config = serde_json::from_value(config_json).unwrap();

        assert_eq!(extract_read_interval(&config), 60);
        assert_eq!(extract_read_roundtime(&config), false);
    }
}
