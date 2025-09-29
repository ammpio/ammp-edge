//! Data processing module
//!
//! This module provides data processing functionality equivalent to the Python
//! processor/process_reading.py module. It handles data type conversion, scaling,
//! offset application, and type casting for readings from various device types.

use crate::node_mgmt::drivers::FieldOpts;
use crate::data_mgmt::models::RtValue;
use anyhow::{Result, anyhow};
use std::collections::HashMap;

/// Process raw bytes using field configuration from driver system
///
/// This is a convenience function that converts FieldOpts to ProcessingParams
/// and processes the raw bytes. This provides the clean separation between
/// reading (getting raw bytes) and processing (converting to final values).
pub fn process_field_reading(val_bytes: &[u8], field_config: &FieldOpts) -> Result<RtValue> {
    let params = ProcessingParams::from_field_opts(field_config)?;
    process_reading(val_bytes, &params)
}

/// Process a raw reading value according to the provided parameters
///
/// This function replicates the Python `process_reading()` functionality,
/// supporting the same parameter set and processing pipeline.
pub fn process_reading(val_bytes: &[u8], params: &ProcessingParams) -> Result<RtValue> {
    // Parse the raw bytes according to parse_as parameter
    let value = if val_bytes.is_empty() {
        return Ok(RtValue::None);
    } else {
        parse_value_bytes(val_bytes, params)?
    };

    let value = match value {
        Some(v) => v,
        None => return Ok(RtValue::None),
    };

    // Apply multiplier and offset (unless dealing with string/boolean)
    let scaled_value = if matches!(params.typecast, Some(TypeCast::Str) | Some(TypeCast::Bool)) {
        value
    } else {
        apply_multiplier_offset(value, params.multiplier, params.offset)?
    };

    // Apply final type casting
    let final_value = apply_typecast(scaled_value, params.typecast)?;

    Ok(final_value)
}

/// Parse raw bytes according to the parse_as parameter
fn parse_value_bytes(val_bytes: &[u8], params: &ProcessingParams) -> Result<Option<f64>> {
    match params.parse_as {
        ParseAs::Str => {
            let val_str = std::str::from_utf8(val_bytes)
                .map_err(|_| anyhow!("Could not decode bytes into UTF-8 string"))?;
            value_from_string(val_str, params)
        }
        ParseAs::Hex => {
            let val_hex_str = std::str::from_utf8(val_bytes)
                .map_err(|_| anyhow!("Could not decode bytes into UTF-8 string"))?;
            let hex_bytes = hex::decode(val_hex_str)
                .map_err(|_| anyhow!("Could not parse {} as hex value", val_hex_str))?;
            value_from_bytes(&hex_bytes, params)
        }
        ParseAs::Bytes => value_from_bytes(val_bytes, params),
    }
}

/// Extract numeric value from string representation
fn value_from_string(val_str: &str, params: &ProcessingParams) -> Result<Option<f64>> {
    // Check value mapping first
    if let Some(ref valuemap) = params.valuemap
        && let Some(mapped_value) = valuemap.get(val_str)
    {
        return Ok(Some(*mapped_value));
    }

    // Parse as number based on typecast
    let value = match params.typecast {
        Some(TypeCast::Int) => val_str.parse::<i64>()? as f64,
        Some(TypeCast::Float) => val_str.parse::<f64>()?,
        Some(TypeCast::Bool) => {
            let bool_val = val_str.parse::<bool>()?;
            if bool_val { 1.0 } else { 0.0 }
        }
        Some(TypeCast::Str) => {
            // For strings, we don't convert to f64, handle separately
            return Ok(Some(0.0)); // Placeholder, will be handled in typecast
        }
        None => val_str.parse::<f64>()?, // Default to float parsing
    };

    Ok(Some(value))
}

/// Extract numeric value from bytes using the specified datatype
fn value_from_bytes(val_bytes: &[u8], params: &ProcessingParams) -> Result<Option<f64>> {
    // Check value mapping first (hex format)
    if let Some(ref valuemap) = params.valuemap {
        let hex_key = format!("0x{}", hex::encode(val_bytes));
        if let Some(mapped_value) = valuemap.get(&hex_key) {
            return Ok(Some(*mapped_value));
        }
    }

    let datatype = params
        .datatype
        .as_ref()
        .ok_or_else(|| anyhow!("datatype parameter required for bytes parsing"))?;

    let value = match datatype {
        DataType::Int16 => {
            if val_bytes.len() < 2 {
                return Err(anyhow!("Insufficient bytes for int16"));
            }
            i16::from_be_bytes([val_bytes[0], val_bytes[1]]) as f64
        }
        DataType::UInt16 => {
            if val_bytes.len() < 2 {
                return Err(anyhow!("Insufficient bytes for uint16"));
            }
            u16::from_be_bytes([val_bytes[0], val_bytes[1]]) as f64
        }
        DataType::Int32 => {
            if val_bytes.len() < 4 {
                return Err(anyhow!("Insufficient bytes for int32"));
            }
            i32::from_be_bytes([val_bytes[0], val_bytes[1], val_bytes[2], val_bytes[3]]) as f64
        }
        DataType::UInt32 => {
            if val_bytes.len() < 4 {
                return Err(anyhow!("Insufficient bytes for uint32"));
            }
            u32::from_be_bytes([val_bytes[0], val_bytes[1], val_bytes[2], val_bytes[3]]) as f64
        }
        DataType::Int64 => {
            if val_bytes.len() < 8 {
                return Err(anyhow!("Insufficient bytes for int64"));
            }
            let mut bytes = [0u8; 8];
            bytes.copy_from_slice(&val_bytes[0..8]);
            i64::from_be_bytes(bytes) as f64
        }
        DataType::UInt64 => {
            if val_bytes.len() < 8 {
                return Err(anyhow!("Insufficient bytes for uint64"));
            }
            let mut bytes = [0u8; 8];
            bytes.copy_from_slice(&val_bytes[0..8]);
            u64::from_be_bytes(bytes) as f64
        }
        DataType::Float | DataType::Single => {
            if val_bytes.len() < 4 {
                return Err(anyhow!("Insufficient bytes for float"));
            }
            let mut bytes = [0u8; 4];
            bytes.copy_from_slice(&val_bytes[0..4]);
            f32::from_be_bytes(bytes) as f64
        }
        DataType::Double => {
            if val_bytes.len() < 8 {
                return Err(anyhow!("Insufficient bytes for double"));
            }
            let mut bytes = [0u8; 8];
            bytes.copy_from_slice(&val_bytes[0..8]);
            f64::from_be_bytes(bytes)
        }
    };

    Ok(Some(value))
}

/// Apply multiplier and offset: output = multiplier * reading + offset
fn apply_multiplier_offset(
    value: f64,
    multiplier: Option<f64>,
    offset: Option<f64>,
) -> Result<f64> {
    let multiplied = value * multiplier.unwrap_or(1.0);
    let result = multiplied + offset.unwrap_or(0.0);
    Ok(result)
}

/// Apply final type casting to get the desired output type
pub fn apply_typecast(value: f64, typecast: Option<TypeCast>) -> Result<RtValue> {
    match typecast {
        Some(TypeCast::Int) => Ok(RtValue::Int(value as i64)),
        Some(TypeCast::Float) => Ok(RtValue::Float(value)),
        Some(TypeCast::Bool) => Ok(RtValue::Bool(value != 0.0)),
        Some(TypeCast::Str) => Ok(RtValue::String(value.to_string())),
        None => Ok(RtValue::Float(value)), // Default to float
    }
}

/// Parameters for processing readings
#[derive(Debug, Clone)]
pub struct ProcessingParams {
    /// How to parse the raw bytes
    pub parse_as: ParseAs,
    /// Data type for bytes/hex parsing
    pub datatype: Option<DataType>,
    /// Type casting for final output
    pub typecast: Option<TypeCast>,
    /// Value mapping (hex keys for bytes, string keys for strings)
    pub valuemap: Option<HashMap<String, f64>>,
    /// Multiplier to apply
    pub multiplier: Option<f64>,
    /// Offset to apply (after multiplier)
    pub offset: Option<f64>,
}

impl Default for ProcessingParams {
    fn default() -> Self {
        Self {
            parse_as: ParseAs::Bytes,
            datatype: None,
            typecast: None,
            valuemap: None,
            multiplier: None,
            offset: None,
        }
    }
}

impl ProcessingParams {
    /// Create ProcessingParams from FieldOpts (driver field configuration)
    ///
    /// This provides the bridge between the driver configuration system
    /// and the data processing system.
    pub fn from_field_opts(field_config: &FieldOpts) -> Result<Self> {
        // Convert datatype from driver schema to processing enum
        let datatype = if let Some(dt) = &field_config.datatype {
            Some(dt.to_string().parse::<DataType>()?)
        } else {
            None
        };

        // Convert typecast from driver schema to processing enum
        let typecast = if let Some(tc) = &field_config.typecast {
            Some(match tc.to_string().as_str() {
                "int" => TypeCast::Int,
                "float" => TypeCast::Float,
                "str" => TypeCast::Str,
                "bool" => TypeCast::Bool,
                other => return Err(anyhow!("Unsupported typecast: {}", other)),
            })
        } else {
            None
        };

        // Convert datamap to valuemap
        let valuemap = if !field_config.datamap.is_empty() {
            let mut vm = HashMap::new();
            for (key, value) in &field_config.datamap {
                if let Some(num_val) = value.as_f64() {
                    vm.insert(key.clone(), num_val);
                }
                // Note: null values in datamap are ignored as they typically represent invalid readings
            }
            if !vm.is_empty() { Some(vm) } else { None }
        } else {
            None
        };

        Ok(ProcessingParams {
            parse_as: ParseAs::Bytes, // Default for Modbus and most binary protocols
            datatype,
            typecast,
            valuemap,
            multiplier: field_config.multiplier,
            offset: field_config.offset,
        })
    }
}

/// How to parse raw input data
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ParseAs {
    /// Parse as raw bytes (default)
    Bytes,
    /// Parse as UTF-8 string containing numeric value
    Str,
    /// Parse as UTF-8 string containing hex representation
    Hex,
}

/// Data types supported for bytes parsing
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DataType {
    Int16,
    UInt16,
    Int32,
    UInt32,
    Int64,
    UInt64,
    Float,
    Single, // Alias for Float
    Double,
}

impl std::str::FromStr for DataType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "int16" => Ok(DataType::Int16),
            "uint16" => Ok(DataType::UInt16),
            "int32" => Ok(DataType::Int32),
            "uint32" => Ok(DataType::UInt32),
            "int64" => Ok(DataType::Int64),
            "uint64" => Ok(DataType::UInt64),
            "float" => Ok(DataType::Float),
            "single" => Ok(DataType::Single),
            "double" => Ok(DataType::Double),
            _ => Err(anyhow!("Unsupported datatype: {}", s)),
        }
    }
}

/// Type casting options for final output
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TypeCast {
    Int,
    Float,
    Str,
    Bool,
}

impl std::str::FromStr for TypeCast {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "int" => Ok(TypeCast::Int),
            "float" => Ok(TypeCast::Float),
            "str" => Ok(TypeCast::Str),
            "bool" => Ok(TypeCast::Bool),
            _ => Err(anyhow!("Unsupported typecast: {}", s)),
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_uint16() {
        let bytes = [0x12, 0x34]; // 0x1234 = 4660
        let params = ProcessingParams {
            datatype: Some(DataType::UInt16),
            ..Default::default()
        };

        let result = process_reading(&bytes, &params).unwrap();
        assert_eq!(result, RtValue::Float(4660.0));
    }

    #[test]
    fn test_process_with_multiplier_offset() {
        let bytes = [0x00, 0x64]; // 100
        let params = ProcessingParams {
            datatype: Some(DataType::UInt16),
            multiplier: Some(0.1),
            offset: Some(5.0),
            typecast: Some(TypeCast::Float),
            ..Default::default()
        };

        let result = process_reading(&bytes, &params).unwrap();
        assert_eq!(result, RtValue::Float(15.0)); // 100 * 0.1 + 5.0 = 15.0
    }

    #[test]
    fn test_process_string_value() {
        let bytes = b"123.45";
        let params = ProcessingParams {
            parse_as: ParseAs::Str,
            typecast: Some(TypeCast::Float),
            ..Default::default()
        };

        let result = process_reading(bytes, &params).unwrap();
        assert_eq!(result, RtValue::Float(123.45));
    }

    #[test]
    fn test_value_mapping() {
        let bytes = [0x12, 0x34];
        let mut valuemap = HashMap::new();
        valuemap.insert("0x1234".to_string(), 999.0);

        let params = ProcessingParams {
            datatype: Some(DataType::UInt16),
            valuemap: Some(valuemap),
            ..Default::default()
        };

        let result = process_reading(&bytes, &params).unwrap();
        assert_eq!(result, RtValue::Float(999.0));
    }

    #[test]
    fn test_typecast_to_int() {
        let bytes = [0x00, 0x64]; // 100
        let params = ProcessingParams {
            datatype: Some(DataType::UInt16),
            typecast: Some(TypeCast::Int),
            ..Default::default()
        };

        let result = process_reading(&bytes, &params).unwrap();
        assert_eq!(result, RtValue::Int(100));
    }
}
