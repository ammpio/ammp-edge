//! Output field processing using JSONata expressions
//!
//! This module provides functionality to derive additional fields from existing readings
//! using JSONata expressions, similar to the Python processor/get_output.py module.

use anyhow::{Result, anyhow};
use serde_json::{Value, json};
use std::collections::HashMap;

use crate::data_mgmt::models::{DeviceReading, Reading, RtValue};
use crate::data_mgmt::process::TypeCast;
use crate::node_mgmt::config::Config;
use derived_models::config::{Output, Typecast};

/// Process output fields from device readings using JSONata expressions
///
/// This function replicates the Python get_output functionality, taking device readings
/// and applying JSONata expressions to derive calculated fields.
pub fn process_outputs(device_readings: &[DeviceReading], config: &Config) -> Result<Vec<Reading>> {
    let mut output_readings = Vec::new();

    // Convert device readings to the expected JSON format for JSONata processing
    let readings_json = convert_device_readings_to_json(device_readings);

    // Process each output configuration
    for output_config in &config.output {
        match evaluate_output(output_config, &readings_json) {
            Ok(Some(reading)) => output_readings.push(reading),
            Ok(None) => {
                log::debug!(
                    "Output expression '{}' returned no value",
                    output_config.source
                );
            }
            Err(e) => {
                log::warn!(
                    "Failed to evaluate output expression '{}': {}",
                    output_config.source,
                    e
                );
            }
        }
    }

    Ok(output_readings)
}

/// Evaluate a single output configuration against the readings JSON
fn evaluate_output(output_config: &Output, readings_json: &Value) -> Result<Option<Reading>> {
    // Evaluate the JSONata expression
    let result = evaluate_jsonata(readings_json, &output_config.source)?;

    if result.is_null() {
        return Ok(None);
    }

    // Apply typecast to the result
    let value = apply_typecast(result, output_config.typecast)?;

    Ok(Some(Reading {
        field: output_config.field.clone(),
        value,
    }))
}

/// Evaluate a JSONata expression against JSON data
fn evaluate_jsonata(data: &Value, expression: &str) -> Result<Value> {
    use bumpalo::Bump;
    use jsonata_rs::JsonAta;

    let arena = Bump::new();
    let jsonata = JsonAta::new(expression, &arena)
        .map_err(|e| anyhow!("Failed to parse JSONata expression '{}': {}", expression, e))?;

    // Convert serde_json::Value to string for input to JSONata
    let input_str = serde_json::to_string(data)
        .map_err(|e| anyhow!("Failed to serialize input data: {}", e))?;

    let result = jsonata
        .evaluate(Some(&input_str), None)
        .map_err(|e| anyhow!("JSONata evaluation failed: {}", e))?;

    // Convert JSONata result back to serde_json::Value
    convert_jsonata_value_to_json(result)
}

/// Convert a JSONata Value to serde_json::Value
fn convert_jsonata_value_to_json<'a>(value: &'a jsonata_rs::Value<'a>) -> Result<Value> {
    match value {
        value if value.is_null() => Ok(Value::Null),
        value if value.is_bool() => Ok(json!(value.as_bool())),
        value if value.is_number() => Ok(json!(value.as_f64())),
        value if value.is_string() => Ok(json!(value.as_str())),
        value if value.is_array() => {
            let mut array = Vec::new();
            for item in value.members() {
                array.push(convert_jsonata_value_to_json(item)?);
            }
            Ok(json!(array))
        }
        value if value.is_object() => {
            let mut object = serde_json::Map::new();
            for (key, val) in value.entries() {
                object.insert(key.to_string(), convert_jsonata_value_to_json(val)?);
            }
            Ok(json!(object))
        }
        _ => Ok(Value::Null), // Handle other cases as null
    }
}

/// Apply typecast to a JSON value, converting it to the appropriate RtValue
///
/// This function bridges between the schema Typecast enum and the existing
/// process module functionality to avoid duplication.
fn apply_typecast(value: Value, typecast: Typecast) -> Result<RtValue> {
    // Handle string typecast separately since process module doesn't handle JSON strings directly
    if matches!(typecast, Typecast::Str) {
        let string_value = match value {
            Value::String(s) => s,
            _ => value.to_string(),
        };
        return Ok(RtValue::String(string_value));
    }

    // Convert schema Typecast to process TypeCast
    let process_typecast = match typecast {
        Typecast::Int => TypeCast::Int,
        Typecast::Float => TypeCast::Float,
        Typecast::Bool => TypeCast::Bool,
        Typecast::Str => unreachable!(), // Already handled above
    };

    // Convert JSON value to numeric for process module (it expects f64)
    let numeric_value = match value {
        Value::Number(n) => n.as_f64().unwrap_or(0.0),
        Value::Bool(b) => {
            if b {
                1.0
            } else {
                0.0
            }
        }
        Value::String(s) => s.parse::<f64>().unwrap_or(0.0),
        _ => 0.0,
    };

    // Use existing typecast functionality from process module
    let processed =
        crate::data_mgmt::process::apply_typecast(numeric_value, Some(process_typecast))?;

    // The process module now returns RtValue directly
    Ok(processed)
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
    use crate::node_mgmt::config::Device;
    use derived_models::config::ReadingType;

    fn create_test_device(key: &str) -> Device {
        Device {
            key: key.to_string(),
            device_model: "test_model".to_string(),
            driver: "test_driver".to_string(),
            reading_type: ReadingType::Modbustcp,
            vendor_id: "test_vendor".to_string(),
            enabled: true,
            address: None,
            name: Some("Test Device".to_string()),
            min_read_interval: None,
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
    fn test_apply_typecast() {
        // Test int typecast
        let result = apply_typecast(json!(42.7), Typecast::Int).unwrap();
        assert_eq!(result, RtValue::Int(42));

        // Test float typecast
        let result = apply_typecast(json!(42), Typecast::Float).unwrap();
        assert_eq!(result, RtValue::Float(42.0));

        // Test string typecast
        let result = apply_typecast(json!("hello"), Typecast::Str).unwrap();
        assert_eq!(result, RtValue::String("hello".to_string()));

        // Test bool typecast
        let result = apply_typecast(json!(true), Typecast::Bool).unwrap();
        assert_eq!(result, RtValue::Bool(true));

        let result = apply_typecast(json!(0), Typecast::Bool).unwrap();
        assert_eq!(result, RtValue::Bool(false));
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

        let result = evaluate_jsonata(
            &data,
            "device1[var = \"P_L1\"].value + device1[var = \"P_L2\"].value",
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), json!(300.0));
    }

    #[test]
    fn test_process_outputs_integration() {
        use derived_models::config::{AmmpEdgeConfiguration, Output};
        use std::collections::HashMap;

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

        let config = AmmpEdgeConfiguration {
            devices: HashMap::new(),
            readings: HashMap::new(),
            output: vec![output],
            calc_vendor_id: None,
            drivers: HashMap::new(),
            name: None,
            push_throttle_delay: None,
            push_timeout: None,
            read_interval: 60,
            read_roundtime: false,
            status_readings: vec![],
            timestamp: None,
            volatile_q_size: None,
        };

        let result = process_outputs(&[device_reading], &config);
        assert!(result.is_ok());

        let outputs = result.unwrap();
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].field, "P_total");
        assert_eq!(outputs[0].value, RtValue::Float(450.0));
    }
}
