//! Example demonstrating JSONata output processing
//!
//! This example shows how to use the output processing functionality
//! to derive calculated fields from device readings using JSONata expressions.

use ae::data_mgmt::{
    models::{DeviceReading, Record, RtValue},
    output::process_outputs,
};
use ae::node_mgmt::config::Device;
use derived_models::config::{AmmpEdgeConfiguration, Output, ReadingType};
use derived_models::driver::Typecast;
use std::collections::HashMap;

fn main() -> anyhow::Result<()> {
    // Create some sample device readings
    let device_readings = create_sample_readings();

    // Create configuration with output expressions
    let config = create_sample_config();

    // Process outputs using JSONata expressions
    let calculated_fields = process_outputs(&device_readings, &config)?;

    println!("Calculated output fields:");
    for field in calculated_fields {
        println!("  {}: {:?}", field.field, field.value);
    }

    Ok(())
}

fn create_sample_readings() -> Vec<DeviceReading> {
    // Create first device (power meter) readings
    let mut power_meter_record = Record::new();
    power_meter_record.set_field("P_L1".to_string(), RtValue::Float(1000.0)); // Phase 1 power
    power_meter_record.set_field("P_L2".to_string(), RtValue::Float(1200.0)); // Phase 2 power
    power_meter_record.set_field("P_L3".to_string(), RtValue::Float(800.0)); // Phase 3 power

    let power_meter = DeviceReading {
        device: Device {
            key: "power_meter_1".to_string(),
            device_model: "em210".to_string(),
            driver: "em210_driver".to_string(),
            reading_type: ReadingType::Modbustcp,
            vendor_id: "pm-001".to_string(),
            enabled: true,
            address: None,
            name: Some("Main Power Meter".to_string()),
            min_read_interval: None,
        },
        record: power_meter_record,
    };

    // Create second device (inverter) readings
    let mut inverter_record = Record::new();
    inverter_record.set_field("P_ac".to_string(), RtValue::Float(2500.0)); // AC power output
    inverter_record.set_field("efficiency".to_string(), RtValue::Float(0.95)); // Efficiency ratio

    let inverter = DeviceReading {
        device: Device {
            key: "inverter_1".to_string(),
            device_model: "sma_stp".to_string(),
            driver: "sma_stp_driver".to_string(),
            reading_type: ReadingType::Modbustcp,
            vendor_id: "inv-001".to_string(),
            enabled: true,
            address: None,
            name: Some("Solar Inverter".to_string()),
            min_read_interval: None,
        },
        record: inverter_record,
    };

    vec![power_meter, inverter]
}

fn create_sample_config() -> AmmpEdgeConfiguration {
    let outputs = vec![
        // Calculate total power from all three phases
        Output {
            device: Some("power_meter_1".to_string()),
            field: "P_total".to_string(),
            source: "power_meter_1[var = \"P_L1\"].value + power_meter_1[var = \"P_L2\"].value + power_meter_1[var = \"P_L3\"].value".to_string(),
            typecast: Typecast::Float,
        },
        // Calculate DC power from AC power and efficiency
        Output {
            device: Some("inverter_1".to_string()),
            field: "P_dc_calculated".to_string(),
            source: "inverter_1[var = \"P_ac\"].value / inverter_1[var = \"efficiency\"].value".to_string(),
            typecast: Typecast::Float,
        },
        // Calculate net power (consumption - generation)
        Output {
            device: None, // This is a global calculation
            field: "P_net".to_string(),
            source: "(power_meter_1[var = \"P_L1\"].value + power_meter_1[var = \"P_L2\"].value + power_meter_1[var = \"P_L3\"].value) - inverter_1[var = \"P_ac\"].value".to_string(),
            typecast: Typecast::Float,
        },
    ];

    AmmpEdgeConfiguration {
        devices: HashMap::new(),
        readings: HashMap::new(),
        output: outputs,
        calc_vendor_id: Some("calculated".to_string()),
        drivers: HashMap::new(),
        name: Some("JSONata Output Example".to_string()),
        push_throttle_delay: None,
        push_timeout: None,
        read_interval: 60,
        read_roundtime: false,
        status_readings: vec![],
        timestamp: None,
        volatile_q_size: None,
    }
}
