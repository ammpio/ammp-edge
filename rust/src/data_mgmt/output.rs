//! Output field processing using JSONata expressions
//!
//! This module provides functionality to derive additional fields from existing readings
//! using JSONata expressions, similar to the Python processor/get_output.py module.

use std::collections::HashMap;

use anyhow::{Result, anyhow};
use bumpalo::Bump;
use derived_models::config::Output;
use jsonata_rs::JsonAta;
use serde_json::{Value, json};

use crate::data_mgmt::models::{DeviceReading, DeviceRef, Reading, Record, RtValue};
use crate::node_mgmt::config::Config;
use crate::node_mgmt::drivers::Typecast;

/// Calculate outputs from device readings and structure as DeviceReading
/// that can be included in MQTT payload
pub fn get_outputs_from_device_readings(
    device_readings: &[DeviceReading],
    config: &Config,
) -> Option<DeviceReading> {
    let mut record = Record::new();
    let output_readings = process_outputs(device_readings, &config.output).ok()?;
    if output_readings.is_empty() {
        return None;
    }
    for reading in output_readings {
        record.set_field(reading.field, reading.value);
    }

    if let Some(timestamp) = device_readings[0].record.get_timestamp() {
        record.set_timestamp(timestamp);
    }

    Some(DeviceReading {
        device: DeviceRef::new(
            "_calc".to_string(),
            config.calc_vendor_id.clone().unwrap_or_default(),
        ),
        record,
    })
}

/// Process output fields from device readings using JSONata expressions
///
/// This function takes device readings and applies JSONata expressions to derive calculated fields.
pub fn process_outputs(
    device_readings: &[DeviceReading],
    output_configs: &[Output],
) -> Result<Vec<Reading>> {
    let mut output_readings = Vec::new();

    // Convert device readings to the expected JSON format for JSONata processing
    let readings_json = convert_device_readings_to_json(device_readings);

    // Process each output field configuration
    for output in output_configs {
        match evaluate_output(output, &readings_json) {
            Ok(Some(reading)) => output_readings.push(reading),
            Ok(None) => {
                log::info!("Output expression '{}' returned no value", output.source);
            }
            Err(e) => {
                log::warn!(
                    "Failed to evaluate output expression '{}': {}",
                    output.source,
                    e
                );
            }
        }
    }

    Ok(output_readings)
}

/// Evaluate a single output configuration against the readings JSON
fn evaluate_output(output: &Output, readings_json: &Value) -> Result<Option<Reading>> {
    let value =
        evaluate_jsonata_and_typecast_result(readings_json, &output.source, output.typecast)?;

    Ok(Some(Reading {
        field: output.field.clone(),
        value,
    }))
}

/// Evaluate a JSONata expression against JSON data
fn evaluate_jsonata_and_typecast_result(
    data: &Value,
    expression: &str,
    typecast: Typecast,
) -> Result<RtValue> {
    let arena = Bump::new();
    let jsonata = JsonAta::new(expression, &arena)
        .map_err(|e| anyhow!("Failed to parse JSONata expression '{}': {}", expression, e))?;

    // Convert serde_json::Value to string for input to JSONata
    let input_str = serde_json::to_string(data)
        .map_err(|e| anyhow!("Failed to serialize input data: {}", e))?;

    let result = jsonata
        .evaluate(Some(&input_str), None)
        .map_err(|e| anyhow!("JSONata evaluation failed: {}", e))?;

    if result.is_null() || result.is_undefined() {
        return Ok(RtValue::None);
    }

    match typecast {
        Typecast::Int => Ok(RtValue::Int(result.as_isize() as i64)),
        Typecast::Float => Ok(RtValue::Float(result.as_f64())),
        Typecast::Bool => Ok(RtValue::Bool(result.as_bool())),
        Typecast::Str => Ok(RtValue::String(result.as_str().to_string())),
    }
}

/// Convert device readings to JSON format expected by JSONata expressions
///
/// The format matches the Python implementation structure:
/// ```json
/// {
///   "device_1": [{"var": "field1", "value": 10}, {"var": "field2", "value": 20}],
///   "device_2": [{"var": "field3", "value": 30}]
/// }
/// ```
fn convert_device_readings_to_json(device_readings: &[DeviceReading]) -> Value {
    let mut result = HashMap::new();

    for device_reading in device_readings {
        let device_key = &device_reading.device.key;
        let mut readings = Vec::new();

        for (field_name, field_value) in device_reading.record.all_fields() {
            let reading_json = json!({
                "var": field_name,
                "value": rt_value_to_json(field_value)
            });
            readings.push(reading_json);
        }

        result.insert(device_key.clone(), readings);
    }

    json!(result)
}

/// Convert RtValue to JSON Value
///
/// This function extracts the inner value from RtValue for JSON compatibility.
/// Note: We can't use RtValue's Serialize directly because it serializes as a tagged enum
/// (e.g., {"Float": 100.0}) but we need the raw values (e.g., 100.0) for JSONata compatibility.
fn rt_value_to_json(rt_value: &RtValue) -> Value {
    match rt_value {
        RtValue::None => json!(null),
        RtValue::Bool(b) => json!(b),
        RtValue::Float(f) => json!(f),
        RtValue::Int(i) => json!(i),
        RtValue::String(s) => json!(s),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_mgmt::models::Record;

    fn create_test_device(key: &str) -> DeviceRef {
        DeviceRef {
            key: key.to_string(),
            vendor_id: "test_vendor".to_string(),
        }
    }

    #[test]
    fn test_convert_device_readings_to_json() {
        let mut record = Record::new();
        record.set_field("P_L1".to_string(), RtValue::Float(100.0));
        record.set_field("P_L2".to_string(), RtValue::Float(200.0));

        let device_reading = DeviceReading {
            device: create_test_device("em210_grid"),
            record,
        };

        let json = convert_device_readings_to_json(&[device_reading]);

        assert!(json["em210_grid"].is_array());
        assert_eq!(json["em210_grid"].as_array().unwrap().len(), 2);

        // Check that the structure matches expected format
        let first_reading = &json["em210_grid"][0];
        assert!(first_reading["var"].is_string());
        assert!(first_reading["value"].is_number());
    }

    #[test]
    fn test_rt_value_to_json() {
        assert_eq!(rt_value_to_json(&RtValue::None), json!(null));
        assert_eq!(rt_value_to_json(&RtValue::Int(42)), json!(42));
        assert_eq!(rt_value_to_json(&RtValue::Float(3.23)), json!(3.23));
        assert_eq!(rt_value_to_json(&RtValue::Bool(true)), json!(true));
        assert_eq!(
            rt_value_to_json(&RtValue::String("test".to_string())),
            json!("test")
        );
    }

    #[test]
    fn test_evaluate_jsonata_simple() {
        // Test simple arithmetic
        let data = json!({
            "device1": [
                {"var": "P_L1", "value": 100},
                {"var": "P_L2", "value": 200}
            ]
        });

        let result = evaluate_jsonata_and_typecast_result(
            &data,
            "device1[var = \"P_L1\"].value + device1[var = \"P_L2\"].value",
            Typecast::Float,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), RtValue::Float(300.0));
    }

    #[test]
    fn test_process_outputs_integration() {
        use derived_models::config::Output;

        // Create test device readings
        let mut record1 = Record::new();
        record1.set_field("P_L1".to_string(), RtValue::Float(100.0));
        record1.set_field("P_L2".to_string(), RtValue::Float(200.0));
        record1.set_field("P_L3".to_string(), RtValue::Float(150.0));

        let device_reading = DeviceReading {
            device: create_test_device("em210_grid"),
            record: record1,
        };

        // Create test config with output
        let output = Output {
            device: Some("em210_grid".to_string()),
            field: "P_total".to_string(),
            source: "em210_grid[var = \"P_L1\"].value + em210_grid[var = \"P_L2\"].value + em210_grid[var = \"P_L3\"].value".to_string(),
            typecast: Typecast::Float,
        };

        let result = process_outputs(&[device_reading], &[output]);
        assert!(result.is_ok());

        let outputs = result.unwrap();
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].field, "P_total");
        assert_eq!(outputs[0].value, RtValue::Float(450.0));
    }

    #[test]
    fn test_output_with_undefined_result() {
        use derived_models::config::Output;

        let device_reading = DeviceReading {
            device: create_test_device("some_device"),
            record: Record::new(),
        };

        // Create test config with output
        let output = Output {
            device: Some("another_device".to_string()),
            field: "fuel_level_percent".to_string(),
            source: "(another_device[var = \"level\"].value)/2.45 * 100".to_string(),
            typecast: Typecast::Float,
        };

        let result = process_outputs(&[device_reading], &[output]);
        assert!(result.is_ok());

        let outputs = result.unwrap();
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].field, "fuel_level_percent");
        assert_eq!(outputs[0].value, RtValue::None);
    }
}
