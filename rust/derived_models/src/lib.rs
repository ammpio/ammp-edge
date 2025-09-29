//! Derived Rust types from JSON schemas
//!
//! This crate contains types automatically generated from JSON schema files:
//! - `config`: Types from config.schema.json
//! - `driver`: Types from driver.schema.json
//! - `data`: Types from data.schema.json
//!
//! These types are generated at build time and should not be edited manually.
//!
//! ## Import Policy
//!
//! **Do not import types directly from this crate.** Instead, use the re-exports
//! provided by the appropriate domain modules:
//! - `node_mgmt::config` for config types (Device, ReadingType, etc.)
//! - `node_mgmt::drivers` for driver schema types
//! - `data_mgmt::payload` for data payload types (Metadata, DeviceData, etc.)
//!
//! This ensures proper domain boundaries and makes refactoring easier.

#![allow(clippy::derivable_impls, clippy::clone_on_copy)]

pub mod config;
pub mod data;
pub mod driver;
