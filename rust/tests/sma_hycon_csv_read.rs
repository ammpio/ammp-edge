use ae::node_mgmt::config::config_from_str;
use ae::readers;

mod stubs;

#[test]
fn test_get_csv_over_ftp() {
    let config = config_from_str(stubs::config::VALID_PAYLOAD_1).unwrap();
    let readings = readers::sma_hycon_csv::run_acquisition(&config);
    assert!(readings.len() > 8640);
}
