use std::time::Duration;

pub const API_REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
pub const DB_WRITE_TIMEOUT: Duration = Duration::from_secs(120);
pub const LOG_LEVEL: &str = "info";

pub const MQTT_BRIDGE_HOST: &str = "localhost";
pub const MQTT_BRIDGE_PORT: u16 = 1883;
