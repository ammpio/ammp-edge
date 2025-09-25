//! Derived Rust types from JSON schemas
//!
//! This crate contains types automatically generated from JSON schema files:
//! - `config`: Types from config.schema.json
//! - `driver`: Types from driver.schema.json
//! - `data`: Types from data.schema.json
//!
//! These types are generated at build time and should not be edited manually.

pub mod config;
pub mod data;
pub mod driver;
