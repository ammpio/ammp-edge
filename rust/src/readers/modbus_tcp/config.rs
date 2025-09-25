//! ModbusTCP configuration types and utilities
//!
//! This module provides domain-specific wrappers around the generated types
//! from the JSON schemas, adding Modbus-specific functionality and validation.

use anyhow::{Result, anyhow};
use std::time::Duration;

// Import the generated types
use derived_models::config::{AmmpEdgeConfiguration as Config, Device, ReadingType};
use derived_models::driver::{DriverSchema, DriverSchemaFieldsValue};

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

/// Configuration for reading a specific register/variable
#[derive(Clone, Debug)]
pub struct ReadingConfig {
    pub variable_name: String,
    pub register: u16,
    pub word_count: u16,
    pub datatype: String,
    pub function_code: u8,
    pub multiplier: f64,
    pub offset: f64,
    pub unit: Option<String>,
}

impl ReadingConfig {
    /// Create reading config from configuration data
    pub fn from_driver_field(
        variable_name: &str,
        field_config: &DriverSchemaFieldsValue,
        common_config: Option<&derived_models::driver::CommonParametersForEachField>,
    ) -> Result<Self> {
        // Get register address
        let register = field_config
            .register
            .ok_or_else(|| anyhow!("Field {} missing register address", variable_name))?
            as u16;

        // Determine datatype from field or common config
        let datatype = field_config
            .datatype
            .or_else(|| common_config.and_then(|c| c.datatype))
            .ok_or_else(|| anyhow!("Field {} missing datatype", variable_name))?;

        let datatype_str = datatype.to_string();

        // Calculate word count based on datatype
        let word_count = Self::calculate_word_count(&datatype_str)?;

        // Get function code
        let function_code = if field_config.fncode != 0 {
            field_config.fncode as u8
        } else if let Some(common) = common_config {
            common.fncode as u8
        } else {
            3 // Default to holding registers
        };

        // Get scaling parameters
        let multiplier = field_config.multiplier.unwrap_or(1.0);
        let offset = field_config.offset.unwrap_or(0.0);

        Ok(ReadingConfig {
            variable_name: variable_name.to_string(),
            register,
            word_count,
            datatype: datatype_str,
            function_code,
            multiplier,
            offset,
            unit: field_config.unit.clone(),
        })
    }

    /// Create a test reading configuration for development
    pub fn test_config(
        variable_name: &str,
        register: u16,
        datatype: &str,
        multiplier: Option<f64>,
    ) -> Result<Self> {
        let word_count = Self::calculate_word_count(datatype)?;

        Ok(ReadingConfig {
            variable_name: variable_name.to_string(),
            register,
            word_count,
            datatype: datatype.to_string(),
            function_code: 3, // Holding registers
            multiplier: multiplier.unwrap_or(1.0),
            offset: 0.0,
            unit: None,
        })
    }

    /// Calculate register range (start, count)
    pub fn register_range(&self) -> (u16, u16) {
        (self.register, self.word_count)
    }

    /// Calculate word count based on datatype
    fn calculate_word_count(datatype: &str) -> Result<u16> {
        match datatype.to_lowercase().as_str() {
            "uint16" | "int16" => Ok(1),
            "uint32" | "int32" | "float" | "single" => Ok(2),
            "uint64" | "int64" | "double" => Ok(4),
            _ => Err(anyhow!("Unsupported datatype: {}", datatype)),
        }
    }

    /// Parse raw bytes according to datatype
    pub fn parse_raw_bytes(&self, bytes: &[u8]) -> Result<f64> {
        use byteorder::{BigEndian, ReadBytesExt};
        use std::io::Cursor;

        if bytes.len() < (self.word_count as usize * 2) {
            return Err(anyhow!(
                "Insufficient bytes for datatype {}: got {}, need {}",
                self.datatype,
                bytes.len(),
                self.word_count * 2
            ));
        }

        let mut cursor = Cursor::new(bytes);

        let raw_value = match self.datatype.to_lowercase().as_str() {
            "uint16" => cursor.read_u16::<BigEndian>()? as f64,
            "int16" => cursor.read_i16::<BigEndian>()? as f64,
            "uint32" => cursor.read_u32::<BigEndian>()? as f64,
            "int32" => cursor.read_i32::<BigEndian>()? as f64,
            "uint64" => cursor.read_u64::<BigEndian>()? as f64,
            "int64" => cursor.read_i64::<BigEndian>()? as f64,
            "float" | "single" => cursor.read_f32::<BigEndian>()? as f64,
            "double" => cursor.read_f64::<BigEndian>()?,
            _ => return Err(anyhow!("Unsupported datatype: {}", self.datatype)),
        };

        // Apply scaling
        let scaled_value = raw_value * self.multiplier + self.offset;
        Ok(scaled_value)
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

    // Convert to DriverSchema if available
    let driver_schema = if let Some(driver_json) = driver_config {
        Some(serde_json::from_value::<DriverSchema>(
            serde_json::Value::Object(driver_json.clone()),
        )?)
    } else {
        None
    };

    // Filter readings that belong to this device
    for (_reading_name, reading_schema) in &config.readings {
        if reading_schema.device == device_id {
            // Try to get field configuration from driver
            if let Some(driver) = &driver_schema {
                if let Some(field_config) = driver.fields.get(&reading_schema.var) {
                    let reading_config = ReadingConfig::from_driver_field(
                        &reading_schema.var,
                        field_config,
                        driver.common.as_ref(),
                    )?;
                    reading_configs.push(reading_config);
                } else {
                    log::warn!(
                        "Field {} not found in driver {} for device {}",
                        reading_schema.var,
                        device.driver,
                        device_id
                    );
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

    #[test]
    fn test_modbus_device_config_test_config() {
        let config = ModbusDeviceConfig::test_config("test_device", "192.168.1.100", 502, 1);
        assert_eq!(config.host, "192.168.1.100");
        assert_eq!(config.port, 502);
        assert_eq!(config.unit_id, 1);
        assert_eq!(config.timeout, Duration::from_secs(10));
    }

    #[test]
    fn test_reading_config_word_count_calculation() {
        let config_u16 = ReadingConfig::test_config("test", 1000, "uint16", None).unwrap();
        assert_eq!(config_u16.word_count, 1);

        let config_u32 = ReadingConfig::test_config("test", 1000, "uint32", None).unwrap();
        assert_eq!(config_u32.word_count, 2);

        let config_float = ReadingConfig::test_config("test", 1000, "float", None).unwrap();
        assert_eq!(config_float.word_count, 2);

        let config_double = ReadingConfig::test_config("test", 1000, "double", None).unwrap();
        assert_eq!(config_double.word_count, 4);
    }

    #[test]
    fn test_reading_config_with_multiplier() {
        let config = ReadingConfig::test_config("voltage", 1000, "uint16", Some(0.1)).unwrap();
        assert_eq!(config.variable_name, "voltage");
        assert_eq!(config.register, 1000);
        assert_eq!(config.multiplier, 0.1);
    }

    #[test]
    fn test_reading_config_word_count_calculation_direct() {
        assert_eq!(ReadingConfig::calculate_word_count("uint16").unwrap(), 1);
        assert_eq!(ReadingConfig::calculate_word_count("int16").unwrap(), 1);
        assert_eq!(ReadingConfig::calculate_word_count("uint32").unwrap(), 2);
        assert_eq!(ReadingConfig::calculate_word_count("int32").unwrap(), 2);
        assert_eq!(ReadingConfig::calculate_word_count("float").unwrap(), 2);
        assert_eq!(ReadingConfig::calculate_word_count("double").unwrap(), 4);
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
