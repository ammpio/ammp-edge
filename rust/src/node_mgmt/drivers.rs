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

pub use derived_models::driver::{DataType, DriverSchema, FieldOpts, RegisterOrder};

/// Cache for loaded driver definitions to avoid reloading on every reading cycle
static DRIVER_CACHE: Lazy<Mutex<HashMap<String, DriverSchema>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Load driver definition for a specific driver name
///
/// First checks the config.drivers object, then falls back to filesystem.
pub fn load_driver(config: &Config, driver_name: &str) -> Result<DriverSchema> {
    if let Some(inline_driver) = config.drivers.get(driver_name) {
        log::debug!("Loading driver '{}' from inline config", driver_name);
        let json_value = serde_json::to_value(inline_driver)?;
        let driver_schema: DriverSchema = serde_json::from_value(json_value)?;
        return Ok(driver_schema);
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

        let driver_schema: DriverSchema = serde_json::from_str(&driver_content).map_err(|e| {
            anyhow!(
                "Failed to parse driver JSON {}: {}",
                driver_path.display(),
                e
            )
        })?;

        // Cache the loaded driver
        {
            let mut cache = DRIVER_CACHE.lock().unwrap();
            cache.insert(driver_name.to_string(), driver_schema.clone());
        }

        log::debug!("Cached driver '{}'", driver_name);
        return Ok(driver_schema);
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

/// Resolve field definition by merging common and field-specific settings
///
/// Field-specific settings override common settings
pub fn resolve_field_definition(driver: &DriverSchema, field_name: &str) -> Result<FieldOpts> {
    let field_def = driver
        .fields
        .get(field_name)
        .ok_or_else(|| anyhow!("Field '{}' not found in driver", field_name))?;

    // Start with default values
    let mut resolved = FieldOpts::default();

    // Apply common settings first
    merge_field_opts(&mut resolved, &driver.common);

    // Apply field-specific settings (they override common)
    merge_field_opts(&mut resolved, field_def);

    Ok(resolved)
}

/// Merge source FieldOpts into target, with source taking precedence
fn merge_field_opts(target: &mut FieldOpts, source: &FieldOpts) {
    if let Some(register) = source.register {
        target.register = Some(register);
    }
    if let Some(words) = source.words {
        target.words = Some(words);
    }
    if let Some(datatype) = source.datatype {
        target.datatype = Some(datatype);
    }
    if let Some(fncode) = source.fncode {
        target.fncode = Some(fncode);
    }
    if let Some(typecast) = source.typecast {
        target.typecast = Some(typecast);
    }
    if let Some(multiplier) = source.multiplier {
        target.multiplier = Some(multiplier);
    }
    if let Some(offset) = source.offset {
        target.offset = Some(offset);
    }
    if let Some(ref unit) = source.unit {
        target.unit = Some(unit.clone());
    }
    if let Some(ref description) = source.description {
        target.description = Some(description.clone());
    }
    if !source.datamap.is_empty() {
        target.datamap = source.datamap.clone();
    }
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

        let driver: DriverSchema = serde_json::from_value(driver_json).unwrap();

        assert_eq!(driver.common.fncode, Some(4));
        assert_eq!(driver.fields.len(), 2);

        let voltage_field = resolve_field_definition(&driver, "voltage").unwrap();
        assert_eq!(voltage_field.register, Some(10));
        assert_eq!(voltage_field.fncode, Some(4)); // From common
        assert_eq!(voltage_field.multiplier, Some(0.1));
        assert_eq!(voltage_field.unit, Some("V".to_string()));

        let power_field = resolve_field_definition(&driver, "power").unwrap();
        assert_eq!(power_field.register, Some(20));
        assert_eq!(power_field.words.map(|w| w.get()), Some(2)); // Overrides common
        assert_eq!(
            power_field.datatype.as_ref().map(|d| d.to_string()),
            Some("uint32".to_string())
        ); // Overrides common
    }

    #[test]
    fn test_resolve_field_without_register() {
        let driver_json = json!({
            "fields": {
                "field_without_register": {
                    "multiplier": 0.1,
                    "unit": "V"
                }
            }
        });

        let driver: DriverSchema = serde_json::from_value(driver_json).unwrap();
        let result = resolve_field_definition(&driver, "field_without_register");

        // Should succeed - register validation is now reader-specific
        assert!(result.is_ok());
        let field = result.unwrap();
        assert_eq!(field.register, None);
        assert_eq!(field.multiplier, Some(0.1));
        assert_eq!(field.unit, Some("V".to_string()));
    }
}
