use chrono::NaiveDate;

use ae::node_mgmt::config::config_from_str;
use ae::readers;

// Points at the shared FTP mock; the encombi fixture is mounted at /ftp/data/encombi.
const ENCOMBI_CONFIG: &str = r#"
{
    "devices": {
        "encombi_csv": {
            "key": "encombi_csv",
            "name": "ENcombi Controller - CSV backfill",
            "driver": "encombi_csv",
            "address": {
                "base_url": "ftp://testuser:TestPWD123!@127.0.0.1:21/encombi/",
                "timezone": "Africa/Lagos"
            },
            "enabled": true,
            "vendor_id": "encombi-1",
            "device_model": "gen_control_encombi",
            "reading_type": "encombi_csv"
        }
    },
    "readings": {},
    "timestamp": "1970-01-01T00:00:00Z"
}
"#;

#[test]
fn reads_and_parses_encombi_csv_over_ftp() {
    let config = config_from_str(ENCOMBI_CONFIG).unwrap();
    let date = NaiveDate::from_ymd_opt(2026, 5, 6).unwrap();
    let readings = readers::encombi_csv::run_acquisition(&config, Some(date));
    assert_eq!(readings.len(), 39);
}
