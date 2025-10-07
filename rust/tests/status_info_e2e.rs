use std::str;
use std::thread;
use std::time::Duration;

use ae::constants::topics;
use assert_cmd::Command;
use kvstore::KVDb;
use rumqttc::{Client, Event, MqttOptions, Packet, QoS};
use serde_json::Value;

mod stubs;

/// Test end-to-end status info reading flow
///
/// This test:
/// 1. Sets up a config with status_info definitions
/// 2. Runs start-readings --once to execute one reading cycle
/// 3. Subscribes to MQTT and validates the payload contains expected status readings
#[test]
fn status_info_end_to_end() {
    // Create temporary directory for test data
    let tempdir = tempfile::tempdir().unwrap();
    let data_dir = tempdir.path();

    // Load test config
    let config: Value = serde_json::from_str(stubs::config::STATUS_INFO_TEST_CONFIG).unwrap();

    // Set up KV store with config
    let kvs = KVDb::new(data_dir.join("kvs-db/kvstore.db")).unwrap();
    kvs.set("config", &config).unwrap();

    // Set up MQTT subscriber
    let mut mqttoptions = MqttOptions::new("status-info-e2e-test", "localhost", 1883);
    mqttoptions.set_clean_session(true);
    mqttoptions.set_keep_alive(Duration::from_secs(5));

    let (client, mut connection) = Client::new(mqttoptions, 10);

    // Subscribe to data topic
    client.subscribe(topics::DATA, QoS::AtLeastOnce).unwrap();

    // Spawn reading command in background thread
    let data_dir_clone = data_dir.to_path_buf();
    let command_thread = thread::spawn(move || {
        // Give subscriber time to connect
        thread::sleep(Duration::from_millis(500));

        println!("Starting readings command...");
        let mut cmd = Command::cargo_bin("ae").unwrap();
        cmd.env("AE_DATA_DIR", data_dir_clone.as_os_str())
            .arg("start-readings")
            .arg("--once")
            .timeout(Duration::from_secs(10))
            .assert()
            .success();
        println!("Readings command completed");
    });

    // Wait for MQTT messages
    let mut received_payload = false;
    let timeout_at = std::time::Instant::now() + Duration::from_secs(10);

    println!("Waiting for MQTT messages...");
    for notification in connection.iter() {
        if std::time::Instant::now() > timeout_at {
            panic!("Timeout waiting for MQTT message");
        }

        match notification {
            Ok(Event::Incoming(Packet::Publish(msg))) => {
                println!("Received message on topic: {}", msg.topic);

                if msg.topic == topics::DATA {
                    let payload_str = str::from_utf8(&msg.payload).unwrap();
                    println!("Payload: {}", payload_str);

                    let payload: Value = serde_json::from_str(payload_str).unwrap();

                    // Validate payload structure
                    assert!(payload["t"].is_number(), "Payload should have timestamp");
                    assert!(
                        payload["r"].is_array(),
                        "Payload should have readings array"
                    );

                    let readings = payload["r"].as_array().unwrap();

                    // Find the EMS device reading
                    let ems_reading = readings
                        .iter()
                        .find(|r| r["_d"] == "ems" || r["_vid"] == "ems-test-1")
                        .expect("Should find EMS device reading");

                    println!(
                        "EMS reading: {}",
                        serde_json::to_string_pretty(ems_reading).unwrap()
                    );

                    // Validate status readings are present
                    assert!(
                        ems_reading["_s"].is_array(),
                        "EMS reading should have status readings array"
                    );

                    let status_readings = ems_reading["_s"].as_array().unwrap();
                    assert!(
                        !status_readings.is_empty(),
                        "Status readings array should not be empty"
                    );

                    println!("Found {} status readings", status_readings.len());

                    // Validate each status reading has content and level
                    for status_reading in status_readings {
                        assert!(
                            status_reading["c"].is_string(),
                            "Status reading should have content (c) field"
                        );
                        assert!(
                            status_reading["l"].is_number(),
                            "Status reading should have level (l) field"
                        );

                        let content = status_reading["c"].as_str().unwrap();
                        let level = status_reading["l"].as_u64().unwrap();

                        println!("  Status: {} (level: {})", content, level);

                        // Validate against expected values based on register bits
                        match content {
                            "Relay Fault Detected" => {
                                // Register 200, bit 2 is set -> level 3
                                assert_eq!(level, 3, "Relay fault should have level 3");
                            }
                            "High Temperature Warning" => {
                                // Register 202, bit 9 is set -> level 2 (custom map)
                                assert_eq!(level, 2, "High temp warning should have level 2");
                            }
                            "System Alarm" => {
                                // Register 201, bits 8-11 (MSB) = 0b0010 = 4
                                assert_eq!(level, 4, "System alarm should have level 4");
                            }
                            _ => panic!("Unexpected status reading content: {}", content),
                        }
                    }

                    // Validate we have all three expected status readings
                    assert_eq!(
                        status_readings.len(),
                        3,
                        "Should have exactly 3 status readings"
                    );

                    received_payload = true;
                    break;
                }
            }
            Ok(_) => {
                // Ignore other events
            }
            Err(e) => {
                eprintln!("MQTT connection error: {}", e);
                break;
            }
        }
    }

    // Clean up
    client.disconnect().unwrap();
    command_thread.join().unwrap();

    assert!(
        received_payload,
        "Should have received at least one payload with status readings"
    );
}
