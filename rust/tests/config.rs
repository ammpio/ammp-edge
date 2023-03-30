use ae::node_mgmt::config;

mod stubs;

#[test]
fn test_parse_example_config() {
    assert!(config::from_str(stubs::config::VALID_PAYLOAD_1).is_ok());
}

#[test]
fn test_parse_bad_configs() {
    assert!(config::from_str(stubs::config::INVALID_JSON).is_err());
    assert!(config::from_str(stubs::config::INVALID_PAYLOAD_1).is_err());
}
