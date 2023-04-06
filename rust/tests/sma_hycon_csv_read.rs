use std::str::FromStr;

use ae::node_mgmt::Config;
use ae::readers;

mod stubs;

#[test]
fn test_get_csv_over_ftp() {
    let config = Config::from_str(stubs::config::VALID_PAYLOAD_1).unwrap();
    let readings = readers::sma_hycon_csv::run_acquisition(&config);
    assert!(readings.len() > 8640);
}
