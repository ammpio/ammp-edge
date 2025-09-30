//! Data processing module
//!
//! This module provides data processing functionality equivalent to the Python
//! processor/process_reading.py module. It handles data type conversion, scaling,
//! offset application, and type casting for readings from various device types.

use crate::data_mgmt::models::RtValue;
use crate::node_mgmt::drivers::{BitOrder, DataType, FieldOpts, ParseAs, Typecast};
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

    let value = if field_config.multiplier.is_some() || field_config.offset.is_some() {
        value.apply_multiplier_offset(field_config.multiplier, field_config.offset)
    } else {
        value
    };

    let typecast_value = apply_typecast(value, field_config.typecast)?;

    Ok(typecast_value)
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
    // Apply bitwise extraction if start_bit is specified
    let val_bytes = if field_config.start_bit.is_some() {
        extract_bits(val_bytes, field_config)?
    } else {
        val_bytes.to_vec()
    };

    // Check value mapping first (hex format)
    if let Some(mapped_value) = field_config
        .valuemap
        .get(&format!("0x{}", hex::encode(&val_bytes)))
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
        Some(Typecast::Int) => Ok(RtValue::Int(value.as_i64())),
        Some(Typecast::Float) => Ok(RtValue::Float(value.as_f64())),
        Some(Typecast::Bool) => Ok(RtValue::Bool(value.is_nonzero())),
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
    /// Convert to float
    pub fn as_f64(&self) -> f64 {
        match self {
            NumericValue::Int(i) => *i as f64,
            NumericValue::Float(f) => *f,
        }
    }

    /// Convert to integer
    pub fn as_i64(&self) -> i64 {
        match self {
            NumericValue::Int(i) => *i,
            NumericValue::Float(f) => *f as i64,
        }
    }

    /// Apply multiplier and offset, converting to float when operations are applied
    pub fn apply_multiplier_offset(
        self,
        multiplier: Option<f64>,
        offset: Option<f64>,
    ) -> NumericValue {
        let result = self.as_f64() * multiplier.unwrap_or(1.0) + offset.unwrap_or(0.0);
        NumericValue::Float(result)
    }

    /// Check if non-zero
    pub fn is_nonzero(&self) -> bool {
        match self {
            NumericValue::Int(i) => *i != 0,
            NumericValue::Float(f) => *f != 0.0,
        }
    }
}

/// Extract specific bits from byte array according to start_bit, length_bits, and bit_order
///
/// Only works with exactly 2 bytes (16 bits / single Modbus register)
///
/// Bit numbering follows the bit_order parameter:
/// - LSB (default): bits are numbered from right to left (0 = rightmost/least significant)
/// - MSB: bits are numbered from left to right (0 = leftmost/most significant)
///
/// Returns a byte array containing the extracted value as an unsigned integer
fn extract_bits(val_bytes: &[u8], field_config: &FieldOpts) -> Result<Vec<u8>> {
    const SOURCE_BITS: usize = 16;
    const SOURCE_BYTES: usize = 2;

    let start_bit = field_config
        .start_bit
        .ok_or_else(|| anyhow!("start_bit must be specified for bit extraction"))?
        as usize;

    let length_bits = field_config.length_bits.map(|n| n.get()).unwrap_or(1) as usize;

    let bit_order = field_config.bit_order.unwrap_or(BitOrder::Lsb);

    if val_bytes.len() != SOURCE_BYTES {
        return Err(anyhow!(
            "Bitwise extraction only supported for exactly 2 bytes (16 bits), got {} bytes",
            val_bytes.len()
        ));
    }

    let full_value = u16::from_be_bytes(val_bytes.try_into().unwrap());

    if start_bit >= SOURCE_BITS {
        return Err(anyhow!(
            "start_bit {} out of range for {} bits (MSB ordering)",
            start_bit,
            SOURCE_BITS
        ));
    }

    let actual_start_bit = match bit_order {
        BitOrder::Lsb => {
            // LSB: bit 0 is rightmost, so we count from the right
            start_bit
        }
        BitOrder::Msb => {
            // MSB: bit 0 is leftmost, so we need to convert to LSB numbering
            SOURCE_BITS - start_bit - length_bits
        }
    };

    // Note that the order of the returned bits is not altered based on BitOrder;
    // only the range of bits that's returned

    if actual_start_bit + length_bits > SOURCE_BITS {
        return Err(anyhow!(
            "Bit range (start={}, length={}) exceeds available bits ({})",
            start_bit,
            length_bits,
            SOURCE_BITS
        ));
    }

    // Extract the bits using a mask
    let mask = if length_bits >= SOURCE_BITS {
        u16::MAX
    } else {
        (1u16 << length_bits) - 1
    };
    let extracted = (full_value >> actual_start_bit) & mask;

    Ok(extracted.to_be_bytes().to_vec())
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

    #[test]
    fn test_process_float() {
        let bytes = [0x48, 0x9e, 0xcc, 0x5a]; // 0x489ecc5a as f32 = 325218.8125
        let field_config = FieldOpts {
            datatype: Some(DataType::Float),
            ..Default::default()
        };

        let result = process_reading(&bytes, &field_config).unwrap();

        if let RtValue::Float(f) = result {
            // Check that it's approximately 325218.8125
            assert!(
                (f - 325218.8125).abs() < 0.001,
                "Expected ~325218.8125, got {}",
                f
            );
        } else {
            panic!("Expected RtValue::Float, got {:?}", result);
        }

    #[test]
    fn test_bit_extraction_lsb_single_bit() {
        // 0x00AA = 0b00000000_10101010
        let bytes = [0x00, 0xAA];
        let field_config = FieldOpts {
            start_bit: Some(1), // Second bit from right (LSB)
            length_bits: Some(1.try_into().unwrap()),
            bit_order: Some(BitOrder::Lsb),
            datatype: Some(DataType::Uint16),
        assert_eq!(result, RtValue::Int(1)); // Bit 1 is set
    }

    #[test]
    fn test_bit_extraction_lsb_multiple_bits() {
        // 0x00AA = 0b00000000_10101010, extract bits 4-7 (4 bits starting from bit 4)
        let bytes = [0x00, 0xAA];
        let field_config = FieldOpts {
            start_bit: Some(4),
            length_bits: Some(4.try_into().unwrap()),
            bit_order: Some(BitOrder::Lsb),
            datatype: Some(DataType::Uint16),
            ..Default::default()
        };

        let result = process_reading(&bytes, &field_config).unwrap();
        // Bits 4-7 of 0b00000000_10101010 = 0b1010 = 10
        assert_eq!(result, RtValue::Int(10));
    }

    #[test]
    fn test_bit_extraction_msb_ordering() {
        // 0x00AA = 0b00000000_10101010, extract 4 bits starting from bit 0 (MSB)
        let bytes = [0x00, 0xAA];
        let field_config = FieldOpts {
            start_bit: Some(0),
            length_bits: Some(4.try_into().unwrap()),
            bit_order: Some(BitOrder::Msb),
            datatype: Some(DataType::Uint16),
            ..Default::default()
        };

        let result = process_reading(&bytes, &field_config).unwrap();
        // From MSB: bits 0-3 of 0b00000000_10101010 = 0b0000 = 0
        assert_eq!(result, RtValue::Int(0));
    }

    #[test]
    fn test_bit_extraction_two_bytes() {
        // 0x12 0x34 = 0b00010010_00110100
        let bytes = [0x12, 0x34];
        let field_config = FieldOpts {
            start_bit: Some(4),                       // Start from bit 4 (LSB)
            length_bits: Some(8.try_into().unwrap()), // Extract 8 bits
            bit_order: Some(BitOrder::Lsb),
            datatype: Some(DataType::Uint16),
            ..Default::default()
        };

        let result = process_reading(&bytes, &field_config).unwrap();
        // Bits 4-11 of 0x1234 = 0b00010010_00110100
        // Extract 8 bits from bit 4: 0b00100011 = 0x23 = 35
        assert_eq!(result, RtValue::Int(35));
    }

    #[test]
    fn test_bit_extraction_wrong_size() {
        // Bitwise extraction should only work with exactly 2 bytes
        let bytes = [0x12]; // Only 1 byte
        let field_config = FieldOpts {
            start_bit: Some(0),
            length_bits: Some(1.try_into().unwrap()),
            bit_order: Some(BitOrder::Lsb),
            ..Default::default()
        };

        let result = process_reading(&bytes, &field_config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("exactly 2 bytes"));
    }

    #[test]
    fn test_bit_extraction_default_length() {
        // Default length_bits should be 1
        let bytes = [0x00, 0xAA]; // 0b00000000_10101010
        let field_config = FieldOpts {
            start_bit: Some(7), // Bit 7 (LSB)
            bit_order: Some(BitOrder::Lsb),
            datatype: Some(DataType::Uint16),
            ..Default::default()
        };

        let result = process_reading(&bytes, &field_config).unwrap();
        assert_eq!(result, RtValue::Int(1)); // Bit 7 is set
    }
}
