#![feature(explicit_generic_args_with_impl_trait)]

use std::ffi::OsStr;

use assert_cmd::{assert::Assert, Command};
use mockito::{mock, Matcher};
use regex::Regex;
use serde::{Deserialize, Serialize};

use kvstore::KVDb;

#[derive(Debug, Deserialize, Serialize)]
struct R1 {
    access_key: String,
    message: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct R2 {
    message: String,
}

fn cmd_init_assert(data_dir: impl AsRef<OsStr>) -> Assert {
    let mut cmd = Command::cargo_bin("ae").unwrap();
    cmd.env("AE_DATA_DIR", data_dir).arg("init").assert()
}

#[test]
fn do_init_via_cli() {
    let tempdir = tempfile::tempdir().unwrap();

    let api_base_url = mockito::server_url();

    let kvs = KVDb::new(tempdir.path().join("kvs-db/kvstore.db")).unwrap();
    kvs.set("http_api_base_url", api_base_url).ok();

    let activation_path = Matcher::Regex(r"^/nodes/([0-9,a-f]*)/activate$".to_string());
    const SAMPLE_ACCESS_KEY: &str = "secret";
    let sample_resp_1 = R1 {
        access_key: SAMPLE_ACCESS_KEY.to_string(),
        message: "Activation request approved. Please use provided key to verify access and confirm activation.".to_string(),
    };
    let sample_resp_2 = R2 {
        message: "Node abcdef123456 successfully activated".to_string(),
    };

    let _m1 = mock("GET", activation_path.clone())
        .with_body(serde_json::to_vec(&sample_resp_1).unwrap())
        .expect(1)
        .create();
    let _m2 = mock("POST", activation_path)
        .match_header("Authorization", SAMPLE_ACCESS_KEY)
        .with_body(serde_json::to_vec(&sample_resp_2).unwrap())
        .expect(1)
        .create();

    cmd_init_assert(tempdir.path()).success();

    let node_id = kvs.get::<String>("node_id").unwrap().unwrap();
    log::info!("Initialized with node ID: {}", node_id);

    assert!(Regex::new(r"^[0-9,a-f]{12}$").unwrap().is_match(&node_id));
    assert_eq!(
        kvs.get::<String>("access_key").unwrap().unwrap(),
        SAMPLE_ACCESS_KEY
    );
}
