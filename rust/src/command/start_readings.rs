use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use kvstore::KVDb;
use tokio::time::{Duration, interval, sleep};

use crate::{
    data_mgmt::{
        output::apply_outputs_to_device_readings, payload::Metadata,
        publish::publish_readings_with_publisher, readings::get_readings,
    },
    interfaces::{kvpath, mqtt::MqttPublisher},
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
    let read_interval = config.read_interval as u32;
    let read_roundtime = config.read_roundtime;

    log::info!(
        "Reading cycle configured - interval: {}s, roundtime: {}",
        read_interval,
        read_roundtime
    );

    // Create persistent MQTT publisher
    let mut mqtt_publisher = MqttPublisher::new(Some("data")).await;

    // Create interval timer
    if read_roundtime {
        sleep_until_aligned_interval(read_interval).await;
    }
    let mut interval_timer = interval(Duration::from_secs(read_interval as u64));

    // Main reading loop
    loop {
        interval_timer.tick().await;

        log::debug!("Starting reading cycle");

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

    // Sleep for 20 seconds to avoid interference with other reader
    sleep(Duration::from_secs(20)).await;

    // Delegate to the reading orchestrator
    let mut all_readings = get_readings(config).await?;

    // Ensure all readings have timestamps
    for reading in &mut all_readings {
        if reading.record.get_timestamp().is_none() {
            reading.record.set_timestamp(start_timestamp);
        }
    }

    // Calculate outputs and add to readings (modifies all_readings in-place)
    apply_outputs_to_device_readings(&mut all_readings, config);

    let reading_count = all_readings.len();
    log::info!("Publishing {} device readings", reading_count);

    let duration = start_time.elapsed();
    let metadata = Metadata {
        data_provider: Some("test".into()),
        reading_duration: Some(duration.as_secs_f64()),
        ..Default::default()
    };

    // Publish readings to MQTT
    publish_readings_with_publisher(mqtt_publisher, all_readings, Some(metadata)).await?;
    log::debug!(
        "Published {} device readings in {:?}",
        reading_count,
        duration
    );

    Ok(reading_count)
}

/// Sleep until the next aligned interval boundary
async fn sleep_until_aligned_interval(interval_secs: u32) {
    let interval_millis = interval_secs as u128 * 1_000;

    // Calculate next aligned time
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis();

    let next_aligned = now + (interval_millis) - (now % (interval_millis));
    let delay_until_aligned = Duration::from_millis((next_aligned - now).try_into().unwrap());

    log::debug!(
        "Aligning reading cycle to round timestamps, delay: {:?}",
        delay_until_aligned
    );

    // Sleep until aligned time
    sleep(delay_until_aligned).await;
}
