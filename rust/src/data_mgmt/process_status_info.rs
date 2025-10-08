//! Status info processing module
//!
//! This module provides functionality to process status info readings,
//! extracting bit values from registers and mapping them to status levels.

use anyhow::{Result, anyhow};

use crate::data_mgmt::payload::StatusReading;
use crate::data_mgmt::process::extract_bits;
use crate::node_mgmt::drivers::{FieldOpts, StatusInfoOpts};

/// Process a raw reading value according to the status info configuration
///
/// This function:
/// 1. Extracts bits from the register value using start_bit and length_bits
/// 2. Maps the extracted value to a status level using status_level_value_map
/// 3. Returns the content message and status level
pub fn process_status_info(
    val_bytes: &[u8],
    status_info_config: &StatusInfoOpts,
) -> Result<StatusReading> {
    if val_bytes.is_empty() {
        return Err(anyhow!("Empty byte array for status info"));
    }

    // Create FieldOpts from StatusInfoOptsValue to reuse extract_bits function
    let field_opts = field_opts_from_status_info(status_info_config);

    // Extract the bit value
    let extracted_value = extract_bits(val_bytes, &field_opts)? as u8;

    // Map the extracted value to a status level
    let status_level = map_value_to_level(extracted_value, status_info_config)?;

    // Get the content message
    let content = status_info_config
        .content
        .clone()
        .ok_or_else(|| anyhow!("Status info missing content field"))?;

    Ok(StatusReading {
        c: content,
        l: status_level,
    })
}

/// Convert StatusInfoOpts to FieldOpts for bit extraction
fn field_opts_from_status_info(status_info: &StatusInfoOpts) -> FieldOpts {
    FieldOpts {
        start_bit: status_info.start_bit,
        length_bits: status_info.length_bits,
        bit_order: status_info.bit_order,
        register: status_info.register,
        fncode: status_info.fncode,
        words: status_info.words,
        order: status_info.order,
        ..FieldOpts::default()
    }
}

/// Map an extracted value to a status level using the status_level_value_map
///
/// If the value is in the map, return the corresponding status level.
/// If the value is not in the map or the map is empty, return the value as-is.
fn map_value_to_level(value: u8, status_info_config: &StatusInfoOpts) -> Result<u8> {
    // Look up the value in the mapping
    for (map_value, status_level) in &status_info_config.status_level_value_map {
        if *map_value == value {
            return Ok(*status_level);
        }
    }

    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_mgmt::drivers::BitOrder;
    use std::num::NonZeroU16;

    fn create_test_status_info(
        start_bit: u8,
        length_bits: u16,
        content: &str,
        status_level_map: Vec<(u8, u8)>,
    ) -> StatusInfoOpts {
        StatusInfoOpts {
            start_bit: Some(start_bit),
            length_bits: Some(NonZeroU16::new(length_bits).unwrap()),
            bit_order: Some(BitOrder::Lsb),
            content: Some(content.to_string()),
            status_level_value_map: status_level_map,
            register: None,
            fncode: None,
            words: None,
            order: None,
        }
    }

    #[test]
    fn test_process_status_info_single_bit() {
        // 0x00AA = 0b00000000_10101010
        let bytes = [0x00, 0xAA];

        // Extract bit 1 (second bit from right in LSB)
        let status_info = create_test_status_info(1, 1, "Low Oil Pressure", vec![(0, 0), (1, 3)]);

        let result = process_status_info(&bytes, &status_info).unwrap();
        assert_eq!(result.c, "Low Oil Pressure");
        assert_eq!(result.l, 3); // Bit 1 is set, maps to level 3
    }

    #[test]
    fn test_process_status_info_multiple_bits() {
        // 0x00AA = 0b00000000_10101010
        let bytes = [0x00, 0xAA];

        // Extract bits 4-7 (4 bits starting from bit 4)
        // Bits 4-7 of 0b00000000_10101010 = 0b1010 = 10
        let status_info = create_test_status_info(
            4,
            4,
            "Temperature Warning",
            vec![(0, 0), (5, 1), (10, 2), (15, 3)],
        );

        let result = process_status_info(&bytes, &status_info).unwrap();
        assert_eq!(result.c, "Temperature Warning");
        assert_eq!(result.l, 2); // Value 10 maps to level 2
    }

    #[test]
    fn test_process_status_info_no_mapping() {
        // When no mapping is provided, the extracted value is used as the level
        let bytes = [0x00, 0x05]; // 0b00000000_00000101 = 5

        let status_info = StatusInfoOpts {
            start_bit: Some(0),
            length_bits: Some(NonZeroU16::new(8).unwrap()),
            bit_order: Some(BitOrder::Lsb),
            content: Some("Generic Status".to_string()),
            status_level_value_map: vec![], // Empty mapping
            register: None,
            fncode: None,
            words: None,
            order: None,
        };

        let result = process_status_info(&bytes, &status_info).unwrap();
        assert_eq!(result.c, "Generic Status");
        assert_eq!(result.l, 5); // Value used directly as level
    }

    #[test]
    fn test_process_status_info_value_not_in_map() {
        // When a value is not in the map, it's used as-is (not an error)
        let bytes = [0x00, 0xFF]; // 0b00000000_11111111 = 255

        let status_info = create_test_status_info(
            0,
            8,
            "Test Status",
            vec![(0, 0), (1, 3)], // Only maps 0 and 1, but 255 is not mapped
        );

        let result = process_status_info(&bytes, &status_info);
        assert!(result.is_ok());
        let status = result.unwrap();
        assert_eq!(status.c, "Test Status");
        assert_eq!(status.l, 255); // Value used directly as level
    }

    #[test]
    fn test_process_status_info_empty_bytes() {
        let bytes = [];
        let status_info = create_test_status_info(0, 1, "Test", vec![(0, 0), (1, 3)]);

        let result = process_status_info(&bytes, &status_info);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Empty byte array"));
    }

    #[test]
    fn test_process_status_info_missing_content() {
        let bytes = [0x00, 0xAA];
        let mut status_info = create_test_status_info(0, 1, "Test", vec![(0, 0), (1, 3)]);
        status_info.content = None; // Remove content

        let result = process_status_info(&bytes, &status_info);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("missing content field")
        );
    }
}
