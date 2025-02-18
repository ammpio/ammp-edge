use std::collections::HashMap;

use chrono::{offset::Utc, DateTime};
use serde::{Deserialize, Serialize};

use crate::node_mgmt::config::Device;

use super::payload::DeviceDataExtraValue;

#[derive(Clone, Debug, Deserialize, Serialize)]

pub enum RtValue {
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

    pub fn all_fields_as_device_data_extra(&self) -> HashMap<String, DeviceDataExtraValue> {
        self.fields
            .iter()
            .map(|(k, v)| {
                let value = match v {
                    RtValue::Bool(b) => DeviceDataExtraValue::Boolean(*b),
                    RtValue::Float(f) => DeviceDataExtraValue::Number(*f),
                    RtValue::Int(i) => DeviceDataExtraValue::Integer(*i),
                    RtValue::String(s) => DeviceDataExtraValue::String(s.to_string()),
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

#[derive(Debug)]
pub struct DeviceReading {
    pub device: Device,
    pub record: Record,
}

// #[derive(Debug)]
// pub struct DeviceReadings {
//     pub device: Device,
//     pub records: Vec<Record>,
// }
