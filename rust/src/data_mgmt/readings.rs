//! Reading orchestration - determines what readings to take and delegates to specific readers
//!
//! This module organizes readings by device and delegates to the appropriate reader modules.
use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use kvstore::KVDb;
use once_cell::sync::Lazy;
use tokio::sync::Mutex;
use tokio::time::Duration;

use crate::{
    constants::keys,
    data_mgmt::models::DeviceReading,
    interfaces::kvpath,
    node_mgmt::config::{Config, Device, ReadingType},
    node_mgmt::drivers::{DriverSchema, load_driver},
    readers::modbus_tcp,
};

/// Global registry of mutexes for physical devices
/// Ensures only one read operation per physical device at a time
static DEVICE_LOCKS: Lazy<Mutex<HashMap<PhysicalDeviceId, Arc<Mutex<()>>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Main entry point for reading orchestration
///
/// Analyzes the configuration, determines what ModbusTCP readings need to be taken,
/// organizes them by device, and delegates to the ModbusTCP reader.
/// In the future this will be extended to support other device types.
pub async fn get_readings(
    reading_timestamp: DateTime<Utc>,
    config: &Config,
) -> Result<Vec<DeviceReading>> {
    log::info!("Starting ModbusTCP reading cycle");

    // Organize readings by device, filtering based on min_read_interval
    let device_readings_map = organize_readings_by_device(reading_timestamp, config).await?;

    if device_readings_map.is_empty() {
        log::debug!("No enabled devices with readings found");
        return Ok(Vec::new());
    }

    log::info!(
        "Found {} device(s) with readings to process",
        device_readings_map.len()
    );

    let config_drivers = config.drivers.clone();

    // Execute readings for ModbusTCP devices only
    // Other device types (like SMA HyCon CSV) have their own dedicated commands
    let all_readings = read_modbus_devices(&config_drivers, &device_readings_map).await?;

    log::debug!("Completed reading cycle: {} readings", all_readings.len());

    Ok(all_readings)
}

/// Organize readings by device, filtering for enabled devices, and obeying min_read_interval
async fn organize_readings_by_device(
    reading_timestamp: DateTime<Utc>,
    config: &Config,
) -> Result<HashMap<String, DeviceReadingJob>> {
    let mut dev_read_job_map: HashMap<String, DeviceReadingJob> = HashMap::new();
    let cache = KVDb::new(kvpath::SQLITE_CACHE.as_path())?;

    // Iterate through all configured field readings
    for (reading_name, reading_config) in &config.readings {
        // Get the device this reading refers to
        let device_key = &reading_config.device;
        let Some(device) = config.devices.get(device_key) else {
            log::error!(
                "Reading '{}' references unknown device '{}'",
                reading_name,
                device_key
            );
            continue;
        };

        // Skip explicitly disabled devices
        if !device.enabled {
            continue;
        }

        // Skip device if min_read_interval not met
        if let Some(min_read_interval) = device.min_read_interval
            && !min_read_interval_elapsed(device_key, min_read_interval, reading_timestamp, &cache)
        {
            log::debug!(
                "Skipping device '{}'; min_read_interval of {}s not met",
                device_key,
                min_read_interval
            );
            continue;
        }

        // Add variable name to device reading job map
        dev_read_job_map
            .entry(device_key.clone())
            .or_insert_with(|| {
                DeviceReadingJob::new_from_device_and_key(device, device_key.clone())
            })
            .field_names
            .push(reading_config.var.clone());
    }

    // Iterate through all configured status readings
    for status_reading in &config.status_readings {
        let device_key = &status_reading.d;
        let Some(device) = config.devices.get(device_key) else {
            log::error!("Status reading references unknown device '{}'", device_key);
            continue;
        };

        // Skip explicitly disabled devices
        if !device.enabled {
            continue;
        }

        // Skip device if min_read_interval not met
        if let Some(min_read_interval) = device.min_read_interval
            && !min_read_interval_elapsed(device_key, min_read_interval, reading_timestamp, &cache)
        {
            log::debug!(
                "Skipping device '{}'; min_read_interval of {}s not met",
                device_key,
                min_read_interval
            );
            continue;
        }

        // Add status info name to device reading job map
        dev_read_job_map
            .entry(device_key.clone())
            .or_insert_with(|| {
                DeviceReadingJob::new_from_device_and_key(device, device_key.clone())
            })
            .status_info_names
            .push(status_reading.r.clone());
    }

    log::trace!("Assembled device reading job map: {:?}", dev_read_job_map);

    Ok(dev_read_job_map)
}

/// Process all ModbusTCP devices
async fn read_modbus_devices(
    config_drivers: &HashMap<String, DriverSchema>,
    dev_read_job_map: &HashMap<String, DeviceReadingJob>,
) -> Result<Vec<DeviceReading>> {
    // Filter ModbusTCP devices
    let modbus_devices: Vec<_> = dev_read_job_map
        .iter()
        .filter(|(_, dev_read_job)| dev_read_job.device.reading_type == ReadingType::Modbustcp)
        .collect();

    if modbus_devices.is_empty() {
        return Ok(Vec::new());
    }

    log::info!("Processing {} ModbusTCP device(s)", modbus_devices.len());

    // Spawn a task for each device (all run in parallel)
    // Mutex ensures devices sharing the same physical hardware read sequentially
    let reading_tasks = modbus_devices
        .into_iter()
        .map(|(_, dev_read_job)| spawn_device_reading_job(dev_read_job.clone(), config_drivers))
        .collect::<Vec<_>>();

    // Execute all tasks with timeout and collect results
    let timeout_duration = Duration::from_secs(60);
    let results = tokio::time::timeout(timeout_duration, futures::future::join_all(reading_tasks))
        .await?
        .into_iter()
        .filter_map(|result| match result {
            Ok(reading) => Some(reading),
            Err(e) => {
                log::warn!("ModbusTCP reading task failed: {}", e);
                None
            }
        })
        .collect();

    Ok(results)
}

/// Spawn a task to read a single device, using a mutex to prevent concurrent reads of the same physical device
fn spawn_device_reading_job(
    dev_read_job: DeviceReadingJob,
    config_drivers: &HashMap<String, DriverSchema>,
) -> tokio::task::JoinHandle<DeviceReading> {
    let config_drivers = config_drivers.clone();

    tokio::spawn(async move {
        // Create device-level span for this reading operation
        let span = tracing::info_span!(
            "read_device",
            device_key = %dev_read_job.device.key,
        );
        let _enter = span.enter();

        // Get or create a mutex for this physical device
        let lock = get_device_lock(PhysicalDeviceId::from_device(&dev_read_job.device)).await;

        // Acquire the lock - this ensures only one task reads this physical device at a time
        let _guard = lock.lock().await;

        log::debug!(
            "Acquired lock for physical device, reading '{}'",
            dev_read_job.device.key
        );

        // Perform the actual read
        match read_single_device(&dev_read_job, &config_drivers).await {
            Ok(reading) => reading,
            Err(e) => {
                log::warn!("Device reading failed: {}", e);
                DeviceReading::from_device(&dev_read_job.device)
            }
        }
        // Lock is automatically released when _guard is dropped
    })
}

/// Get or create a mutex for a physical device
async fn get_device_lock(physical_id: PhysicalDeviceId) -> Arc<Mutex<()>> {
    let mut locks = DEVICE_LOCKS.lock().await;
    locks
        .entry(physical_id)
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone()
}

fn min_read_interval_elapsed(
    device_key: &str,
    min_read_interval: i64,
    reading_timestamp: DateTime<Utc>,
    cache: &KVDb,
) -> bool {
    let cache_key = format!("{}/{}", keys::LAST_READING_TS_FOR_DEV_PFX, device_key);
    let last_timestamp: i64 = match cache.get(&cache_key) {
        Ok(value) => value.unwrap_or(0),
        Err(e) => {
            log::error!(
                "Error obtaining last reading timestamp for device '{}': {}",
                device_key,
                e
            );
            return true;
        }
    };
    let elapsed = reading_timestamp.timestamp() - last_timestamp;
    elapsed >= min_read_interval
}

/// Read a single ModbusTCP device
async fn read_single_device(
    dev_read_job: &DeviceReadingJob,
    config_drivers: &HashMap<String, DriverSchema>,
) -> Result<DeviceReading> {
    let driver = load_driver(config_drivers, &dev_read_job.device.driver).map_err(|e| {
        anyhow!(
            "Failed to load driver '{}': {}",
            dev_read_job.device.driver,
            e
        )
    })?;

    modbus_tcp::read_device(dev_read_job, &driver)
        .await
        .map_err(|e| {
            anyhow!(
                "ModbusTCP device '{}' reading failed: {}",
                dev_read_job.device.key,
                e
            )
        })
}

/// Represents the readings for a single device
///
/// This is used to organize the readings by device and pass to the ModbusTCP reader.
#[derive(Clone, Debug)]
pub struct DeviceReadingJob {
    pub device: Device,
    pub field_names: Vec<String>,
    pub status_info_names: Vec<String>,
}

impl DeviceReadingJob {
    /// Create a new DeviceReadingJob; device does not need to have a key set yet
    pub fn new_from_device_and_key(device: &Device, device_key: String) -> Self {
        Self {
            device: Device {
                key: device_key,
                ..device.clone()
            },
            field_names: Vec::new(),
            status_info_names: Vec::new(),
        }
    }
}

/// Identifies a unique physical device by host/mac and port
///
/// Two devices are considered the same physical device if they have:
/// - The same MAC address (if both have MAC), OR
/// - The same host (if at least one doesn't have MAC)
/// - AND the same port
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct PhysicalDeviceId {
    /// Normalized identifier: MAC address (if available) or host
    /// MAC takes precedence as it's more reliable
    mac_or_host: String,
    /// Port number
    port: Option<i64>,
}

impl PhysicalDeviceId {
    fn from_device(device: &Device) -> Self {
        let address = device.address.as_ref();

        // Prefer MAC address as primary identifier, fall back to host
        let mac_or_host = if let Some(mac) = address.and_then(|a| a.mac.as_ref()) {
            // Normalize MAC address: uppercase, remove separators
            format!(
                "mac:{}",
                mac.chars()
                    .filter(|c| c.is_alphanumeric())
                    .collect::<String>()
                    .to_lowercase()
            )
        } else if let Some(host) = address.and_then(|a| a.host.as_ref()) {
            // Normalize host: lowercase
            format!("host:{}", host.to_lowercase())
        } else {
            // No identifier available - use device key as fallback
            format!("key:{}", device.key)
        };

        let port = address.and_then(|a| a.port);

        Self { mac_or_host, port }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_organize_readings_empty_config() {
        let config_json = serde_json::json!({
            "devices": {},
            "readings": {},
            "read_interval": 60,
            "read_roundtime": false
        });
        let config: Config = serde_json::from_value(config_json).unwrap();

        let result = organize_readings_by_device(DateTime::from_timestamp(0, 0).unwrap(), &config)
            .await
            .unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_organize_readings_with_devices() {
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

        let result = organize_readings_by_device(DateTime::from_timestamp(0, 0).unwrap(), &config)
            .await
            .unwrap();

        assert_eq!(result.len(), 1);
        assert!(result.contains_key("test_device"));

        let dev_read_job = &result["test_device"];
        assert_eq!(dev_read_job.device.key, "test_device");
        assert_eq!(dev_read_job.field_names.len(), 2);
        assert_eq!(dev_read_job.status_info_names.len(), 0);

        assert!(dev_read_job.field_names.contains(&"voltage_L1".to_string()));
        assert!(
            dev_read_job
                .field_names
                .contains(&"power_total".to_string())
        );
    }

    #[test]
    fn test_physical_device_id_same_host_and_port() {
        use crate::node_mgmt::config::DeviceAddress;

        // Two devices with same host and port should have same PhysicalDeviceId
        let device1 = Device {
            key: "device1".to_string(),
            driver: "modbus_tcp".to_string(),
            reading_type: ReadingType::Modbustcp,
            vendor_id: "test-123".to_string(),
            enabled: true,
            address: Some(DeviceAddress {
                host: Some("192.168.1.100".to_string()),
                port: Some(502),
                unit_id: Some(1),
                mac: None,
                register_offset: None,
                base_url: None,
                baudrate: None,
                device: None,
                slaveaddr: None,
                timezone: None,
            }),
            device_model: None,
            name: None,
            timeout: None,
            min_read_interval: None,
        };

        let device2 = Device {
            key: "device2".to_string(),
            address: Some(DeviceAddress {
                unit_id: Some(2),
                ..device1.address.clone().unwrap()
            }),
            ..device1.clone()
        };

        let id1 = PhysicalDeviceId::from_device(&device1);
        let id2 = PhysicalDeviceId::from_device(&device2);

        assert_eq!(id1, id2, "Same host and port should produce same ID");
    }

    #[test]
    fn test_physical_device_id_different_hosts() {
        use crate::node_mgmt::config::DeviceAddress;

        // Two devices with different hosts should have different PhysicalDeviceId
        let device1 = Device {
            key: "device1".to_string(),
            driver: "modbus_tcp".to_string(),
            reading_type: ReadingType::Modbustcp,
            vendor_id: "test-123".to_string(),
            enabled: true,
            address: Some(DeviceAddress {
                host: Some("192.168.1.100".to_string()),
                port: Some(502),
                unit_id: Some(1),
                mac: None,
                register_offset: None,
                base_url: None,
                baudrate: None,
                device: None,
                slaveaddr: None,
                timezone: None,
            }),
            device_model: None,
            name: None,
            timeout: None,
            min_read_interval: None,
        };

        let device2 = Device {
            key: "device2".to_string(),
            address: Some(DeviceAddress {
                host: Some("192.168.1.101".to_string()),
                ..device1.address.clone().unwrap()
            }),
            ..device1.clone()
        };

        let id1 = PhysicalDeviceId::from_device(&device1);
        let id2 = PhysicalDeviceId::from_device(&device2);

        assert_ne!(id1, id2, "Different hosts should produce different IDs");
    }

    #[test]
    fn test_physical_device_id_mac_priority() {
        use crate::node_mgmt::config::DeviceAddress;

        // MAC address should be used as identifier even with different hosts
        let device1 = Device {
            key: "device1".to_string(),
            driver: "modbus_tcp".to_string(),
            reading_type: ReadingType::Modbustcp,
            vendor_id: "test-123".to_string(),
            enabled: true,
            address: Some(DeviceAddress {
                host: Some("192.168.1.100".to_string()),
                mac: Some("E8:A4:C1:05:19:B2".to_string()),
                port: Some(502),
                unit_id: Some(1),
                register_offset: None,
                base_url: None,
                baudrate: None,
                device: None,
                slaveaddr: None,
                timezone: None,
            }),
            device_model: None,
            name: None,
            timeout: None,
            min_read_interval: None,
        };

        let device2 = Device {
            key: "device2".to_string(),
            address: Some(DeviceAddress {
                host: Some("inverter.local".to_string()),
                mac: Some("e8:a4:c1:05:19:b2".to_string()),
                ..device1.address.clone().unwrap()
            }),
            ..device1.clone()
        };

        let id1 = PhysicalDeviceId::from_device(&device1);
        let id2 = PhysicalDeviceId::from_device(&device2);

        assert_eq!(
            id1, id2,
            "Same MAC should produce same ID regardless of host"
        );
    }

    #[test]
    fn test_physical_device_id_different_ports() {
        use crate::node_mgmt::config::DeviceAddress;

        // Same host but different ports = different physical devices
        let device1 = Device {
            key: "device1".to_string(),
            driver: "modbus_tcp".to_string(),
            reading_type: ReadingType::Modbustcp,
            vendor_id: "test-123".to_string(),
            enabled: true,
            address: Some(DeviceAddress {
                host: Some("192.168.1.100".to_string()),
                port: Some(502),
                unit_id: Some(1),
                mac: None,
                register_offset: None,
                base_url: None,
                baudrate: None,
                device: None,
                slaveaddr: None,
                timezone: None,
            }),
            device_model: None,
            name: None,
            timeout: None,
            min_read_interval: None,
        };

        let device2 = Device {
            key: "device2".to_string(),
            address: Some(DeviceAddress {
                port: Some(503),
                ..device1.address.clone().unwrap()
            }),
            ..device1.clone()
        };

        let id1 = PhysicalDeviceId::from_device(&device1);
        let id2 = PhysicalDeviceId::from_device(&device2);

        assert_ne!(id1, id2, "Different ports should produce different IDs");
    }
}
