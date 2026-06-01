use once_cell::sync::Lazy;

// The ENcombi production log (YYYY-MM-DD_Prod.txt) is tab-delimited with NO header row,
// so columns are mapped positionally. Column 0 is the (time-only) timestamp and is
// handled separately by the parser; the fields below map the remaining columns.
//
// Power columns are reported in kW by the controller and scaled to W (multiplier 1000)
// to match AMMP conventions; percentages, irradiance, temperature and the status flags
// are passed through as-is.
//
// The Mains column is signed: per the discovery doc, a positive value means import from
// the grid and is emitted as `grid_in_P`; a negative value means export to the grid and
// is emitted as `grid_out_P` (absolutised). This is the `negative_name` mechanism.

#[derive(Clone, Debug)]
pub struct DriverField {
    /// AMMP field name emitted when the parsed value is positive (or any value
    /// if `negative_name` is None).
    pub name: &'static str,
    /// If set and the parsed value is negative, this name is emitted instead and
    /// the value is absolutised. Used for sign-split columns like Mains.
    pub negative_name: Option<&'static str>,
    /// Zero-based column index in the tab-delimited row
    pub index: usize,
    pub unit: &'static str,
    pub multiplier: Option<f64>,
}

pub struct Driver {
    pub fields: Vec<DriverField>,
}

pub static ENCOMBI_CSV: Lazy<Driver> = Lazy::new(|| Driver {
    fields: vec![
        DriverField {
            name: "pvinv_P_total",
            negative_name: None,
            index: 1,
            unit: "W",
            multiplier: Some(1000.0),
        },
        DriverField {
            name: "genset_P",
            negative_name: None,
            index: 2,
            unit: "W",
            multiplier: Some(1000.0),
        },
        DriverField {
            // Mains is signed: positive -> grid_in_P (import), negative -> grid_out_P (export, |value|).
            name: "grid_in_P",
            negative_name: Some("grid_out_P"),
            index: 3,
            unit: "W",
            multiplier: Some(1000.0),
        },
        DriverField {
            name: "load_P",
            negative_name: None,
            index: 4,
            unit: "W",
            multiplier: Some(1000.0),
        },
        DriverField {
            name: "pv_capacity",
            negative_name: None,
            index: 5,
            unit: "W",
            multiplier: Some(1000.0),
        },
        DriverField {
            name: "pv_reference",
            negative_name: None,
            index: 6,
            unit: "W",
            multiplier: Some(1000.0),
        },
        DriverField {
            name: "genset_load",
            negative_name: None,
            index: 7,
            unit: "%",
            multiplier: None,
        },
        DriverField {
            name: "irradiance",
            negative_name: None,
            index: 8,
            unit: "W/m2",
            multiplier: None,
        },
        DriverField {
            name: "module_temp",
            negative_name: None,
            index: 9,
            unit: "C",
            multiplier: None,
        },
        DriverField {
            name: "curtail_active",
            negative_name: None,
            index: 10,
            unit: "",
            multiplier: None,
        },
        DriverField {
            name: "start_active",
            negative_name: None,
            index: 11,
            unit: "",
            multiplier: None,
        },
        DriverField {
            name: "genset_breaker_on",
            negative_name: None,
            index: 12,
            unit: "",
            multiplier: None,
        },
        DriverField {
            name: "mains_breaker_on",
            negative_name: None,
            index: 13,
            unit: "",
            multiplier: None,
        },
    ],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encombi_csv_driver() {
        // 13 data fields (column 0 is the timestamp, handled separately)
        assert_eq!(ENCOMBI_CSV.fields.len(), 13);
        let pv = ENCOMBI_CSV
            .fields
            .iter()
            .find(|f| f.name == "pvinv_P_total")
            .unwrap();
        assert_eq!(pv.index, 1);
        assert_eq!(pv.multiplier, Some(1000.0));
        let bom = ENCOMBI_CSV
            .fields
            .iter()
            .find(|f| f.name == "module_temp")
            .unwrap();
        assert_eq!(bom.index, 9);
        assert_eq!(bom.multiplier, None);
        // Mains is the only sign-split column
        let mains = ENCOMBI_CSV
            .fields
            .iter()
            .find(|f| f.name == "grid_in_P")
            .unwrap();
        assert_eq!(mains.index, 3);
        assert_eq!(mains.negative_name, Some("grid_out_P"));
    }
}
