use std::{str::from_utf8, thread, time::Duration};

use assert_cmd::{assert::Assert, Command};
use rumqttc::{Client, Event, MqttOptions, Packet, QoS};

const SNAP_REV: u16 = 500;

fn mqtt_pub_assert(snap_rev: u16) -> Assert {
    let mut cmd = Command::cargo_bin("ae").unwrap();
    cmd.env("SNAP_REVISION", snap_rev.to_string())
        .arg("mqtt-pub-meta")
        .assert()
}

#[test]
fn metadata_from_mqtt() {
    let mut mqttoptions = MqttOptions::new("mqtt-pub-e2e-test-subscriber", "localhost", 1883);
    mqttoptions.set_clean_session(true);

    let (mut client, mut connection) = Client::new(mqttoptions, 10);

    client.subscribe("u/meta/#", QoS::ExactlyOnce).unwrap();

    thread::spawn(move || {
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
            let payload = from_utf8(&msg.payload).unwrap();
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
}
