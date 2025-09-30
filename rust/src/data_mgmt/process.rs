//! Data processing module
//!
//! This module provides data processing functionality equivalent to the Python
//! processor/process_reading.py module. It handles data type conversion, scaling,
//! offset application, and type casting for readings from various device types.

use crate::data_mgmt::models::RtValue;
use crate::node_mgmt::drivers::{DataType, FieldOpts, ParseAs, Typecast};
use anyhow::{Result, anyhow};

/// Process a raw reading value according to the field configuration
pub fn process_reading(val_bytes: &[u8], field_config: &FieldOpts) -> Result<RtValue> {
    // Parse the raw bytes according to parse_as parameter
    let value = if val_bytes.is_empty() {
        return Ok(RtValue::None);
    } else {
        parse_value_bytes(val_bytes, field_config)?
    };

    let value = match value {
        Some(v) => v,
        None => return Ok(RtValue::None),
    };

    let value = value.apply_multiplier_offset(field_config.multiplier, field_config.offset);

    let final_value = apply_typecast(value, field_config.typecast)?;

    Ok(final_value)
}

/// Parse raw bytes according to the parse_as parameter
fn parse_value_bytes(val_bytes: &[u8], field_config: &FieldOpts) -> Result<Option<NumericValue>> {
    let parse_as = field_config.parse_as.unwrap_or(ParseAs::Bytes);
    match parse_as {
        ParseAs::Str => {
            let val_str = std::str::from_utf8(val_bytes)
                .map_err(|_| anyhow!("Could not decode bytes into UTF-8 string"))?;
            value_from_string(val_str, field_config)
        }
        ParseAs::Hex => {
            let val_hex_str = std::str::from_utf8(val_bytes)
                .map_err(|_| anyhow!("Could not decode bytes into UTF-8 string"))?;
            let hex_bytes = hex::decode(val_hex_str)
                .map_err(|_| anyhow!("Could not parse {} as hex value", val_hex_str))?;
            value_from_bytes(&hex_bytes, field_config)
        }
        ParseAs::Bytes => value_from_bytes(val_bytes, field_config),
    }
}

/// Extract numeric value from string representation
fn value_from_string(val_str: &str, field_config: &FieldOpts) -> Result<Option<NumericValue>> {
    // Check value mapping first
    if let Some(mapped_value) = field_config.valuemap.get(val_str) {
        if let Some(mapped_value) = mapped_value {
            return Ok(Some(NumericValue::Float(*mapped_value)));
        } else {
            return Ok(None);
        }
    }

    // Parse as number based on typecast
    let value = match field_config.typecast {
        Some(Typecast::Int) => {
            let int_val = val_str.parse::<i64>()?;
            NumericValue::Int(int_val)
        }
        Some(Typecast::Float) => {
            let float_val = val_str.parse::<f64>()?;
            NumericValue::Float(float_val)
        }
        Some(Typecast::Bool) => {
            let bool_val = val_str.parse::<bool>()?;
            NumericValue::Int(if bool_val { 1 } else { 0 })
        }
        Some(Typecast::Str) => {
            // For strings, we don't convert to numeric, handle separately
            return Ok(Some(NumericValue::Float(0.0))); // TODO: Placeholder, will be handled in typecast
        }
        None => {
            // Try to parse as integer first, then fallback to float
            if let Ok(int_val) = val_str.parse::<i64>() {
                NumericValue::Int(int_val)
            } else {
                let float_val = val_str.parse::<f64>()?;
                NumericValue::Float(float_val)
            }
        }
    };

    Ok(Some(value))
}

/// Extract numeric value from bytes using the specified datatype
fn value_from_bytes(val_bytes: &[u8], field_config: &FieldOpts) -> Result<Option<NumericValue>> {
    // Check value mapping first (hex format)
    if let Some(mapped_value) = field_config
        .valuemap
        .get(&format!("0x{}", hex::encode(val_bytes)))
    {
        if let Some(mapped_value) = mapped_value {
            return Ok(Some(NumericValue::Float(*mapped_value)));
        } else {
            return Ok(None);
        }
    }

    let datatype = field_config.datatype.as_ref().unwrap_or({
        // Infer datatype from byte length when not explicitly set
        match val_bytes.len() {
            2 => &DataType::Uint16,
            4 => &DataType::Uint32,
            8 => &DataType::Double,
            _ => &DataType::Uint16, // Default fallback
        }
    });

    let value = match datatype {
        DataType::Int16 => {
            if val_bytes.len() < 2 {
                return Err(anyhow!("Insufficient bytes for int16"));
            }
            let value = i16::from_be_bytes([val_bytes[0], val_bytes[1]]);
            NumericValue::Int(value as i64)
        }
        DataType::Uint16 => {
            if val_bytes.len() < 2 {
                return Err(anyhow!("Insufficient bytes for uint16"));
            }
            let value = u16::from_be_bytes([val_bytes[0], val_bytes[1]]);
            NumericValue::Int(value as i64)
        }
        DataType::Int32 => {
            if val_bytes.len() < 4 {
                return Err(anyhow!("Insufficient bytes for int32"));
            }
            let value =
                i32::from_be_bytes([val_bytes[0], val_bytes[1], val_bytes[2], val_bytes[3]]);
            NumericValue::Int(value as i64)
        }
        DataType::Uint32 => {
            if val_bytes.len() < 4 {
                return Err(anyhow!("Insufficient bytes for uint32"));
            }
            let value =
                u32::from_be_bytes([val_bytes[0], val_bytes[1], val_bytes[2], val_bytes[3]]);
            NumericValue::Int(value as i64)
        }
        DataType::Int64 => {
            if val_bytes.len() < 8 {
                return Err(anyhow!("Insufficient bytes for int64"));
            }
            let mut bytes = [0u8; 8];
            bytes.copy_from_slice(&val_bytes[0..8]);
            let value = i64::from_be_bytes(bytes);
            NumericValue::Int(value)
        }
        DataType::Uint64 => {
            if val_bytes.len() < 8 {
                return Err(anyhow!("Insufficient bytes for uint64"));
            }
            let mut bytes = [0u8; 8];
            bytes.copy_from_slice(&val_bytes[0..8]);
            let value = u64::from_be_bytes(bytes);
            // Convert to i64, using saturation for values that don't fit
            NumericValue::Int(if value <= i64::MAX as u64 {
                value as i64
            } else {
                i64::MAX
            })
        }
        DataType::Float => {
            if val_bytes.len() < 4 {
                return Err(anyhow!("Insufficient bytes for float"));
            }
            let mut bytes = [0u8; 4];
            bytes.copy_from_slice(&val_bytes[0..4]);
            let value = f32::from_be_bytes(bytes) as f64;
            NumericValue::Float(value)
        }
        DataType::Double => {
            if val_bytes.len() < 8 {
                return Err(anyhow!("Insufficient bytes for double"));
            }
            let mut bytes = [0u8; 8];
            bytes.copy_from_slice(&val_bytes[0..8]);
            let value = f64::from_be_bytes(bytes);
            NumericValue::Float(value)
        }
    };

    Ok(Some(value))
}

/// Apply final type casting to get the desired output type
pub fn apply_typecast(value: NumericValue, typecast: Option<Typecast>) -> Result<RtValue> {
    match typecast {
        Some(Typecast::Int) => match value {
            NumericValue::Int(i) => Ok(RtValue::Int(i)),
            NumericValue::Float(f) => Ok(RtValue::Int(f as i64)),
        },
        Some(Typecast::Float) => Ok(RtValue::Float(value.as_f64())),
        Some(Typecast::Bool) => {
            let is_nonzero = match value {
                NumericValue::Int(i) => i != 0,
                NumericValue::Float(f) => f != 0.0,
            };
            Ok(RtValue::Bool(is_nonzero))
        }
        Some(Typecast::Str) => Ok(RtValue::String(value.as_f64().to_string())),
        None => match value {
            NumericValue::Int(i) => Ok(RtValue::Int(i)),
            NumericValue::Float(f) => Ok(RtValue::Float(f)),
        },
    }
}

/// Intermediate numeric value that preserves integer precision when possible
#[derive(Debug, Clone, PartialEq)]
pub enum NumericValue {
    /// Integer value
    Int(i64),
    /// Floating point value
    Float(f64),
}

impl NumericValue {
    /// Convert to f64 for compatibility with existing logic
    pub fn as_f64(&self) -> f64 {
        match self {
            NumericValue::Int(i) => *i as f64,
            NumericValue::Float(f) => *f,
        }
    }

    /// Check if this value is an integer
    pub fn is_integer(&self) -> bool {
        matches!(self, NumericValue::Int(_))
    }

    /// Apply multiplier and offset, converting to float when operations are applied
    pub fn apply_multiplier_offset(
        self,
        multiplier: Option<f64>,
        offset: Option<f64>,
    ) -> NumericValue {
        let mult = multiplier.unwrap_or(1.0);
        let offs = offset.unwrap_or(0.0);

        // If both multiplier and offset are identity values, preserve the original type
        if mult == 1.0 && offs == 0.0 {
            return self;
        }

        // If any multiplier or offset is applied, convert to float
        let result = self.as_f64() * mult + offs;
        NumericValue::Float(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_process_uint16() {
        let bytes = [0x12, 0x34]; // 0x1234 = 4660
        let field_config = FieldOpts {
            datatype: Some(DataType::Uint16),
            ..Default::default()
        };

        let result = process_reading(&bytes, &field_config).unwrap();
        assert_eq!(result, RtValue::Int(4660)); // Integer precision preserved!
    }

    #[test]
    fn test_process_with_multiplier_offset() {
        let bytes = [0x00, 0x64]; // 100
        let field_config = FieldOpts {
            datatype: Some(DataType::Uint16),
            multiplier: Some(0.1),
            offset: Some(5.0),
            typecast: Some(Typecast::Float),
            ..Default::default()
        };

        let result = process_reading(&bytes, &field_config).unwrap();
        assert_eq!(result, RtValue::Float(15.0)); // 100 * 0.1 + 5.0 = 15.0
    }

    #[test]
    fn test_process_string_value() {
        let bytes = b"123.45";
        let field_config = FieldOpts {
            parse_as: Some(ParseAs::Str),
            typecast: Some(Typecast::Float),
            ..Default::default()
        };

        let result = process_reading(bytes, &field_config).unwrap();
        assert_eq!(result, RtValue::Float(123.45));
    }

    #[test]
    fn test_value_mapping() {
        let bytes = [0x12, 0x34];
        let mut valuemap = HashMap::new();
        valuemap.insert("0x1234".to_string(), Some(999.0));

        let field_config = FieldOpts {
            datatype: Some(DataType::Uint16),
            valuemap,
            ..Default::default()
        };

        let result = process_reading(&bytes, &field_config).unwrap();
        assert_eq!(result, RtValue::Float(999.0));
    }

    #[test]
    fn test_integer_precision_preservation() {
        // Test that integers stay as integers when no multiplier/offset
        let bytes = [0x04, 0xD2]; // 1234 in uint16
        let field_config = FieldOpts {
            datatype: Some(DataType::Uint16),
            typecast: None,   // No explicit typecast
            multiplier: None, // No multiplier
            offset: None,     // No offset
            ..Default::default()
        };

        let result = process_reading(&bytes, &field_config).unwrap();
        assert_eq!(result, RtValue::Int(1234)); // Should be integer, not float

        // Test that integers become floats when offset is applied
        let field_config_with_offset = FieldOpts {
            datatype: Some(DataType::Uint16),
            typecast: None,
            multiplier: None,
            offset: Some(10.0), // Any offset converts to float
            ..Default::default()
        };

        let result_with_offset = process_reading(&bytes, &field_config_with_offset).unwrap();
        assert_eq!(result_with_offset, RtValue::Float(1244.0)); // 1234 + 10, becomes float

        // Test that fractional operations result in float
        let field_config_with_multiplier = FieldOpts {
            datatype: Some(DataType::Uint16),
            typecast: None,
            multiplier: Some(0.5), // Fractional multiplier
            offset: None,
            ..Default::default()
        };

        let result_with_multiplier =
            process_reading(&bytes, &field_config_with_multiplier).unwrap();
        assert_eq!(result_with_multiplier, RtValue::Float(617.0)); // 1234 * 0.5, becomes float
    }

    #[test]
    fn test_large_unsigned_values() {
        // Test that very large uint64 values saturate to i64::MAX
        let bytes = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]; // u64::MAX
        let field_config = FieldOpts {
            datatype: Some(DataType::Uint64),
            typecast: None,
            ..Default::default()
        };

        let result = process_reading(&bytes, &field_config).unwrap();
        assert_eq!(result, RtValue::Int(i64::MAX)); // Should saturate to i64::MAX

        // Test normal uint64 value that fits in i64
        let bytes = [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0xD2]; // 1234 in uint64
        let result = process_reading(&bytes, &field_config).unwrap();
        assert_eq!(result, RtValue::Int(1234)); // Should preserve exact value
    }

    #[test]
    fn test_typecast_to_int() {
        let bytes = [0x00, 0x64]; // 100
        let field_config = FieldOpts {
            datatype: Some(DataType::Uint16),
            typecast: Some(Typecast::Int),
            ..Default::default()
        };

        let result = process_reading(&bytes, &field_config).unwrap();
        assert_eq!(result, RtValue::Int(100));
    }
}
