//! ModbusTCP configuration types and utilities
//!
//! This module provides domain-specific wrappers around the generated types
//! from the JSON schemas, adding Modbus-specific functionality and validation.

use anyhow::{Result, anyhow};
use std::time::Duration;

// Import the generated types through domain module re-exports
use crate::helpers::arp_get_ip_from_mac;
use crate::node_mgmt::config::{Config, Device, DeviceAddress, ReadingType};
use crate::node_mgmt::drivers::{DriverSchema, FieldOpts, resolve_field_definition};

use super::defaults;

/// Configuration for a ModbusTCP device connection
#[derive(Clone, Debug)]
pub struct ModbusDeviceConfig {
    pub device_key: String,
    pub host: String,
    pub port: u16,
    pub unit_id: u8,
    pub timeout: Duration,
    pub register_offset: u16,
}

impl ModbusDeviceConfig {
    /// Create device config from the main configuration
    pub fn from_config(device_key: &str, device: &Device) -> Result<Self> {
        // Ensure this is a ModbusTCP device
        match device.reading_type {
            ReadingType::Modbustcp => {}
            other => {
                return Err(anyhow!(
                    "Device {} has reading type {:?}, not modbustcp",
                    device_key,
                    other
                ));
            }
        }

        let address = device.address.as_ref().ok_or_else(|| {
            anyhow!(
                "ModbusTCP device {} missing address configuration",
                device_key
            )
        })?;

        let host = Self::get_host(device_key, address)?;

        let port = address.port.map(|p| p as u16).unwrap_or(defaults::PORT);

        let unit_id = address
            .unit_id
            .map(|u| u as u8)
            .unwrap_or(defaults::UNIT_ID);

        let register_offset = address
            .register_offset
            .map(|o| o as u16)
            .unwrap_or(defaults::REGISTER_OFFSET);

        let timeout = device
            .timeout
            .map(|t| Duration::from_secs(t as u64))
            .unwrap_or(defaults::TIMEOUT);

        Ok(ModbusDeviceConfig {
            device_key: device_key.to_string(),
            host,
            port,
            unit_id,
            register_offset,
            timeout,
        })
    }

    /// Determine host IP - either from configured host or resolve from MAC
    fn get_host(device_key: &str, address: &DeviceAddress) -> Result<String> {
        // Determine host IP - either from configured host or resolve from MAC
        let host = if let Some(host_ip) = &address.host {
            // Host IP is already configured
            host_ip.clone()
        } else if let Some(mac_addr) = &address.mac {
            // Try to resolve IP from MAC address using ARP table
            log::info!(
                "Resolving IP for MAC address {} on device {}",
                mac_addr,
                device_key
            );
            match arp_get_ip_from_mac(mac_addr) {
                Ok(Some(ip)) => {
                    log::info!("[{}] Resolved MAC {} to IP {}", device_key, mac_addr, ip,);
                    ip
                }
                Ok(None) => {
                    return Err(anyhow!(
                        "[{}] MAC {} not found in ARP table. Device may be offline or not on local network.",
                        device_key,
                        mac_addr
                    ));
                }
                Err(e) => {
                    return Err(anyhow!(
                        "[{}] Failed to resolve MAC {} to IP: {}",
                        device_key,
                        mac_addr,
                        e
                    ));
                }
            }
        } else {
            return Err(anyhow!(
                "[{}] Missing both host IP and MAC address",
                device_key,
            ));
        };

        Ok(host)
    }

    /// Create a test configuration for development
    pub fn test_config(device_key: &str, host: &str, port: u16, unit_id: u8) -> Self {
        ModbusDeviceConfig {
            device_key: device_key.to_string(),
            host: host.to_string(),
            port,
            unit_id,
            timeout: Duration::from_secs(10),
            register_offset: 0,
        }
    }
}

/// Configuration for reading a specific field
#[derive(Clone, Debug)]
pub struct ReadingConfig {
    pub variable_name: String,
    pub field_config: FieldOpts,
    pub fncode: u8,
    pub register: u16,
    pub words: u16,
}

impl ReadingConfig {
    /// Create reading config from driver and field name
    pub fn from_driver_field(variable_name: &str, driver: &DriverSchema) -> Result<Self> {
        // Use the driver system to resolve field configuration
        let field_config = resolve_field_definition(driver, variable_name)?;

        // Validate required fields for ModbusTCP
        let register = field_config
            .register
            .ok_or_else(|| anyhow!("Field {} missing register address", variable_name))?;

        let fncode = field_config
            .fncode
            .ok_or_else(|| anyhow!("Field {} missing fncode (function code)", variable_name))?;

        Ok(ReadingConfig {
            variable_name: variable_name.to_string(),
            fncode,
            register,
            words: field_config.words.map(|w| w.get()).unwrap_or(1) as u16,
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
    for (device_key, device) in &config.devices {
        if matches!(device.reading_type, ReadingType::Modbustcp) {
            let device_config = ModbusDeviceConfig::from_config(device_key, device)?;
            let reading_configs = extract_device_readings(config, device_key)?;
            modbus_devices.push((device_key.to_string(), device_config, reading_configs));
        }
    }

    Ok(modbus_devices)
}

/// Extract readings for a specific device from configuration
pub fn extract_device_readings(config: &Config, device_key: &str) -> Result<Vec<ReadingConfig>> {
    let mut reading_configs = Vec::new();

    // Get the device to find its driver
    let device = config
        .devices
        .get(device_key)
        .ok_or_else(|| anyhow!("Device {} not found", device_key))?;

    // Get driver configuration
    let driver_config = config.drivers.get(&device.driver);

    // Convert to DriverSchema if available (it's already a DriverSchema now)
    let driver_schema = driver_config;

    // Filter readings that belong to this device
    for reading_schema in config.readings.values() {
        if reading_schema.device == device_key {
            // Try to get field configuration from driver
            if let Some(driver) = &driver_schema {
                match ReadingConfig::from_driver_field(&reading_schema.var, driver) {
                    Ok(reading_config) => {
                        reading_configs.push(reading_config);
                    }
                    Err(e) => {
                        log::warn!(
                            "[{}] Failed to create reading config for field {} in driver {}: {}",
                            device_key,
                            reading_schema.var,
                            device.driver,
                            e
                        );
                    }
                }
            } else {
                log::warn!(
                    "[{}] Driver {} not found in configuration",
                    device_key,
                    device.driver,
                );
            }
        }
    }

    Ok(reading_configs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_mgmt::config::DeviceAddress;
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
        let (host, port, unit_id) = (config.host, config.port, config.unit_id);
        assert_eq!(host, "192.168.1.100");
        assert_eq!(port, 502);
        assert_eq!(unit_id, 1);
    }

    #[test]
    fn test_modbus_device_config_with_host_ip() {
        // Test device with configured host IP (should work without MAC resolution)
        let device = Device {
            key: "test_device".to_string(),
            device_model: Some("test_model".to_string()),
            driver: "test_driver".to_string(),
            reading_type: ReadingType::Modbustcp,
            vendor_id: "test_vendor".to_string(),
            enabled: true,
            address: Some(DeviceAddress {
                host: Some("192.168.1.100".to_string()),
                port: Some(502),
                unit_id: Some(1),
                ..Default::default()
            }),
            name: Some("Test Device".to_string()),
            timeout: Some(10),
            min_read_interval: None,
        };

        let config = ModbusDeviceConfig::from_config("test_device", &device).unwrap();
        assert_eq!(config.host, "192.168.1.100");
        assert_eq!(config.port, 502);
        assert_eq!(config.unit_id, 1);
    }

    #[test]
    fn test_modbus_device_config_with_mac_only() {
        // Test device with only MAC address (will fail in test env but shouldn't panic)
        let device = Device {
            key: "test_device".to_string(),
            device_model: Some("test_model".to_string()),
            driver: "test_driver".to_string(),
            reading_type: ReadingType::Modbustcp,
            vendor_id: "test_vendor".to_string(),
            enabled: true,
            address: Some(DeviceAddress {
                mac: Some("aa:bb:cc:dd:ee:ff".to_string()),
                port: Some(502),
                unit_id: Some(1),
                register_offset: Some(0),
                ..Default::default()
            }),
            name: Some("Test Device".to_string()),
            timeout: Some(10),
            min_read_interval: None,
        };

        // This will likely fail since the MAC won't be in ARP table, but test error handling
        let result = ModbusDeviceConfig::from_config("test_device", &device);

        // Should get a meaningful error (either MAC not found or ARP table read failure)
        if let Err(e) = result {
            let error_msg = e.to_string();
            assert!(
                error_msg.contains("not found in ARP table")
                    || error_msg.contains("Unable to load ARP table"),
                "Unexpected error message: {}",
                error_msg
            );
        }
        // If it succeeds (unlikely), that's also fine - means MAC was actually found
    }

    #[test]
    fn test_modbus_device_config_missing_address() {
        // Test device with no address at all
        let device = Device {
            key: "test_device".to_string(),
            device_model: Some("test_model".to_string()),
            driver: "test_driver".to_string(),
            reading_type: ReadingType::Modbustcp,
            vendor_id: "test_vendor".to_string(),
            enabled: true,
            address: None,
            name: Some("Test Device".to_string()),
            timeout: Some(10),
            min_read_interval: None,
        };

        let result = ModbusDeviceConfig::from_config("test_device", &device);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("missing address configuration")
        );
    }

    #[test]
    fn test_modbus_device_config_missing_host_and_mac() {
        // Test device with address but no host or MAC
        let device = Device {
            key: "test_device".to_string(),
            device_model: Some("test_model".to_string()),
            driver: "test_driver".to_string(),
            reading_type: ReadingType::Modbustcp,
            vendor_id: "test_vendor".to_string(),
            enabled: true,
            address: Some(DeviceAddress {
                port: Some(502),
                unit_id: Some(1),
                ..Default::default()
            }),
            name: Some("Test Device".to_string()),
            timeout: Some(10),
            min_read_interval: None,
        };

        let result = ModbusDeviceConfig::from_config("test_device", &device);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Missing both host IP and MAC address")
        );
    }
}
