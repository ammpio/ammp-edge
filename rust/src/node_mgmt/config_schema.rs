// Auto-generated from config.schema.json using `cargo typify --no-builder ...`
// (from https://github.com/oxidecomputer/typify/blob/main/cargo-typify/README.md)
//
// NB: Need to remove #[serde(deny_unknown_fields)] line from pub struct ReadingsToTake
//

#![allow(clippy::redundant_closure_call)]
#![allow(clippy::needless_lifetimes)]
#![allow(clippy::match_single_binding)]
#![allow(clippy::clone_on_copy)]

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ASchema {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
    pub field: String,
    pub source: String,
    pub typecast: TypecastOfOutput,
}
impl From<&ASchema> for ASchema {
    fn from(value: &ASchema) -> Self {
        value.clone()
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AddressOfTheDevice {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baudrate: Option<BaudRateForSerialDevice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mac: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slaveaddr: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit_id: Option<i64>,
}
impl From<&AddressOfTheDevice> for AddressOfTheDevice {
    fn from(value: &AddressOfTheDevice) -> Self {
        value.clone()
    }
}
#[doc = "This document records the configuration of an AMMP Edge node"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AmmpEdgeConfiguration {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calc_vendor_id: Option<String>,
    pub devices: std::collections::HashMap<String, Device>,
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub drivers:
        std::collections::HashMap<String, std::collections::HashMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub output: Vec<ASchema>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub push_throttle_delay: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub push_timeout: Option<i64>,
    #[serde(default = "defaults::default_u64::<i64, 60>")]
    pub read_interval: i64,
    #[serde(default)]
    pub read_roundtime: bool,
    pub readings: ReadingsToTake,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volatile_q_size: Option<i64>,
}
impl From<&AmmpEdgeConfiguration> for AmmpEdgeConfiguration {
    fn from(value: &AmmpEdgeConfiguration) -> Self {
        value.clone()
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct BaudRateForSerialDevice(i64);
impl std::ops::Deref for BaudRateForSerialDevice {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl From<BaudRateForSerialDevice> for i64 {
    fn from(value: BaudRateForSerialDevice) -> Self {
        value.0
    }
}
impl From<&BaudRateForSerialDevice> for BaudRateForSerialDevice {
    fn from(value: &BaudRateForSerialDevice) -> Self {
        value.clone()
    }
}
impl std::convert::TryFrom<i64> for BaudRateForSerialDevice {
    type Error = &'static str;
    fn try_from(value: i64) -> Result<Self, &'static str> {
        if ![2400_i64, 9600_i64, 115200_i64].contains(&value) {
            Err("invalid value")
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> serde::Deserialize<'de> for BaudRateForSerialDevice {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Self::try_from(i64::deserialize(deserializer)?)
            .map_err(|e| <D::Error as serde::de::Error>::custom(e.to_string()))
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Device {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<AddressOfTheDevice>,
    pub device_model: String,
    pub driver: String,
    #[serde(default = "defaults::default_bool::<true>")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub reading_type: TypeOfReading,
    pub vendor_id: String,
}
impl From<&Device> for Device {
    fn from(value: &Device) -> Self {
        value.clone()
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ReadingSchema {
    pub device: String,
    pub var: String,
}
impl From<&ReadingSchema> for ReadingSchema {
    fn from(value: &ReadingSchema) -> Self {
        value.clone()
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
//#[serde(deny_unknown_fields)]
pub struct ReadingsToTake {
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, ReadingSchema>,
}
impl From<&ReadingsToTake> for ReadingsToTake {
    fn from(value: &ReadingsToTake) -> Self {
        value.clone()
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum TypeOfReading {
    #[serde(rename = "sys")]
    Sys,
    #[serde(rename = "modbusrtu")]
    Modbusrtu,
    #[serde(rename = "modbustcp")]
    Modbustcp,
    #[serde(rename = "mqtt")]
    Mqtt,
    #[serde(rename = "rawserial")]
    Rawserial,
    #[serde(rename = "rawtcp")]
    Rawtcp,
    #[serde(rename = "sma_hycon_csv")]
    SmaHyconCsv,
    #[serde(rename = "sma_speedwire")]
    SmaSpeedwire,
    #[serde(rename = "snmp")]
    Snmp,
}
impl From<&TypeOfReading> for TypeOfReading {
    fn from(value: &TypeOfReading) -> Self {
        value.clone()
    }
}
impl ToString for TypeOfReading {
    fn to_string(&self) -> String {
        match *self {
            Self::Sys => "sys".to_string(),
            Self::Modbusrtu => "modbusrtu".to_string(),
            Self::Modbustcp => "modbustcp".to_string(),
            Self::Mqtt => "mqtt".to_string(),
            Self::Rawserial => "rawserial".to_string(),
            Self::Rawtcp => "rawtcp".to_string(),
            Self::SmaHyconCsv => "sma_hycon_csv".to_string(),
            Self::SmaSpeedwire => "sma_speedwire".to_string(),
            Self::Snmp => "snmp".to_string(),
        }
    }
}
impl std::str::FromStr for TypeOfReading {
    type Err = &'static str;
    fn from_str(value: &str) -> Result<Self, &'static str> {
        match value {
            "sys" => Ok(Self::Sys),
            "modbusrtu" => Ok(Self::Modbusrtu),
            "modbustcp" => Ok(Self::Modbustcp),
            "mqtt" => Ok(Self::Mqtt),
            "rawserial" => Ok(Self::Rawserial),
            "rawtcp" => Ok(Self::Rawtcp),
            "sma_hycon_csv" => Ok(Self::SmaHyconCsv),
            "sma_speedwire" => Ok(Self::SmaSpeedwire),
            "snmp" => Ok(Self::Snmp),
            _ => Err("invalid value"),
        }
    }
}
impl std::convert::TryFrom<&str> for TypeOfReading {
    type Error = &'static str;
    fn try_from(value: &str) -> Result<Self, &'static str> {
        value.parse()
    }
}
impl std::convert::TryFrom<&String> for TypeOfReading {
    type Error = &'static str;
    fn try_from(value: &String) -> Result<Self, &'static str> {
        value.parse()
    }
}
impl std::convert::TryFrom<String> for TypeOfReading {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, &'static str> {
        value.parse()
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum TypecastOfOutput {
    #[serde(rename = "int")]
    Int,
    #[serde(rename = "float")]
    Float,
    #[serde(rename = "str")]
    Str,
    #[serde(rename = "bool")]
    Bool,
}
impl From<&TypecastOfOutput> for TypecastOfOutput {
    fn from(value: &TypecastOfOutput) -> Self {
        value.clone()
    }
}
impl ToString for TypecastOfOutput {
    fn to_string(&self) -> String {
        match *self {
            Self::Int => "int".to_string(),
            Self::Float => "float".to_string(),
            Self::Str => "str".to_string(),
            Self::Bool => "bool".to_string(),
        }
    }
}
impl std::str::FromStr for TypecastOfOutput {
    type Err = &'static str;
    fn from_str(value: &str) -> Result<Self, &'static str> {
        match value {
            "int" => Ok(Self::Int),
            "float" => Ok(Self::Float),
            "str" => Ok(Self::Str),
            "bool" => Ok(Self::Bool),
            _ => Err("invalid value"),
        }
    }
}
impl std::convert::TryFrom<&str> for TypecastOfOutput {
    type Error = &'static str;
    fn try_from(value: &str) -> Result<Self, &'static str> {
        value.parse()
    }
}
impl std::convert::TryFrom<&String> for TypecastOfOutput {
    type Error = &'static str;
    fn try_from(value: &String) -> Result<Self, &'static str> {
        value.parse()
    }
}
impl std::convert::TryFrom<String> for TypecastOfOutput {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, &'static str> {
        value.parse()
    }
}
mod defaults {
    pub(super) fn default_bool<const V: bool>() -> bool {
        V
    }
    pub(super) fn default_u64<T, const V: u64>() -> T
    where
        T: std::convert::TryFrom<u64>,
        <T as std::convert::TryFrom<u64>>::Error: std::fmt::Debug,
    {
        T::try_from(V).unwrap()
    }
}
