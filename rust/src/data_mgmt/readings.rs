//! Reading orchestration - determines what readings to take and delegates to specific readers
//!
//! This module organizes readings by device and delegates to the appropriate reader modules.

use anyhow::Result;
use std::collections::HashMap;
use tokio::time::{Duration, Instant};

use crate::{
    data_mgmt::models::DeviceReading,
    node_mgmt::config::{Config, Device, ReadingType},
    readers::modbus_tcp,
};

/// Main entry point for reading orchestration
///
/// Analyzes the configuration, determines what ModbusTCP readings need to be taken,
/// organizes them by device, and delegates to the ModbusTCP reader.
/// Other device types have their own dedicated commands.
pub async fn get_readings(config: &Config) -> Result<Vec<DeviceReading>> {
    let start_time = Instant::now();

    log::info!("Starting ModbusTCP reading cycle");

    // Organize readings by device
    let device_readings_map = organize_readings_by_device(config)?;

    if device_readings_map.is_empty() {
        log::debug!("No enabled devices with readings found");
        return Ok(Vec::new());
    }

    log::info!(
        "Found {} device(s) with readings to process",
        device_readings_map.len()
    );

    // Execute readings for ModbusTCP devices only
    // Other device types (like SMA HyCon CSV) have their own dedicated commands
    let all_readings = read_modbus_devices(config, &device_readings_map).await?;

    let duration = start_time.elapsed();
    log::info!(
        "Completed reading cycle: {} readings in {:?}",
        all_readings.len(),
        duration
    );

    Ok(all_readings)
}

/// Organize readings by device, filtering for enabled devices and readings
fn organize_readings_by_device(
    config: &Config,
) -> Result<HashMap<String, (Device, Vec<ReadingRequest>)>> {
    let mut device_readings_map: HashMap<String, (Device, Vec<ReadingRequest>)> = HashMap::new();

    // Iterate through all configured readings
    for (reading_name, reading_config) in &config.readings {
        // Note: ReadingSchema doesn't have an enabled field, so we process all readings
        // If needed, enabled/disabled functionality can be added at the device level

        // Get the device this reading refers to
        let device_id = &reading_config.device;
        let Some(device) = config.devices.get(device_id) else {
            log::error!(
                "Reading '{}' references unknown device '{}'",
                reading_name,
                device_id
            );
            continue;
        };

        // Skip explicitly disabled devices
        if !device.enabled {
            continue;
        }

        // Create device with key populated
        let device_with_key = Device {
            key: device_id.clone(),
            ..device.clone()
        };

        // Create reading request
        let reading_request = ReadingRequest {
            reading_name: reading_name.clone(),
            variable_name: reading_config.var.clone(),
        };

        // Add to device map
        device_readings_map
            .entry(device_id.clone())
            .or_insert_with(|| (device_with_key, Vec::new()))
            .1
            .push(reading_request);
    }

    Ok(device_readings_map)
}

/// Process all ModbusTCP devices
async fn read_modbus_devices(
    config: &Config,
    device_readings_map: &HashMap<String, (Device, Vec<ReadingRequest>)>,
) -> Result<Vec<DeviceReading>> {
    // Filter ModbusTCP devices
    let modbus_devices: Vec<_> = device_readings_map
        .iter()
        .filter(|(_, (device, _))| device.reading_type == ReadingType::Modbustcp)
        .collect();

    if modbus_devices.is_empty() {
        return Ok(Vec::new());
    }

    log::info!("Processing {} ModbusTCP device(s)", modbus_devices.len());

    // Create reading tasks for each device
    let reading_tasks =
        modbus_devices
            .into_iter()
            .map(|(device_id, (device, reading_requests))| {
                let device_id = device_id.clone();
                let device = device.clone();
                let reading_requests = reading_requests.clone();
                let config = config.clone();

                tokio::spawn(async move {
                    modbus_tcp::read_device(&config, &device_id, &device, &reading_requests).await
                })
            });

    // Execute all tasks concurrently with timeout
    let timeout_duration = Duration::from_secs(60); // 1 minute max for all devices
    let results =
        tokio::time::timeout(timeout_duration, futures::future::join_all(reading_tasks)).await?;

    // Collect successful readings
    let mut readings = Vec::new();
    for result in results {
        match result? {
            Ok(device_readings) => {
                readings.extend(device_readings);
            }
            Err(e) => {
                log::warn!("ModbusTCP device reading failed: {}", e);
            }
        }
    }

    Ok(readings)
}

/// Represents a reading request for a specific variable on a device
#[derive(Clone, Debug)]
pub struct ReadingRequest {
    /// Name of the reading in the configuration
    pub reading_name: String,
    /// Variable name to read from the device
    pub variable_name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_organize_readings_empty_config() {
        let config_json = serde_json::json!({
            "devices": {},
            "readings": {},
            "read_interval": 60,
            "read_roundtime": false
        });
        let config: Config = serde_json::from_value(config_json).unwrap();

        let result = organize_readings_by_device(&config).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_organize_readings_with_devices() {
        let config_json = serde_json::json!({
            "devices": {
                "test_device": {
                    "key": "test_device",
                    "driver": "modbus_tcp",
                    "reading_type": "modbustcp",
                    "device_model": "test",
                    "vendor_id": "test-123",
                    "enabled": true,
                    "address": {
                        "host": "192.168.1.100",
                        "port": 502,
                        "unit_id": 1
                    }
                }
            },
            "readings": {
                "voltage": {
                    "device": "test_device",
                    "var": "voltage_L1",
                    "enabled": true
                },
                "power": {
                    "device": "test_device",
                    "var": "power_total",
                    "enabled": true
                }
            },
            "read_interval": 60,
            "read_roundtime": false
        });
        let config: Config = serde_json::from_value(config_json).unwrap();

        let result = organize_readings_by_device(&config).unwrap();

        assert_eq!(result.len(), 1);
        assert!(result.contains_key("test_device"));

        let (device, readings) = &result["test_device"];
        assert_eq!(device.key, "test_device");
        assert_eq!(readings.len(), 2);

        let reading_names: Vec<_> = readings.iter().map(|r| &r.reading_name).collect();
        assert!(reading_names.contains(&&"voltage".to_string()));
        assert!(reading_names.contains(&&"power".to_string()));
    }
}
