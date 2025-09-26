//! ModbusTCP configuration types and utilities
//!
//! This module provides domain-specific wrappers around the generated types
//! from the JSON schemas, adding Modbus-specific functionality and validation.

use anyhow::{Result, anyhow};
use std::time::Duration;

// Import the generated types through domain module re-exports
use crate::node_mgmt::config::{Config, Device, ReadingType};
use crate::node_mgmt::drivers::{DriverSchema, FieldOpts, resolve_field_definition};

/// Configuration for a ModbusTCP device connection
#[derive(Clone, Debug)]
pub struct ModbusDeviceConfig {
    pub device_id: String,
    pub host: String,
    pub port: u16,
    pub unit_id: u8,
    pub timeout: Duration,
    pub register_offset: u16,
}

impl ModbusDeviceConfig {
    /// Create device config from the main configuration
    pub fn from_config(device_id: &str, device: &Device) -> Result<Self> {
        // Ensure this is a ModbusTCP device
        match device.reading_type {
            ReadingType::Modbustcp => {}
            other => {
                return Err(anyhow!(
                    "Device {} has reading type {:?}, not modbustcp",
                    device_id,
                    other
                ));
            }
        }

        let address = device.address.as_ref().ok_or_else(|| {
            anyhow!(
                "ModbusTCP device {} missing address configuration",
                device_id
            )
        })?;

        let host = address
            .host
            .as_ref()
            .ok_or_else(|| anyhow!("ModbusTCP device {} missing host address", device_id))?
            .clone();

        let port = address.port.map(|p| p as u16).unwrap_or(502); // Default ModbusTCP port

        let unit_id = address.unit_id.map(|u| u as u8).unwrap_or(1); // Default unit ID

        let register_offset = address.register_offset.map(|o| o as u16).unwrap_or(0);

        Ok(ModbusDeviceConfig {
            device_id: device_id.to_string(),
            host,
            port,
            unit_id,
            timeout: Duration::from_secs(5), // Default timeout
            register_offset,
        })
    }

    /// Create a test configuration for development
    pub fn test_config(device_id: &str, host: &str, port: u16, unit_id: u8) -> Self {
        ModbusDeviceConfig {
            device_id: device_id.to_string(),
            host: host.to_string(),
            port,
            unit_id,
            timeout: Duration::from_secs(10),
            register_offset: 0,
        }
    }

    /// Get connection parameters as a tuple
    pub fn connection_params(&self) -> (&str, u16, u8) {
        (&self.host, self.port, self.unit_id)
    }
}

/// Configuration for reading a specific field
#[derive(Clone, Debug)]
pub struct ReadingConfig {
    pub variable_name: String,
    pub field_config: FieldOpts,
    pub register: u16,
    pub words: u16,
    pub fncode: u8,
}

impl ReadingConfig {
    /// Parse raw bytes using the configured data processing parameters
    pub fn parse_raw_bytes(&self, bytes: &[u8]) -> Result<f64> {
        use crate::data_mgmt::process::{
            DataType, ParseAs, ProcessingParams, TypeCast, process_reading,
        };

        // Get datatype from field config
        let datatype = self
            .field_config
            .datatype
            .ok_or_else(|| anyhow!("Missing datatype for field {}", self.variable_name))?;

        // Convert to processing enum
        let datatype_enum = datatype.to_string().parse::<DataType>()?;

        // Extract valuemap from datamap field
        let valuemap = if !self.field_config.datamap.is_empty() {
            let mut vm = std::collections::HashMap::new();
            for (key, value) in &self.field_config.datamap {
                if let Some(num_val) = value.as_f64() {
                    vm.insert(key.clone(), num_val);
                }
            }
            if !vm.is_empty() { Some(vm) } else { None }
        } else {
            None
        };

        // Create processing parameters
        let params = ProcessingParams {
            parse_as: ParseAs::Bytes,
            datatype: Some(datatype_enum),
            typecast: Some(TypeCast::Float), // ModbusTCP readings are typically numeric
            valuemap,
            multiplier: self.field_config.multiplier,
            offset: self.field_config.offset,
        };

        // Process the reading
        let processed = process_reading(bytes, &params)?;

        // Extract numeric value
        match processed {
            crate::data_mgmt::process::ProcessedValue::Float(f) => Ok(f),
            crate::data_mgmt::process::ProcessedValue::Int(i) => Ok(i as f64),
            _ => Err(anyhow!("Expected numeric value from ModbusTCP reading")),
        }
    }

    /// Create reading config from driver and field name
    pub fn from_driver_field(variable_name: &str, driver: &DriverSchema) -> Result<Self> {
        // Use the driver system to resolve field configuration
        let field_config = resolve_field_definition(driver, variable_name)?;

        // Validate required fields for ModbusTCP
        let register = field_config
            .register
            .ok_or_else(|| anyhow!("Field {} missing register address", variable_name))?;

        Ok(ReadingConfig {
            variable_name: variable_name.to_string(),
            register: register as u16,
            words: field_config.words.unwrap_or(1) as u16,
            fncode: field_config.fncode as u8,
            field_config,
        })
    }
}

/// Extract ModbusTCP devices from the main configuration
pub fn extract_modbus_devices(
    config: &Config,
) -> Result<Vec<(String, ModbusDeviceConfig, Vec<ReadingConfig>)>> {
    let mut modbus_devices = Vec::new();

    // Filter devices with reading_type = "modbustcp"
    for (device_id, device) in &config.devices {
        if matches!(device.reading_type, ReadingType::Modbustcp) {
            let device_config = ModbusDeviceConfig::from_config(device_id, device)?;
            let reading_configs = extract_device_readings(config, device_id)?;
            modbus_devices.push((device_id.to_string(), device_config, reading_configs));
        }
    }

    Ok(modbus_devices)
}

/// Extract readings for a specific device from configuration
pub fn extract_device_readings(config: &Config, device_id: &str) -> Result<Vec<ReadingConfig>> {
    let mut reading_configs = Vec::new();

    // Get the device to find its driver
    let device = config
        .devices
        .get(device_id)
        .ok_or_else(|| anyhow!("Device {} not found", device_id))?;

    // Get driver configuration
    let driver_config = config.drivers.get(&device.driver);

    // Convert to DriverSchema if available (it's already a DriverSchema now)
    let driver_schema = driver_config;

    // Filter readings that belong to this device
    for reading_schema in config.readings.values() {
        if reading_schema.device == device_id {
            // Try to get field configuration from driver
            if let Some(driver) = &driver_schema {
                match ReadingConfig::from_driver_field(&reading_schema.var, driver) {
                    Ok(reading_config) => {
                        reading_configs.push(reading_config);
                    }
                    Err(e) => {
                        log::warn!(
                            "Failed to create reading config for field {} in driver {} for device {}: {}",
                            reading_schema.var,
                            device.driver,
                            device_id,
                            e
                        );
                    }
                }
            } else {
                log::warn!(
                    "Driver {} not found in configuration for device {}",
                    device.driver,
                    device_id
                );
            }
        }
    }

    Ok(reading_configs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_modbus_device_config_test_config() {
        let config = ModbusDeviceConfig::test_config("test_device", "192.168.1.100", 502, 1);
        assert_eq!(config.host, "192.168.1.100");
        assert_eq!(config.port, 502);
        assert_eq!(config.unit_id, 1);
        assert_eq!(config.timeout, Duration::from_secs(10));
    }

    #[test]
    fn test_reading_config_from_driver() {
        // Create a test driver schema
        let driver_json = json!({
            "common": {
                "fncode": 4,
                "words": 1,
                "datatype": "uint16"
            },
            "fields": {
                "voltage": {
                    "register": 1000,
                    "multiplier": 0.1,
                    "unit": "V"
                },
                "power": {
                    "register": 2000,
                    "words": 2,
                    "datatype": "uint32",
                    "multiplier": 10.0,
                    "unit": "W"
                }
            }
        });

        let driver: DriverSchema = serde_json::from_value(driver_json).unwrap();

        // Test voltage field (inherits from common)
        let voltage_config = ReadingConfig::from_driver_field("voltage", &driver).unwrap();
        assert_eq!(voltage_config.variable_name, "voltage");
        assert_eq!(voltage_config.register, 1000);
        assert_eq!(voltage_config.words, 1); // From common
        assert_eq!(voltage_config.fncode, 4); // From common
        assert_eq!(voltage_config.field_config.unit, Some("V".to_string()));

        // Test power field (overrides common)
        let power_config = ReadingConfig::from_driver_field("power", &driver).unwrap();
        assert_eq!(power_config.variable_name, "power");
        assert_eq!(power_config.register, 2000);
        assert_eq!(power_config.words, 2); // Overridden
        assert_eq!(power_config.fncode, 4); // From common
        assert_eq!(power_config.field_config.unit, Some("W".to_string()));
    }

    #[test]
    fn test_modbus_device_config_connection_params() {
        let config = ModbusDeviceConfig::test_config("test_device", "192.168.1.100", 502, 1);
        let (host, port, unit_id) = config.connection_params();
        assert_eq!(host, "192.168.1.100");
        assert_eq!(port, 502);
        assert_eq!(unit_id, 1);
    }
}
