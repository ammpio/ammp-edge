pub mod client;
pub mod config;
pub mod defaults;

use anyhow::{Result, anyhow};

use crate::{
    data_mgmt::models::{DeviceReading, Record},
    node_mgmt::config::Device,
    node_mgmt::drivers::load_driver,
};

// Re-export main types for easier access
pub use client::ModbusTcpReader;
pub use config::{ModbusDeviceConfig, ReadingConfig};

/// Read all configured readings from a single ModbusTCP device
///
/// This function follows the pattern established by the SMA reader, taking a device
/// and reading requests, then returning DeviceReading results.
pub async fn read_device(
    config: &crate::node_mgmt::config::Config,
    device: &Device,
    variable_names: &[String],
) -> Result<Vec<DeviceReading>> {
    if variable_names.is_empty() {
        log::debug!("No readings requested for ModbusTCP device: {}", device.key);
        return Ok(Vec::new());
    }

    // Create Modbus device config from the device configuration
    let device_config = ModbusDeviceConfig::from_config(&device.key, device)?;

    // Convert variable names to ReadingConfig format
    let reading_configs = convert_variable_names_to_configs(config, variable_names, device)?;

    log::debug!(
        "Reading {} variables from ModbusTCP device '{}' at {}:{}",
        reading_configs.len(),
        device.key,
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
            "No successful readings from ModbusTCP device: {}",
            device.key
        );
        return Ok(Vec::new());
    }

    // Convert to DeviceReading format using Record
    let mut record = Record::new();
    for reading in readings {
        record.set_field(reading.field, reading.value);
    }

    let device_reading = DeviceReading {
        device: device.clone(),
        record,
    };

    log::info!(
        "Successfully read {} variables from ModbusTCP device \"{}\"",
        device_reading.record.all_fields().len(),
        device.key
    );

    Ok(vec![device_reading])
}

/// Convert variable names to ReadingConfig objects using driver information
fn convert_variable_names_to_configs(
    config: &crate::node_mgmt::config::Config,
    variable_names: &[String],
    device: &Device,
) -> Result<Vec<ReadingConfig>> {
    let mut reading_configs = Vec::new();

    // Load driver definition for this device
    let driver = load_driver(config, &device.driver)
        .map_err(|e| anyhow!("Failed to load driver '{}': {}", device.driver, e))?;

    for variable_name in variable_names {
        // Use the simplified ReadingConfig creation
        let reading_config =
            ReadingConfig::from_driver_field(variable_name, &driver).map_err(|e| {
                anyhow!(
                    "Failed to create reading config for '{}': {}",
                    variable_name,
                    e
                )
            })?;

        reading_configs.push(reading_config);
    }

    if reading_configs.is_empty() {
        log::warn!(
            "No valid reading configurations found for device: {}",
            device.key
        );
    }

    Ok(reading_configs)
}
