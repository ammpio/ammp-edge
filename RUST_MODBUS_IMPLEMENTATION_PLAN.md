# Rust ModbusTCP Reading Implementation Plan

This document outlines a detailed plan for porting Python ModbusTCP reading functionality to Rust, implementing a `start-readings` command that replicates the Python reading cycle behavior using async Rust and tokio-modbus.

## Analysis Summary

Based on analysis of the Python codebase, the reading cycle involves:
1. **Scheduler-based reading cycles** using `read_interval` and `read_roundtime` config parameters
2. **Multi-threaded device reading** with connection pooling and locks
3. **ModbusTCP protocol implementation** via pyModbusTCP library
4. **Data processing pipeline** for register values (datatype conversion, scaling, offset)
5. **MQTT publishing** of aggregated readings with timestamps and metadata

The Rust implementation will leverage the existing config schema, data models, and MQTT publishing infrastructure while introducing async capabilities via tokio.

## Architecture Overview

### Current Rust Infrastructure (Available)
- ✅ **Config Management**: `rust/src/node_mgmt/config.rs` with JSON schema validation
- ✅ **Data Models**: `rust/src/data_mgmt/models.rs` with `DeviceReading` struct
- ✅ **MQTT Publishing**: `rust/src/data_mgmt/publish.rs` with `publish_readings()` function
- ✅ **KV Store Access**: `kvstore::KVDb` for configuration persistence

### New Components to Implement
- 🔄 **Async Reading Cycle**: tokio-based scheduler with interval timing
- 🔄 **ModbusTCP Reader**: tokio-modbus client with connection pooling
- 🔄 **Data Processing**: Value conversion and scaling (port from Python)
- 🔄 **Reading Orchestration**: Multi-device concurrent reading management

## Implementation Plan

### Phase 1: Dependencies and Project Setup

**Add to `Cargo.toml`:**
```toml
[dependencies]
# Existing dependencies...
tokio = { version = "1.0", features = ["full"] }
tokio-modbus = { version = "0.14", features = ["tcp", "rtu"] }
tokio-cron-scheduler = "0.10"
futures = "0.3"
async-trait = "0.1"

# For value processing
byteorder = "1.5"
```

**Update main.rs to async:**
```rust
// Add to main.rs
const CMD_START_READINGS: &str = "start-readings";

// Update main function
#[tokio::main]
async fn main() -> Result<()> {
    // ... existing setup ...
    match args.subcommand()?.as_deref() {
        // ... existing commands ...
        Some(CMD_START_READINGS) => command::start_readings().await,
        _ => Err(anyhow!(/* ... existing error */)),
    }
}
```

### Phase 2: Core Reading Cycle Implementation

**Create `rust/src/command/reading_cycle.rs`:**

```rust
use anyhow::Result;
use tokio::time::{interval, Duration, Instant};
use std::collections::HashMap;

use crate::{
    data_mgmt::{self, models::DeviceReading, payload::Metadata},
    node_mgmt,
    readers::modbus_tcp,
};

pub async fn start_readings() -> Result<()> {
    let kvs = kvstore::KVDb::new(crate::interfaces::kvpath::SQLITE_STORE.as_path())?;
    let config = node_mgmt::config::get(kvs)?;

    // Extract timing parameters
    let read_interval = config.read_interval.unwrap_or(60); // Default 60 seconds
    let read_roundtime = config.read_roundtime.unwrap_or(false);

    log::info!("Starting reading cycle with interval: {}s, roundtime: {}",
               read_interval, read_roundtime);

    // Create interval timer
    let mut interval_timer = if read_roundtime {
        create_aligned_interval(read_interval).await
    } else {
        interval(Duration::from_secs(read_interval as u64))
    };

    loop {
        interval_timer.tick().await;

        match execute_reading_cycle(&config).await {
            Ok(readings) => {
                if !readings.is_empty() {
                    log::info!("Completed reading cycle: {} readings", readings.len());
                    publish_readings(readings).await?;
                }
            }
            Err(e) => log::error!("Reading cycle error: {}", e),
        }
    }
}

async fn execute_reading_cycle(config: &Config) -> Result<Vec<DeviceReading>> {
    // 1. Filter ModbusTCP devices only
    let modbus_devices = filter_modbus_devices(config);

    // 2. Concurrent device reading
    let reading_tasks = modbus_devices.into_iter()
        .map(|(device_id, device_config, readings)| {
            tokio::spawn(read_modbus_device(device_id, device_config, readings))
        });

    // 3. Collect results with timeout
    let timeout_duration = Duration::from_secs(600); // 10 minutes max
    let results = tokio::time::timeout(
        timeout_duration,
        futures::future::join_all(reading_tasks)
    ).await?;

    // 4. Aggregate successful readings
    let mut all_readings = Vec::new();
    for result in results {
        match result? {
            Ok(device_readings) => all_readings.extend(device_readings),
            Err(e) => log::warn!("Device reading failed: {}", e),
        }
    }

    Ok(all_readings)
}
```

### Phase 3: ModbusTCP Reader Implementation

**Create `rust/src/readers/modbus_tcp/mod.rs`:**

```rust
use anyhow::Result;
use std::net::SocketAddr;
use tokio_modbus::prelude::*;
use std::collections::HashMap;

use crate::data_mgmt::models::{DeviceReading, Reading};

pub struct ModbusTcpReader {
    context: tokio_modbus::client::Context,
    device_id: String,
    unit_id: u8,
}

impl ModbusTcpReader {
    pub async fn connect(
        device_id: String,
        host: &str,
        port: u16,
        unit_id: u8,
        timeout: Option<Duration>,
    ) -> Result<Self> {
        let socket_addr: SocketAddr = format!("{}:{}", host, port).parse()?;

        let mut ctx = tcp::connect(socket_addr).await?;
        ctx.set_slave(Slave(unit_id));

        if let Some(timeout) = timeout {
            // Set timeout if supported by tokio-modbus
            // Note: Check tokio-modbus documentation for timeout support
        }

        log::debug!("Connected to Modbus device {}:{}/{}", host, port, unit_id);

        Ok(ModbusTcpReader {
            context: ctx,
            device_id,
            unit_id,
        })
    }

    pub async fn read_registers(
        &mut self,
        register: u16,
        count: u16,
        function_code: u8,
    ) -> Result<Vec<u16>> {
        let registers = match function_code {
            3 => self.context.read_holding_registers(register, count).await?,
            4 => self.context.read_input_registers(register, count).await?,
            _ => return Err(anyhow::anyhow!("Unsupported function code: {}", function_code)),
        };

        log::debug!("Read {} registers from {}: {:?}", count, register, registers);
        Ok(registers)
    }

    pub async fn execute_readings(
        &mut self,
        reading_configs: Vec<ReadingConfig>,
    ) -> Result<Vec<Reading>> {
        let mut readings = Vec::new();

        for config in reading_configs {
            match self.read_single_value(&config).await {
                Ok(value) => {
                    readings.push(Reading {
                        field: config.variable_name,
                        value: value.into(),
                        unit: config.unit,
                    });
                }
                Err(e) => {
                    log::warn!("Failed to read {}: {}", config.variable_name, e);
                }
            }
        }

        Ok(readings)
    }

    async fn read_single_value(&mut self, config: &ReadingConfig) -> Result<f64> {
        // Read raw register values
        let raw_registers = self.read_registers(
            config.register,
            config.word_count,
            config.function_code.unwrap_or(3),
        ).await?;

        // Convert to bytes
        let mut bytes = Vec::new();
        for register in &raw_registers {
            bytes.extend_from_slice(&register.to_be_bytes());
        }

        // Process value according to datatype
        let raw_value = parse_register_value(&bytes, &config.datatype)?;

        // Apply scaling
        let scaled_value = raw_value * config.multiplier.unwrap_or(1.0)
                         + config.offset.unwrap_or(0.0);

        Ok(scaled_value)
    }
}

#[derive(Clone, Debug)]
pub struct ReadingConfig {
    pub variable_name: String,
    pub register: u16,
    pub word_count: u16,
    pub datatype: String,
    pub function_code: Option<u8>,
    pub multiplier: Option<f64>,
    pub offset: Option<f64>,
    pub unit: Option<String>,
}

fn parse_register_value(bytes: &[u8], datatype: &str) -> Result<f64> {
    use byteorder::{BigEndian, ReadBytesExt};
    use std::io::Cursor;

    let mut cursor = Cursor::new(bytes);

    let value = match datatype {
        "uint16" => cursor.read_u16::<BigEndian>()? as f64,
        "int16" => cursor.read_i16::<BigEndian>()? as f64,
        "uint32" => cursor.read_u32::<BigEndian>()? as f64,
        "int32" => cursor.read_i32::<BigEndian>()? as f64,
        "uint64" => cursor.read_u64::<BigEndian>()? as f64,
        "float" | "single" => cursor.read_f32::<BigEndian>()? as f64,
        "double" => cursor.read_f64::<BigEndian>()?,
        _ => return Err(anyhow::anyhow!("Unsupported datatype: {}", datatype)),
    };

    Ok(value)
}
```

### Phase 4: Configuration Integration

**Create configuration mapping functions:**

```rust
// In rust/src/readers/modbus_tcp/config.rs
use crate::node_mgmt::Config;
use super::ReadingConfig;

pub fn extract_modbus_devices(config: &Config) -> Vec<(String, ModbusDeviceConfig, Vec<ReadingConfig>)> {
    let mut modbus_devices = Vec::new();

    // Filter devices with reading_type = "modbustcp"
    for (device_id, device) in &config.devices {
        if device.reading_type == Some("modbustcp".to_string()) {
            let device_config = ModbusDeviceConfig::from_config(device);
            let reading_configs = extract_device_readings(config, device_id);
            modbus_devices.push((device_id.clone(), device_config, reading_configs));
        }
    }

    modbus_devices
}

pub fn extract_device_readings(config: &Config, device_id: &str) -> Vec<ReadingConfig> {
    config.readings.iter()
        .filter_map(|(reading_name, reading)| {
            if reading.device == device_id {
                Some(ReadingConfig::from_config(reading_name, reading, config))
            } else {
                None
            }
        })
        .collect()
}

#[derive(Clone, Debug)]
pub struct ModbusDeviceConfig {
    pub host: String,
    pub port: u16,
    pub unit_id: u8,
    pub timeout: Option<Duration>,
}
```

### Phase 5: Integration with Existing Infrastructure

**Update module structure:**

```rust
// rust/src/readers/mod.rs
pub mod sma_hycon_csv;
pub mod modbus_tcp;

// rust/src/command/mod.rs
pub mod reading_cycle;
// ... existing modules ...

// rust/src/main.rs
mod readers;
use command::reading_cycle;
```

**Connect to existing MQTT publishing:**

```rust
// In reading_cycle.rs
async fn publish_readings(readings: Vec<DeviceReading>) -> Result<()> {
    let metadata = Some(Metadata {
        data_provider: Some("modbus-tcp-reader".into()),
        ..Default::default()
    });

    data_mgmt::publish::publish_readings(readings, metadata)?;
    Ok(())
}
```

## Implementation Phases Summary

### Phase 1: Project Setup (1-2 days)
- Add tokio and tokio-modbus dependencies
- Convert main.rs to async
- Add `start-readings` command placeholder

### Phase 2: Basic Reading Cycle (2-3 days)
- Implement reading cycle scheduler with interval support
- Add configuration loading and device filtering
- Create basic async task structure

### Phase 3: ModbusTCP Implementation (3-4 days)
- Implement tokio-modbus client wrapper
- Add register reading and value processing
- Implement data type conversion and scaling

### Phase 4: Configuration Integration (1-2 days)
- Map JSON schema config to Rust structs
- Implement device and reading extraction
- Add proper error handling

### Phase 5: Testing and Integration (2-3 days)
- Integration with existing MQTT publishing
- Add comprehensive logging
- Testing with real ModbusTCP devices
- Performance optimization

## Key Design Decisions

### 1. Async Architecture
- **tokio runtime** for async/await support
- **tokio-modbus** for ModbusTCP client implementation
- **Concurrent device reading** using tokio::spawn tasks
- **Connection pooling** per device with proper cleanup

### 2. Configuration Compatibility
- **Reuse existing JSON schema** from `config.schema.json`
- **Same config parameters**: `read_interval`, `read_roundtime`, device/reading definitions
- **Backward compatibility** with Python configuration format

### 3. Data Processing Pipeline
- **Equivalent processing** to Python `process_reading()` function
- **Same data types** supported: int16, uint16, int32, uint32, float, double
- **Same scaling logic**: `output = multiplier * reading + offset`
- **Error handling** with graceful degradation

### 4. Integration Points
- **Reuse existing**: Config management, MQTT publishing, KV store access
- **New async layer**: Reading cycle, ModbusTCP client, task coordination
- **Logging consistency** with existing Rust modules

## Risk Mitigation

### 1. Async Runtime Integration
- **Risk**: Mixing sync/async code in existing codebase
- **Mitigation**: Isolate async code to reading cycle command only
- **Fallback**: Keep existing sync commands working unchanged

### 2. tokio-modbus Compatibility
- **Risk**: Library compatibility issues with different ModbusTCP devices
- **Mitigation**: Extensive testing with various device types
- **Fallback**: Implement custom ModbusTCP client if needed

### 3. Performance Impact
- **Risk**: Async overhead affecting reading performance
- **Mitigation**: Benchmark against Python implementation
- **Optimization**: Connection pooling and concurrent reading

### 4. Configuration Complexity
- **Risk**: Config mapping errors between Python and Rust
- **Mitigation**: Unit tests for config parsing and device extraction
- **Validation**: JSON schema validation at runtime

## Success Criteria

1. ✅ **Functional Equivalence**: Same reading results as Python implementation
2. ✅ **Configuration Compatibility**: Works with existing config files
3. ✅ **Performance**: At least equivalent performance to Python version
4. ✅ **Reliability**: Proper error handling and graceful failure recovery
5. ✅ **Integration**: Seamless integration with existing Rust infrastructure
6. ✅ **Maintainability**: Clean, documented, testable code structure

## Next Steps

1. **Review and approve** this implementation plan
2. **Set up development environment** with tokio dependencies
3. **Create minimal prototype** of Phase 1 (project setup)
4. **Validate tokio-modbus integration** with test devices
5. **Begin iterative implementation** following the phased approach

This plan provides a comprehensive roadmap for successfully porting Python ModbusTCP functionality to Rust while leveraging the existing infrastructure and maintaining compatibility with current configurations.