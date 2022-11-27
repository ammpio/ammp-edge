use std::ffi::OsStr;
use std::os::unix::process::ExitStatusExt;
use std::str;
use std::thread;
use std::time::Duration;

use assert_cmd::{assert::Assert, Command};
use kvstore::KVDb;
use rumqttc::{Client, Event, MqttOptions, Packet, QoS};
use serde_json::Value;

const SNAP_REV: u16 = 500;
const CONFIG_PAYLOAD: &str = r#"
{
    "name": "Basic config",
    "devices": {
      "logger": {
        "name": "Logger",
        "driver": "sys_generic",
        "enabled": true,
        "vendor_id": "strato-1",
        "reading_type": "sys"
      },
      "sma_stp_1": {
        "name": "SMA STP-25000 (good)",
        "driver": "sma_stp25000",
        "enabled": true,
        "vendor_id": "1234567890",
        "reading_type": "modbustcp",
        "address": {
          "host": "mock-sma-stp",
          "unit_id": 3
        }
      },
      "sma_stp_2": {
        "name": "SMA STP-25000 (bad)",
        "driver": "sma_stp25000",
        "enabled": true,
        "vendor_id": "000",
        "reading_type": "modbustcp",
        "address": {
          "host": "mock-sma-stp",
          "unit_id": 100
        }
      }
    },
    "readings": {
      "comms_lggr_boot_time": {"device": "logger", "var": "boot_time"},
      "comms_lggr_cpu_load": {"device": "logger", "var": "cpu_load"},
      "comms_lggr_disk_usage": {"device": "logger", "var": "disk_usage"},
      "comms_lggr_mem_usage": {"device": "logger", "var": "memory_usage"},
      "pv_P_1": {"device": "sma_stp_1", "var": "P_total"},
      "pv_E_1": {"device": "sma_stp_1", "var": "total_yield"},
      "pv_P_2": {"device": "sma_stp_2", "var": "P_total"}
    },
    "timestamp": "2022-08-15T13:03:17Z",
    "read_interval": 15,
    "read_roundtime": true
  }
"#;

const BAD_CONFIG_PAYLOAD: &str = "blah";

fn mqtt_pub_assert(snap_rev: u16) -> Assert {
    let mut cmd = Command::cargo_bin("ae").unwrap();
    cmd.env("SNAP_REVISION", snap_rev.to_string())
        .arg("mqtt-pub-meta")
        .assert()
}

fn mqtt_sub_assert(data_dir: impl AsRef<OsStr>, timeout: Duration) -> Assert {
    let mut cmd = Command::cargo_bin("ae").unwrap();
    cmd.env("AE_DATA_DIR", data_dir)
        .arg("mqtt-sub-cfg-cmd")
        .timeout(timeout)
        .assert()
}

#[test]
fn mqtt_publish_meta() {
    let mut mqttoptions = MqttOptions::new("mqtt-pub-e2e-test-subscriber", "localhost", 1883);
    mqttoptions.set_clean_session(true);

    let (mut client, mut connection) = Client::new(mqttoptions, 10);

    client.subscribe("u/meta/#", QoS::ExactlyOnce).unwrap();

    let command_thread = thread::spawn(move || {
        thread::sleep(Duration::from_millis(200));
        let assert = mqtt_pub_assert(SNAP_REV);
        assert.success();
    });

    let num_topics: u8 = 4;
    let mut msg_count: u8 = 0;

    // Start with blank messages on each topic, used to reset previously retained messages
    for (_, notification) in connection.iter().enumerate() {
        println!("Notification = {:?}", notification);
        let event = notification.unwrap();
        if let Event::Incoming(Packet::Publish(msg)) = event {
            assert!(msg.payload.is_empty());
            msg_count += 1;
            if msg_count == num_topics {
                break;
            }
        }
    }

    msg_count = 0;
    // Then we get the acutal payloads
    for (_, notification) in connection.iter().enumerate() {
        println!("Notification = {:?}", notification);
        let event = notification.unwrap();
        if let Event::Incoming(Packet::Publish(msg)) = event {
            let payload = str::from_utf8(&msg.payload).unwrap();
            match msg.topic.as_str() {
                "u/meta/snap_rev" => assert_eq!(payload, SNAP_REV.to_string()),
                "u/meta/boot_time" => assert!(payload.parse::<u32>().unwrap() > 0),
                "u/meta/start_time" => assert!(payload.parse::<u32>().unwrap() > 0),
                "u/meta/arch" => assert!(!payload.is_empty()),
                _ => panic!("Received message on unexpected topic {}", msg.topic),
            }
            msg_count += 1;
            if msg_count == num_topics {
                break;
            }
        }
    }
    client.disconnect().unwrap();
    command_thread.join().unwrap();
}

#[test]
fn mqtt_receive_config() {
    let publisher_thread = thread::spawn(move || {
        thread::sleep(Duration::from_millis(200));
        let mut mqttoptions = MqttOptions::new("mqtt-sub-e2e-test-publisher", "localhost", 1883);
        mqttoptions.set_clean_session(true);

        let (mut client, mut connection) = Client::new(mqttoptions, 10);

        client
            .publish(
                "d/config",
                QoS::AtLeastOnce,
                false,
                CONFIG_PAYLOAD.as_bytes(),
            )
            .unwrap();

        for (_, notification) in connection.iter().enumerate() {
            println!("Notification = {:?}", notification);
            let event = notification.unwrap();
            if let Event::Incoming(Packet::PubAck(_)) = event {
                break;
            }
        }

        client.disconnect().unwrap();
    });

    let tempdir = tempfile::tempdir().unwrap();
    let cmd = mqtt_sub_assert(tempdir.path(), Duration::from_millis(400));
    // We don't assert .success() since the command should time out by design
    // We expect it to be killed with signal 9 (any other exit status code indicates an actual error)
    assert_eq!(cmd.get_output().status.signal(), Some(9));
    assert_eq!(cmd.get_output().status.code(), None);

    let kvs = KVDb::new(tempdir.path().join("kvs-db/kvstore.db")).unwrap();
    let target_config = serde_json::from_str::<Value>(CONFIG_PAYLOAD).unwrap();
    let applied_config: Value = kvs.get("config").unwrap().unwrap();
    assert_eq!(applied_config, target_config);

    publisher_thread.join().unwrap();
}

#[test]
fn mqtt_receive_bad_config() {
    let publisher_thread = thread::spawn(move || {
        thread::sleep(Duration::from_millis(200));
        let mut mqttoptions = MqttOptions::new("mqtt-sub-e2e-test-publisher", "localhost", 1883);
        mqttoptions.set_clean_session(true);

        let (mut client, mut connection) = Client::new(mqttoptions, 10);

        client
            .publish(
                "d/config",
                QoS::AtLeastOnce,
                false,
                BAD_CONFIG_PAYLOAD.as_bytes(),
            )
            .unwrap();

        for (_, notification) in connection.iter().enumerate() {
            println!("Notification = {:?}", notification);
            let event = notification.unwrap();
            if let Event::Incoming(Packet::PubAck(_)) = event {
                break;
            }
        }

        client.disconnect().unwrap();
    });

    let tempdir = tempfile::tempdir().unwrap();
    let cmd = mqtt_sub_assert(tempdir.path(), Duration::from_millis(400));
    // We don't assert .success() since the command should time out by design
    // We expect it to be killed with signal 9 (any other exit status code indicates an actual error)
    assert_eq!(cmd.get_output().status.signal(), Some(9));
    assert_eq!(cmd.get_output().status.code(), None);

    let kvs = KVDb::new(tempdir.path().join("kvs-db/kvstore.db")).unwrap();
    assert_eq!(kvs.get::<Value>("config").unwrap(), None);

    publisher_thread.join().unwrap();
}
