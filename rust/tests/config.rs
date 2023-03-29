use ae::node_mgmt::config::Config;

mod stubs;

#[test]
fn test_parse_example_config() {
    assert!(serde_json::from_str::<Config>(stubs::config::PAYLOAD_1).is_ok());
}

#[test]
fn test_parse_bad_config() {
    assert!(serde_json::from_str::<Config>(stubs::config::BAD_PAYLOAD).is_err());
}
