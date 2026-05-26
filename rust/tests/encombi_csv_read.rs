use ae::node_mgmt::config::config_from_str;
use ae::readers;

// Points at the shared FTP mock; the encombi fixtures are mounted at /ftp/data/encombi.
// 127.0.0.1 (not "localhost") so the NTP clock-offset probe reaches the IPv4 NTP mock.
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
    let readings = readers::encombi_csv::run_acquisition(&config);
    // Selection picks the second-to-last _Prod.txt (2026-05-05_Prod.txt), which has 5 rows;
    // the latest (2026-05-06, incomplete) and the _Sum.txt / ProdViewLog.txt files are skipped.
    assert_eq!(readings.len(), 5);
}
