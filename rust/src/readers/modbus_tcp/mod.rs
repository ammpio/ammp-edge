pub mod client;
pub mod config;
pub mod defaults;

use anyhow::Result;

use crate::{
    data_mgmt::models::{DeviceReading, DeviceRef, Record},
    data_mgmt::readings::DeviceReadingJob,
    node_mgmt::drivers::DriverSchema,
};

// Re-export main types for easier access
pub use client::ModbusTcpReader;
pub use config::{FieldReadingConfig, ModbusDeviceConfig, ModbusReading, StatusReadingConfig};

/// Read all configured readings from a single ModbusTCP device
///
/// This function follows the pattern established by the SMA reader, taking a device
/// and reading requests, then returning DeviceReading results.
pub async fn read_device(
    dev_read_job: &DeviceReadingJob,
    driver: &DriverSchema,
) -> Result<DeviceReading> {
    if dev_read_job.field_names.is_empty() && dev_read_job.status_info_names.is_empty() {
        log::debug!(
            "No readings requested for ModbusTCP device: {}",
            dev_read_job.device.key
        );
        return Ok(DeviceReading::from_device(&dev_read_job.device));
    }

    // Create Modbus device config from the device configuration
    let device_config = ModbusDeviceConfig::from_device(&dev_read_job.device)?;

    // Convert variable names to FieldReadingConfig format
    let mut field_configs = Vec::new();
    for field_name in &dev_read_job.field_names {
        match FieldReadingConfig::from_driver_field(field_name, driver) {
            Ok(config) => field_configs.push(config),
            Err(e) => {
                log::error!(
                    "Failed to create reading config for '{}': {}",
                    field_name,
                    e
                );
            }
        }
    }

    // Convert status info names to StatusReadingConfig format
    let mut status_info_configs = Vec::new();
    for status_info_name in &dev_read_job.status_info_names {
        match StatusReadingConfig::from_driver_status_info(status_info_name, driver) {
            Ok(config) => status_info_configs.push(config),
            Err(e) => {
                log::error!(
                    "Failed to create status info config for '{}': {}",
                    status_info_name,
                    e
                );
            }
        }
    }

    log::debug!(
        "Reading {} fields and {} status info from ModbusTCP device {} at {}:{}",
        field_configs.len(),
        status_info_configs.len(),
        dev_read_job.device.key,
        device_config.host,
        device_config.port
    );

    // Connect to the device
    let mut client = ModbusTcpReader::connect(&device_config).await?;

    // Execute all readings (both fields and status info)
    let (readings, status_readings) = client
        .execute_readings(field_configs, status_info_configs)
        .await?;

    if readings.is_empty() && status_readings.is_empty() {
        log::warn!(
            "[{}] No successful readings from ModbusTCP device",
            device_config.device_key,
        );
    } else {
        log::info!(
            "Successfully read {} fields and {} status info from ModbusTCP device {}",
            readings.len(),
            status_readings.len(),
            device_config.device_key,
        );
    }

    // Convert to DeviceReading format using Record
    // We do want to return an empty device payload even if all readings failed
    let mut record = Record::new();
    for reading in readings {
        record.set_field(reading.field, reading.value);
    }
    for status_reading in status_readings {
        record.add_status_reading(status_reading);
    }

    let device_reading = DeviceReading {
        device: DeviceRef::from_device(&dev_read_job.device),
        record,
    };

    Ok(device_reading)
}
