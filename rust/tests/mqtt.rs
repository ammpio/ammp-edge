use std::ffi::OsStr;
use std::os::unix::process::ExitStatusExt;
use std::str;
use std::thread;
use std::time::Duration;

use assert_cmd::{Command, assert::Assert};
use kvstore::KVDb;
use rumqttc::{Client, Event, MqttOptions, Packet, QoS};
use serde_json::Value;

mod stubs;

const SNAP_REV: u16 = 500;

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

    let (client, mut connection) = Client::new(mqttoptions, 10);

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

        let (client, mut connection) = Client::new(mqttoptions, 10);

        client
            .publish(
                "d/config",
                QoS::AtLeastOnce,
                false,
                stubs::config::VALID_PAYLOAD_1.as_bytes(),
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
    let target_config = serde_json::from_str::<Value>(stubs::config::VALID_PAYLOAD_1).unwrap();
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

        let (client, mut connection) = Client::new(mqttoptions, 10);

        client
            .publish(
                "d/config",
                QoS::AtLeastOnce,
                false,
                stubs::config::INVALID_PAYLOAD_1.as_bytes(),
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
