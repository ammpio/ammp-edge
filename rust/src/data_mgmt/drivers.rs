//! Driver loading and parsing functionality
//!
//! This module handles loading driver definitions from both filesystem JSON files
//! and inline config definitions, with config taking precedence.

use anyhow::{Result, anyhow};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::fs;
use std::sync::Mutex;

use crate::node_mgmt::config::Config;

/// Cache for loaded driver definitions to avoid reloading on every reading cycle
static DRIVER_CACHE: Lazy<Mutex<HashMap<String, DriverDefinition>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Load driver definition for a specific driver name
///
/// First checks the config.drivers object, then falls back to filesystem.
pub fn load_driver(config: &Config, driver_name: &str) -> Result<DriverDefinition> {
    if let Some(inline_driver) = config.drivers.get(driver_name) {
        log::debug!("Loading driver '{}' from inline config", driver_name);
        let json_value = serde_json::to_value(inline_driver)?;
        return parse_driver_from_json_value(&json_value);
    }

    // Check cache for filesystem drivers
    {
        let cache = DRIVER_CACHE.lock().unwrap();
        if let Some(cached_driver) = cache.get(driver_name) {
            log::debug!("Using cached driver '{}'", driver_name);
            return Ok(cached_driver.clone());
        }
    }

    // Fall back to filesystem and cache the result
    let driver_path = crate::helpers::base_path::ROOT_DIR
        .join("drivers")
        .join(format!("{}.json", driver_name));
    if driver_path.exists() {
        log::debug!(
            "Loading driver '{}' from filesystem: {}",
            driver_name,
            driver_path.display()
        );
        let driver_content = fs::read_to_string(&driver_path).map_err(|e| {
            anyhow!(
                "Failed to read driver file {}: {}",
                driver_path.display(),
                e
            )
        })?;

        let driver_json: serde_json::Value =
            serde_json::from_str(&driver_content).map_err(|e| {
                anyhow!(
                    "Failed to parse driver JSON {}: {}",
                    driver_path.display(),
                    e
                )
            })?;

        let driver_definition = parse_driver_from_json_value(&driver_json)?;

        // Cache the loaded driver
        {
            let mut cache = DRIVER_CACHE.lock().unwrap();
            cache.insert(driver_name.to_string(), driver_definition.clone());
        }

        log::debug!("Cached driver '{}'", driver_name);
        return Ok(driver_definition);
    }

    Err(anyhow!(
        "Driver '{}' not found in config or filesystem",
        driver_name
    ))
}

/// Clear the driver cache (useful for testing or if drivers are updated)
#[allow(dead_code)]
pub fn clear_driver_cache() {
    let mut cache = DRIVER_CACHE.lock().unwrap();
    cache.clear();
    log::debug!("Driver cache cleared");
}

/// Parse driver definition from JSON value (either from config or file)
fn parse_driver_from_json_value(json: &serde_json::Value) -> Result<DriverDefinition> {
    let obj = json
        .as_object()
        .ok_or_else(|| anyhow!("Driver definition must be a JSON object"))?;

    // Parse common section (if present)
    let common = if let Some(common_val) = obj.get("common") {
        Some(parse_field_definition(common_val)?)
    } else {
        None
    };

    // Parse fields section
    let fields_obj = obj
        .get("fields")
        .and_then(|v| v.as_object())
        .ok_or_else(|| anyhow!("Driver definition must have 'fields' object"))?;

    let mut fields = HashMap::new();
    for (field_name, field_def) in fields_obj {
        let parsed_field = parse_field_definition(field_def)?;
        fields.insert(field_name.clone(), parsed_field);
    }

    Ok(DriverDefinition { common, fields })
}

/// Parse a field definition (either common or specific field)
fn parse_field_definition(json: &serde_json::Value) -> Result<FieldDefinition> {
    let obj = json
        .as_object()
        .ok_or_else(|| anyhow!("Field definition must be a JSON object"))?;

    Ok(FieldDefinition {
        register: obj
            .get("register")
            .and_then(|v| v.as_u64())
            .map(|v| v as u16),
        words: obj.get("words").and_then(|v| v.as_u64()).map(|v| v as u16),
        datatype: obj
            .get("datatype")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        fncode: obj.get("fncode").and_then(|v| v.as_u64()).map(|v| v as u8),
        typecast: obj
            .get("typecast")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        multiplier: obj.get("multiplier").and_then(|v| v.as_f64()),
        offset: obj.get("offset").and_then(|v| v.as_f64()),
        unit: obj
            .get("unit")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        description: obj
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        valuemap: parse_valuemap(obj.get("valuemap")),
    })
}

/// Parse valuemap from JSON (if present)
fn parse_valuemap(json: Option<&serde_json::Value>) -> Option<HashMap<String, f64>> {
    let obj = json?.as_object()?;
    let mut valuemap = HashMap::new();

    for (key, value) in obj {
        if let Some(num_val) = value.as_f64() {
            valuemap.insert(key.clone(), num_val);
        }
    }

    if valuemap.is_empty() {
        None
    } else {
        Some(valuemap)
    }
}

/// Resolve field definition by merging common and field-specific settings
///
/// Field-specific settings override common settings
pub fn resolve_field_definition(
    driver: &DriverDefinition,
    field_name: &str,
) -> Result<ResolvedFieldDefinition> {
    let field_def = driver
        .fields
        .get(field_name)
        .ok_or_else(|| anyhow!("Field '{}' not found in driver", field_name))?;

    // Start with common settings as base
    let mut resolved = ResolvedFieldDefinition {
        register: None,
        words: 1,                       // Default to 1 word
        datatype: "uint16".to_string(), // Default datatype
        fncode: 3,                      // Default to holding registers
        typecast: None,
        multiplier: None,
        offset: None,
        unit: None,
        description: None,
        valuemap: None,
    };

    // Apply common settings first
    if let Some(ref common) = driver.common {
        if common.words.is_some() {
            resolved.words = common.words.unwrap();
        }
        if let Some(ref dt) = common.datatype {
            resolved.datatype = dt.clone();
        }
        if common.fncode.is_some() {
            resolved.fncode = common.fncode.unwrap();
        }
        if let Some(ref tc) = common.typecast {
            resolved.typecast = Some(tc.clone());
        }
        if common.multiplier.is_some() {
            resolved.multiplier = common.multiplier;
        }
        if common.offset.is_some() {
            resolved.offset = common.offset;
        }
        if let Some(ref unit) = common.unit {
            resolved.unit = Some(unit.clone());
        }
        if let Some(ref desc) = common.description {
            resolved.description = Some(desc.clone());
        }
        if let Some(ref vm) = common.valuemap {
            resolved.valuemap = Some(vm.clone());
        }
    }

    // Override with field-specific settings
    if let Some(reg) = field_def.register {
        resolved.register = Some(reg);
    }
    if let Some(words) = field_def.words {
        resolved.words = words;
    }
    if let Some(ref dt) = field_def.datatype {
        resolved.datatype = dt.clone();
    }
    if let Some(fncode) = field_def.fncode {
        resolved.fncode = fncode;
    }
    if let Some(ref tc) = field_def.typecast {
        resolved.typecast = Some(tc.clone());
    }
    if let Some(mult) = field_def.multiplier {
        resolved.multiplier = Some(mult);
    }
    if let Some(off) = field_def.offset {
        resolved.offset = Some(off);
    }
    if let Some(ref unit) = field_def.unit {
        resolved.unit = Some(unit.clone());
    }
    if let Some(ref desc) = field_def.description {
        resolved.description = Some(desc.clone());
    }
    if let Some(ref vm) = field_def.valuemap {
        resolved.valuemap = Some(vm.clone());
    }

    // Ensure we have a register address
    if resolved.register.is_none() {
        return Err(anyhow!(
            "Field '{}' missing required 'register' property",
            field_name
        ));
    }

    Ok(resolved)
}

/// Driver definition loaded from JSON
#[derive(Debug, Clone)]
pub struct DriverDefinition {
    /// Common settings applied to all fields
    pub common: Option<FieldDefinition>,
    /// Field-specific definitions
    pub fields: HashMap<String, FieldDefinition>,
}

/// Field definition (either common or field-specific)
#[derive(Debug, Clone)]
pub struct FieldDefinition {
    /// Modbus register address
    pub register: Option<u16>,
    /// Number of 16-bit words to read
    pub words: Option<u16>,
    /// Data type for parsing
    pub datatype: Option<String>,
    /// Modbus function code
    pub fncode: Option<u8>,
    /// Type casting for final output
    pub typecast: Option<String>,
    /// Multiplier to apply
    pub multiplier: Option<f64>,
    /// Offset to apply
    pub offset: Option<f64>,
    /// Unit of measurement
    pub unit: Option<String>,
    /// Human-readable description
    pub description: Option<String>,
    /// Value mapping
    pub valuemap: Option<HashMap<String, f64>>,
}

/// Field definition with all values resolved (common + field-specific)
#[derive(Debug, Clone)]
pub struct ResolvedFieldDefinition {
    /// Modbus register address (required)
    pub register: Option<u16>,
    /// Number of 16-bit words to read
    pub words: u16,
    /// Data type for parsing
    pub datatype: String,
    /// Modbus function code
    pub fncode: u8,
    /// Type casting for final output
    pub typecast: Option<String>,
    /// Multiplier to apply
    pub multiplier: Option<f64>,
    /// Offset to apply
    pub offset: Option<f64>,
    /// Unit of measurement
    pub unit: Option<String>,
    /// Human-readable description
    pub description: Option<String>,
    /// Value mapping
    pub valuemap: Option<HashMap<String, f64>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_driver_with_common() {
        let driver_json = json!({
            "common": {
                "fncode": 4,
                "words": 1,
                "datatype": "uint16",
                "typecast": "float"
            },
            "fields": {
                "voltage": {
                    "register": 10,
                    "multiplier": 0.1,
                    "unit": "V"
                },
                "power": {
                    "register": 20,
                    "words": 2,
                    "datatype": "uint32",
                    "multiplier": 10.0,
                    "unit": "W"
                }
            }
        });

        let driver = parse_driver_from_json_value(&driver_json).unwrap();

        assert!(driver.common.is_some());
        assert_eq!(driver.common.as_ref().unwrap().fncode, Some(4));
        assert_eq!(driver.fields.len(), 2);

        let voltage_field = resolve_field_definition(&driver, "voltage").unwrap();
        assert_eq!(voltage_field.register, Some(10));
        assert_eq!(voltage_field.fncode, 4); // From common
        assert_eq!(voltage_field.multiplier, Some(0.1));
        assert_eq!(voltage_field.unit, Some("V".to_string()));

        let power_field = resolve_field_definition(&driver, "power").unwrap();
        assert_eq!(power_field.register, Some(20));
        assert_eq!(power_field.words, 2); // Overrides common
        assert_eq!(power_field.datatype, "uint32"); // Overrides common
    }

    #[test]
    fn test_resolve_field_missing_register() {
        let driver_json = json!({
            "fields": {
                "invalid_field": {
                    "multiplier": 0.1
                }
            }
        });

        let driver = parse_driver_from_json_value(&driver_json).unwrap();
        let result = resolve_field_definition(&driver, "invalid_field");

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("missing required 'register'")
        );
    }
}
