use std::collections::HashMap;

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

// TODO: Refactor this into more generic driver parser

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum Typecast {
    #[serde(rename = "str")]
    Str,
    #[serde(rename = "float")]
    Float,
    #[serde(rename = "int")]
    Int,
    #[serde(rename = "bool")]
    Bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DriverField {
    pub column: String,
    pub description: String,
    pub unit: String,
    pub typecast: Typecast,
    pub multiplier: Option<f64>,
    pub offset: Option<f64>,
}

pub type Driver = HashMap<String, DriverField>;

pub static SMA_HYCON_CSV: Lazy<Driver> = Lazy::new(|| {
    serde_json::from_str::<Driver>(r#"
    {
        "grid_out_P": {"column": "LoadPwrAtTot", "description": "Load power", "unit": "W", "multiplier": 1000, "typecast": "float"},
        "genset_P": {"column": "GenPwrAtTot", "description": "Genset power", "unit": "W", "multiplier": 1000, "typecast": "float"},
        "pvinv_P_total": {"column": "PvPwrAtTot", "description": "PV power", "unit": "W", "multiplier": 1000, "typecast": "float"},
        "grid_in_P": {"column": "GridPwrAtTot", "description": "Grid power", "unit": "W", "multiplier": 1000, "typecast": "float"}
    }
    "#).unwrap()
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sma_hycon_csv_driver() {
        assert_eq!(SMA_HYCON_CSV.len(), 4);
        assert_eq!(SMA_HYCON_CSV["grid_out_P"].multiplier, Some(1000.0));
        assert_eq!(SMA_HYCON_CSV["grid_in_P"].column, "GridPwrAtTot");
        assert_eq!(SMA_HYCON_CSV["grid_in_P"].column, "GridPwrAtTot");
    }
}
