pub mod client;
pub mod config;
pub mod defaults;

use anyhow::Result;

use crate::{
    data_mgmt::models::{DeviceReading, DeviceRef, Record},
    node_mgmt::{config::Device, drivers::DriverSchema},
};

// Re-export main types for easier access
pub use client::ModbusTcpReader;
pub use config::{ModbusDeviceConfig, ReadingConfig};

/// Read all configured readings from a single ModbusTCP device
///
/// This function follows the pattern established by the SMA reader, taking a device
/// and reading requests, then returning DeviceReading results.
pub async fn read_device(
    device: &Device,
    driver: &DriverSchema,
    variable_names: &[String],
) -> Result<Option<DeviceReading>> {
    if variable_names.is_empty() {
        log::debug!("No readings requested for ModbusTCP device: {}", device.key);
        return Ok(None);
    }

    // Create Modbus device config from the device configuration
    let device_config = ModbusDeviceConfig::from_config(&device.key, device)?;

    // Convert variable names to ReadingConfig format
    let reading_configs = get_reading_configs_from_variable_names(device, driver, variable_names)?;

    log::debug!(
        "[{}] Reading {} variables from ModbusTCP device at {}:{}",
        device.key,
        reading_configs.len(),
        device_config.host,
        device_config.port
    );

    // Connect to the device
    let mut client = ModbusTcpReader::connect(
        device.key.clone(),
        &device_config.host,
        device_config.port,
        device_config.unit_id,
        device_config.register_offset,
        device_config.timeout,
    )
    .await?;

    // Execute readings
    let readings = client.execute_readings(reading_configs).await?;

    if readings.is_empty() {
        log::warn!(
            "[{}] No successful readings from ModbusTCP device",
            device.key,
        );
    } else {
        log::info!(
            "[{}] Successfully read {} variables from ModbusTCP device",
            device.key,
            readings.len(),
        );
    }

    // Convert to DeviceReading format using Record
    // We do want to return an empty device payload even if all readings failed
    let mut record = Record::new();
    for reading in readings {
        record.set_field(reading.field, reading.value);
    }

    let device_reading = DeviceReading {
        device: DeviceRef::from_device(device),
        record,
    };

    Ok(Some(device_reading))
}

/// Convert variable names to ReadingConfig objects using driver information
fn get_reading_configs_from_variable_names(
    device: &Device,
    driver: &DriverSchema,
    variable_names: &[String],
) -> Result<Vec<ReadingConfig>> {
    let mut reading_configs = Vec::new();

    for variable_name in variable_names {
        match ReadingConfig::from_driver_field(variable_name, driver) {
            Ok(reading_config) => {
                reading_configs.push(reading_config);
            }
            Err(e) => {
                log::error!(
                    "Failed to create reading config for '{}': {}",
                    variable_name,
                    e
                );
            }
        }
    }

    if reading_configs.is_empty() {
        log::error!(
            "No valid reading configurations found for device: {}",
            device.key
        );
    }

    Ok(reading_configs)
}
