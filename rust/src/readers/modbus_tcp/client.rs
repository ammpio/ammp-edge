use anyhow::{Result, anyhow};
use std::net::ToSocketAddrs;
use tokio::time::Duration;
use tokio_modbus::prelude::*;

use super::config::ReadingConfig;
use crate::data_mgmt::models::{Reading, RtValue};
use crate::node_mgmt::drivers::RegisterOrder;

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

        // Convert registers to bytes with proper order
        let bytes = registers_to_bytes(&raw_registers, config.field_config.order.as_ref())?;

        // Use the centralized processing system for the complete flow:
        // 1. Register order (already applied above)
        // 2. Valuemap checking
        // 3. Datatype parsing
        // 4. Multiplier/offset application
        // 5. Typecast application
        use crate::data_mgmt::process::{ProcessedValue, process_field_reading};

        let processed = process_field_reading(&bytes, &config.field_config)?;

        // Extract numeric value for ModbusTCP (which should always be numeric)
        let scaled_value = match processed {
            ProcessedValue::Float(f) => f,
            ProcessedValue::Int(i) => i as f64,
            _ => return Err(anyhow!(
                "Expected numeric value from ModbusTCP reading, got: {:?}",
                processed
            )),
        };

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
fn registers_to_bytes(registers: &[u16], order: Option<&RegisterOrder>) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();

    // Handle register order
    let ordered_registers: Vec<u16> = match order {
        Some(RegisterOrder::Lsr) => {
            // Least Significant Register first - reverse register order
            registers.iter().rev().cloned().collect()
        }
        Some(RegisterOrder::Msr) | None => {
            // Most Significant Register first (default)
            registers.to_vec()
        }
    };

    // Convert each register to bytes (always big-endian within register)
    for register in ordered_registers {
        bytes.extend_from_slice(&register.to_be_bytes());
    }

    Ok(bytes)
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
        let bytes = registers_to_bytes(&registers, Some(&RegisterOrder::Lsr)).unwrap();
        assert_eq!(bytes, vec![0x56, 0x78, 0x12, 0x34]);
    }

    #[test]
    fn test_registers_to_bytes_msr_order() {
        let registers = vec![0x1234, 0x5678];
        let bytes = registers_to_bytes(&registers, Some(&RegisterOrder::Msr)).unwrap();
        assert_eq!(bytes, vec![0x12, 0x34, 0x56, 0x78]);
    }

}
