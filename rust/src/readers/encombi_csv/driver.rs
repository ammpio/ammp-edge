use once_cell::sync::Lazy;

// The ENcombi production log (YYYY-MM-DD_Prod.txt) is tab-delimited with NO header row,
// so columns are mapped positionally. Column 0 is the (time-only) timestamp and is
// handled separately by the parser; the fields below map the remaining columns.
//
// Power columns are reported in kW by the controller and scaled to W (multiplier 1000)
// to match AMMP conventions; percentages, irradiance, temperature and the status flags
// are passed through as-is.

#[derive(Clone, Debug)]
pub struct DriverField {
    pub name: &'static str,
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
            index: 1,
            unit: "W",
            multiplier: Some(1000.0),
        },
        DriverField {
            name: "genset_P",
            index: 2,
            unit: "W",
            multiplier: Some(1000.0),
        },
        DriverField {
            name: "grid_in_P",
            index: 3,
            unit: "W",
            multiplier: Some(1000.0),
        },
        DriverField {
            name: "grid_out_P",
            index: 4,
            unit: "W",
            multiplier: Some(1000.0),
        },
        DriverField {
            name: "pv_capacity",
            index: 5,
            unit: "W",
            multiplier: Some(1000.0),
        },
        DriverField {
            name: "pv_reference",
            index: 6,
            unit: "W",
            multiplier: Some(1000.0),
        },
        DriverField {
            name: "genset_load",
            index: 7,
            unit: "%",
            multiplier: None,
        },
        DriverField {
            name: "irradiance",
            index: 8,
            unit: "W/m2",
            multiplier: None,
        },
        DriverField {
            name: "module_temp",
            index: 9,
            unit: "C",
            multiplier: None,
        },
        DriverField {
            name: "curtail_active",
            index: 10,
            unit: "",
            multiplier: None,
        },
        DriverField {
            name: "start_active",
            index: 11,
            unit: "",
            multiplier: None,
        },
        DriverField {
            name: "genset_breaker_on",
            index: 12,
            unit: "",
            multiplier: None,
        },
        DriverField {
            name: "mains_breaker_on",
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
        assert_eq!(ENCOMBI_CSV.fields.len(), 13);
        let pv = ENCOMBI_CSV
            .fields
            .iter()
            .find(|f| f.name == "pvinv_P_total")
            .unwrap();
        assert_eq!(pv.index, 1);
        assert_eq!(pv.multiplier, Some(1000.0));
        let grid_in = ENCOMBI_CSV
            .fields
            .iter()
            .find(|f| f.name == "grid_in_P")
            .unwrap();
        assert_eq!(grid_in.index, 3);
        let grid_out = ENCOMBI_CSV
            .fields
            .iter()
            .find(|f| f.name == "grid_out_P")
            .unwrap();
        assert_eq!(grid_out.index, 4);
        let bom = ENCOMBI_CSV
            .fields
            .iter()
            .find(|f| f.name == "module_temp")
            .unwrap();
        assert_eq!(bom.index, 9);
        assert_eq!(bom.multiplier, None);
    }
}
