use std::collections::{BTreeMap, HashMap};

use chrono::{DateTime, offset::Utc};
use serde::{Deserialize, Serialize};

use crate::node_mgmt::config::Device;

use super::payload::DeviceDataExtraValue;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub enum RtValue {
    None,
    Bool(bool),
    Float(f64),
    Int(i64),
    String(String),
}

#[derive(Debug)]
pub struct Record {
    timestamp: Option<DateTime<Utc>>,
    fields: HashMap<String, RtValue>,
}

impl Record {
    pub fn new() -> Self {
        Record {
            timestamp: None,
            fields: HashMap::new(),
        }
    }

    // Getter and setter for the timestamp field
    pub fn get_timestamp(&self) -> Option<DateTime<Utc>> {
        self.timestamp
    }

    pub fn set_timestamp(&mut self, timestamp: DateTime<Utc>) {
        self.timestamp = Some(timestamp);
    }

    // Method to add a value to the fields HashMap
    pub fn set_field(&mut self, key: String, value: RtValue) {
        self.fields.insert(key, value);
    }

    // Method to retrieve a value from the fields HashMap
    pub fn get_field(&self, key: &str) -> Option<&RtValue> {
        self.fields.get(key)
    }

    pub fn all_fields(&self) -> &HashMap<String, RtValue> {
        &self.fields
    }

    pub fn all_fields_as_device_data_extra(&self) -> BTreeMap<String, DeviceDataExtraValue> {
        self.fields
            .iter()
            .filter(|(_, v)| !matches!(v, RtValue::None))
            .map(|(k, v)| {
                let value = match v {
                    RtValue::Bool(b) => DeviceDataExtraValue::Boolean(*b),
                    RtValue::Float(f) => DeviceDataExtraValue::Number(*f),
                    RtValue::Int(i) => DeviceDataExtraValue::Integer(*i),
                    RtValue::String(s) => DeviceDataExtraValue::String(s.to_string()),
                    RtValue::None => unreachable!(), // Already filtered out above
                };
                (k.clone(), value)
            })
            .collect()
    }
}

impl Default for Record {
    fn default() -> Self {
        Record::new()
    }
}

/// Reference to a device
///
/// This is used to reference a device by key metadata, rather than full config.
#[derive(Clone, Debug)]
pub struct DeviceRef {
    pub key: String,
    pub vendor_id: String,
}

impl DeviceRef {
    pub fn new(key: String, vendor_id: String) -> Self {
        Self { key, vendor_id }
    }

    pub fn from_device(device: &Device) -> Self {
        Self {
            key: device.key.clone(),
            vendor_id: device.vendor_id.clone(),
        }
    }
}

#[derive(Debug)]
pub struct DeviceReading {
    pub device: DeviceRef,
    pub record: Record,
}

#[derive(Clone, Debug)]
pub struct Reading {
    pub field: String,
    pub value: RtValue,
}

// #[derive(Debug)]
// pub struct DeviceReadings {
//     pub device: Device,
//     pub records: Vec<Record>,
// }
