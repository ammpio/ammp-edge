use anyhow::{Result, anyhow};
use std::net::ToSocketAddrs;
use tokio::time::Duration;
use tokio_modbus::prelude::*;

use super::config::ReadingConfig;
use crate::data_mgmt::models::{Reading, RtValue};

/// ModbusTCP client for reading device registers
pub struct ModbusTcpReader {
    context: tokio_modbus::client::Context,
    device_id: String,
    unit_id: u8,
}

impl ModbusTcpReader {
    /// Connect to a ModbusTCP device
    pub async fn connect(
        device_id: String,
        host: &str,
        port: u16,
        unit_id: u8,
        _timeout: Option<Duration>,
    ) -> Result<Self> {
        let socket_addr = (host, port)
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| anyhow!("Failed to resolve hostname: {}", host))?;

        log::debug!(
            "Connecting to ModbusTCP device at {}/{}",
            socket_addr,
            unit_id
        );

        let ctx = tcp::connect_slave(socket_addr, Slave(unit_id))
            .await
            .map_err(|e| {
                anyhow::anyhow!(
                    "Failed to connect to ModbusTCP device {}/{}: {}",
                    socket_addr,
                    unit_id,
                    e
                )
            })?;

        log::info!("Connected to ModbusTCP device {}/{}", socket_addr, unit_id);

        Ok(ModbusTcpReader {
            context: ctx,
            device_id,
            unit_id,
        })
    }

    /// Read raw register values from the device
    pub async fn read_registers(
        &mut self,
        register: u16,
        count: u16,
        function_code: u8,
    ) -> Result<Vec<u16>> {
        log::debug!(
            "Reading {} registers from address {} with function code {}",
            count,
            register,
            function_code
        );

        let result = match function_code {
            3 => self.context.read_holding_registers(register, count).await,
            4 => self.context.read_input_registers(register, count).await,
            _ => {
                return Err(anyhow!(
                    "Unsupported ModbusTCP function code: {}",
                    function_code
                ));
            }
        };

        let registers = result
            .map_err(|e| {
                anyhow!(
                    "ModbusTCP connection error for register {}: {}",
                    register,
                    e
                )
            })?
            .map_err(|e| anyhow!("ModbusTCP protocol error for register {}: {}", register, e))?;

        log::debug!(
            "Successfully read {} registers: {:?}",
            registers.len(),
            registers
        );
        Ok(registers)
    }

    /// Execute multiple reading configurations and return processed readings
    pub async fn execute_readings(
        &mut self,
        reading_configs: Vec<ReadingConfig>,
    ) -> Result<Vec<Reading>> {
        let mut readings = Vec::new();

        for config in reading_configs {
            match self.read_single_value(&config).await {
                Ok(value) => {
                    readings.push(Reading {
                        field: config.variable_name.clone(),
                        value: RtValue::Float(value),
                    });
                    log::debug!("Successfully read {}: {}", config.variable_name, value);
                }
                Err(e) => {
                    log::warn!("Failed to read variable '{}': {}", config.variable_name, e);
                    // Continue with other readings even if one fails
                }
            }

            // Note: read_delay_ms was removed from the new ReadingConfig
            // If needed, this can be added back as a global or device-level setting
        }

        Ok(readings)
    }

    /// Read and process a single value according to its configuration
    async fn read_single_value(&mut self, config: &ReadingConfig) -> Result<f64> {
        // Read raw register values
        let raw_registers = self
            .read_registers(config.register, config.words, config.fncode)
            .await?;

        // Use the new parsing method from ReadingConfig
        let bytes = registers_to_bytes(&raw_registers, None)?; // byte_order removed for now
        let scaled_value = config.parse_raw_bytes(&bytes)?;

        Ok(scaled_value)
    }

    /// Get device ID for this reader
    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    /// Get unit ID for this reader
    pub fn unit_id(&self) -> u8 {
        self.unit_id
    }
}

/// Convert register values to bytes, handling byte order
fn registers_to_bytes(registers: &[u16], byte_order: Option<&str>) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();

    // Handle register order (default is MSB first)
    let ordered_registers: Vec<u16> = match byte_order {
        Some("lsr") => {
            // Least Significant Register first - reverse register order
            registers.iter().rev().cloned().collect()
        }
        _ => {
            // Default: Most Significant Register first
            registers.to_vec()
        }
    };

    // Convert each register to bytes (always big-endian within register)
    for register in ordered_registers {
        bytes.extend_from_slice(&register.to_be_bytes());
    }

    Ok(bytes)
}

/// Parse register bytes according to data type specification
/// NOTE: This function is now replaced by ReadingConfig::parse_raw_bytes()
#[allow(dead_code)]
fn parse_register_value(bytes: &[u8], datatype: &str) -> Result<f64> {
    use byteorder::{BigEndian, ReadBytesExt};
    use std::io::Cursor;

    if bytes.is_empty() {
        return Err(anyhow::anyhow!("No bytes to parse"));
    }

    let mut cursor = Cursor::new(bytes);

    let value = match datatype {
        "uint16" => {
            if bytes.len() < 2 {
                return Err(anyhow::anyhow!("Insufficient bytes for uint16"));
            }
            cursor.read_u16::<BigEndian>()? as f64
        }
        "int16" => {
            if bytes.len() < 2 {
                return Err(anyhow::anyhow!("Insufficient bytes for int16"));
            }
            cursor.read_i16::<BigEndian>()? as f64
        }
        "uint32" => {
            if bytes.len() < 4 {
                return Err(anyhow::anyhow!("Insufficient bytes for uint32"));
            }
            cursor.read_u32::<BigEndian>()? as f64
        }
        "int32" => {
            if bytes.len() < 4 {
                return Err(anyhow::anyhow!("Insufficient bytes for int32"));
            }
            cursor.read_i32::<BigEndian>()? as f64
        }
        "uint64" => {
            if bytes.len() < 8 {
                return Err(anyhow::anyhow!("Insufficient bytes for uint64"));
            }
            cursor.read_u64::<BigEndian>()? as f64
        }
        "float" | "single" => {
            if bytes.len() < 4 {
                return Err(anyhow::anyhow!("Insufficient bytes for float"));
            }
            cursor.read_f32::<BigEndian>()? as f64
        }
        "double" => {
            if bytes.len() < 8 {
                return Err(anyhow::anyhow!("Insufficient bytes for double"));
            }
            cursor.read_f64::<BigEndian>()?
        }
        _ => {
            return Err(anyhow::anyhow!("Unsupported datatype: {}", datatype));
        }
    };

    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registers_to_bytes_default_order() {
        let registers = vec![0x1234, 0x5678];
        let bytes = registers_to_bytes(&registers, None).unwrap();
        assert_eq!(bytes, vec![0x12, 0x34, 0x56, 0x78]);
    }

    #[test]
    fn test_registers_to_bytes_lsr_order() {
        let registers = vec![0x1234, 0x5678];
        let bytes = registers_to_bytes(&registers, Some("lsr")).unwrap();
        assert_eq!(bytes, vec![0x56, 0x78, 0x12, 0x34]);
    }

    #[test]
    fn test_parse_register_value_uint16() {
        let bytes = vec![0x12, 0x34];
        let value = parse_register_value(&bytes, "uint16").unwrap();
        assert_eq!(value, 0x1234 as f64);
    }

    #[test]
    fn test_parse_register_value_int32() {
        let bytes = vec![0xFF, 0xFF, 0xFF, 0xFF]; // -1 as int32
        let value = parse_register_value(&bytes, "int32").unwrap();
        assert_eq!(value, -1.0);
    }

    #[test]
    fn test_parse_register_value_float() {
        // IEEE 754 representation of 1.0
        let bytes = vec![0x3F, 0x80, 0x00, 0x00];
        let value = parse_register_value(&bytes, "float").unwrap();
        assert!((value - 1.0).abs() < 0.001);
    }
}
