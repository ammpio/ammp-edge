use anyhow::{Result, anyhow};
use backoff::ExponentialBackoffBuilder;
use std::net::ToSocketAddrs;
use tokio::time::Duration;
use tokio_modbus::prelude::*;

use super::config::ModbusDeviceConfig;
use super::config::{FieldReadingConfig, ModbusReading, StatusReadingConfig};
use crate::data_mgmt::models::{Reading, RtValue};
use crate::data_mgmt::process_status_info::process_status_info;
use crate::node_mgmt::drivers::RegisterOrder;
use derived_models::data::StatusReading;

/// ModbusTCP client for reading device registers
pub struct ModbusTcpReader {
    context: tokio_modbus::client::Context,
    register_offset: u16,
    timeout: Duration,
}

impl ModbusTcpReader {
    /// Connect to a ModbusTCP device with retry logic
    pub async fn connect(config: &ModbusDeviceConfig) -> Result<Self> {
        let socket_addr = (config.host.clone(), config.port)
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| anyhow!("Failed to resolve hostname: {}", config.host))?;

        log::debug!(
            "[{}] Connecting to ModbusTCP device at {}/{}",
            config.device_key,
            socket_addr,
            config.unit_id
        );

        let ctx = Self::retry_connect(config, socket_addr).await?;

        log::info!(
            "[{}] Connected to ModbusTCP device at {}/{}",
            config.device_key,
            socket_addr,
            config.unit_id
        );

        Ok(ModbusTcpReader {
            context: ctx,
            register_offset: config.register_offset,
            timeout: config.timeout,
        })
    }

    /// Retry connection with exponential backoff
    async fn retry_connect(
        config: &ModbusDeviceConfig,
        socket_addr: std::net::SocketAddr,
    ) -> Result<tokio_modbus::client::Context> {
        // Configure exponential backoff: 500ms, 1s, 2s for 3 retries
        let backoff = ExponentialBackoffBuilder::new()
            .with_initial_interval(Duration::from_millis(500))
            .with_max_elapsed_time(Some(config.timeout))
            .build();

        let device_key = config.device_key.clone();
        let unit_id = config.unit_id;
        let timeout = config.timeout;

        backoff::future::retry(backoff, || async {
            log::debug!(
                "[{}] Attempting connection to ModbusTCP device at {}/{}",
                device_key,
                socket_addr,
                unit_id
            );

            tokio::time::timeout(timeout, tcp::connect_slave(socket_addr, Slave(unit_id)))
                .await
                .map_err(|_| {
                    backoff::Error::transient(anyhow!(
                        "[{}] Connection timeout after {:?} to ModbusTCP device at {}/{}",
                        device_key,
                        timeout,
                        socket_addr,
                        unit_id
                    ))
                })?
                .map_err(|e| {
                    backoff::Error::transient(anyhow!(
                        "[{}] Failed to connect to ModbusTCP device at {}/{}: {}",
                        device_key,
                        socket_addr,
                        unit_id,
                        e
                    ))
                })
        })
        .await
        .map_err(|e| anyhow!("Failed to connect after retries: {}", e))
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

        let read_future = match function_code {
            3 => self.context.read_holding_registers(register, count),
            4 => self.context.read_input_registers(register, count),
            _ => {
                return Err(anyhow!(
                    "Unsupported ModbusTCP function code: {}",
                    function_code
                ));
            }
        };

        let result = tokio::time::timeout(self.timeout, read_future)
            .await
            .map_err(|_| {
                anyhow!(
                    "Read timeout after {:?} for register {} with function code {}",
                    self.timeout,
                    register,
                    function_code
                )
            })?;

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
    /// Returns (field_readings, status_readings)
    pub async fn execute_readings(
        &mut self,
        field_configs: Vec<FieldReadingConfig>,
        status_info_configs: Vec<StatusReadingConfig>,
    ) -> Result<(Vec<Reading>, Vec<StatusReading>)> {
        let mut field_readings = Vec::new();
        let mut status_readings = Vec::new();

        log::trace!("Executing field readings: {:?}", field_configs);
        log::trace!("Executing status info readings: {:?}", status_info_configs);

        // Process field readings
        for config in field_configs {
            match self.read_raw_registers(&config).await {
                Ok(raw_bytes) => {
                    // Process the raw bytes using the data processing pipeline
                    match crate::data_mgmt::process::process_reading(
                        &raw_bytes,
                        &config.field_config,
                    ) {
                        Ok(RtValue::None) => {
                            log::debug!("Reading '{}' returned no value", config.name);
                            // Skip None values
                        }
                        Ok(value) => {
                            log::debug!("Successfully read {}: {:?}", config.name, value);
                            field_readings.push(Reading {
                                field: config.name.clone(),
                                value,
                            });
                        }
                        Err(e) => {
                            log::warn!("Failed to process reading '{}': {}", config.name, e);
                            // Continue with other readings even if one fails
                        }
                    }
                }
                Err(e) => {
                    log::error!("Failed to read raw data for '{}': {}", config.name, e);
                    // Continue with other readings even if one fails
                }
            }
        }

        // Process status info readings
        for config in status_info_configs {
            match self.read_raw_registers(&config).await {
                Ok(raw_bytes) => {
                    // Process the raw bytes using the status info processing pipeline
                    match process_status_info(&raw_bytes, &config.status_info_config) {
                        Ok(status_reading) => {
                            log::debug!(
                                "Successfully read status info {}: {:?}",
                                config.name,
                                status_reading
                            );
                            status_readings.push(status_reading);
                        }
                        Err(e) => {
                            log::warn!("Failed to process status info '{}': {}", config.name, e);
                            // Continue with other readings even if one fails
                        }
                    }
                }
                Err(e) => {
                    log::error!("Failed to read raw data for '{}': {}", config.name, e);
                    // Continue with other readings even if one fails
                }
            }
        }

        Ok((field_readings, status_readings))
    }

    /// Read raw register values and convert them to bytes according to configuration
    /// Generic over any type implementing ModbusReading
    async fn read_raw_registers<T: ModbusReading>(&mut self, config: &T) -> Result<Vec<u8>> {
        // Read raw register values
        let register_to_read = self.register_offset + config.register();
        let raw_registers = self
            .read_registers(register_to_read, config.words(), config.fncode())
            .await?;

        // Convert registers to bytes with proper order
        let bytes = registers_to_bytes(&raw_registers, config.order())?;

        Ok(bytes)
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
