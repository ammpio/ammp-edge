pub mod client;
pub mod config;

use anyhow::{Result, anyhow};

use crate::{
    data_mgmt::models::{DeviceReading, Record},
    data_mgmt::readings::ReadingRequest,
    node_mgmt::config::Device,
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
    device_id: &str,
    device: &Device,
    reading_requests: &[ReadingRequest],
) -> Result<Vec<DeviceReading>> {
    if reading_requests.is_empty() {
        log::debug!("No readings requested for ModbusTCP device: {}", device_id);
        return Ok(Vec::new());
    }

    // Create device config from the device configuration
    let device_config = ModbusDeviceConfig::from_config(device_id, device)?;

    // Convert reading requests to ReadingConfig format
    let reading_configs = convert_reading_requests_to_configs(config, reading_requests, device)?;

    log::debug!(
        "Reading {} variables from ModbusTCP device '{}' at {}:{}",
        reading_configs.len(),
        device_id,
        device_config.host,
        device_config.port
    );

    // Connect to the device
    let mut client = ModbusTcpReader::connect(
        device_id.to_string(),
        &device_config.host,
        device_config.port,
        device_config.unit_id,
        Some(device_config.timeout),
    )
    .await?;

    // Execute readings
    let readings = client.execute_readings(reading_configs).await?;

    if readings.is_empty() {
        log::warn!(
            "No successful readings from ModbusTCP device: {}",
            device_id
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
        device_id
    );

    Ok(vec![device_reading])
}

/// Convert ReadingRequest objects to ReadingConfig objects using driver information
fn convert_reading_requests_to_configs(
    config: &crate::node_mgmt::config::Config,
    reading_requests: &[ReadingRequest],
    device: &Device,
) -> Result<Vec<ReadingConfig>> {
    use crate::data_mgmt::drivers::{load_driver, resolve_field_definition};

    let mut reading_configs = Vec::new();

    // Load driver definition for this device
    let driver = load_driver(config, &device.driver)
        .map_err(|e| anyhow!("Failed to load driver '{}': {}", device.driver, e))?;

    for request in reading_requests {
        // Resolve field definition from driver
        let field_def = resolve_field_definition(&driver, &request.variable_name)
            .map_err(|e| anyhow!("Failed to resolve field '{}': {}", request.variable_name, e))?;

        let reading_config = ReadingConfig {
            variable_name: request.variable_name.clone(),
            register: field_def.register.unwrap(), // Already validated in resolve_field_definition
            word_count: field_def.words,
            datatype: field_def.datatype,
            function_code: field_def.fncode,
            multiplier: field_def.multiplier.unwrap_or(1.0),
            offset: field_def.offset.unwrap_or(0.0),
            unit: field_def.unit,
            valuemap: field_def.valuemap,
        };

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
