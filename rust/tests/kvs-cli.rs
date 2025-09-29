use std::ffi::OsStr;

use assert_cmd::{Command, assert::Assert};
use once_cell::sync::Lazy;
use predicates::prelude::*;
use serde_json::{Value, json};

const TEST_KEY: &str = "somekey";
const TEST_VALUE_TEXT: &str = "somevalue";
static TEST_VALUE_JSON: Lazy<Value> = Lazy::new(|| {
    json!({
        "a": {"b": "c"},
        "d": [1, 2, 3]
    })
});

fn kvs_set_assert(
    data_dir: impl AsRef<OsStr>,
    key: impl AsRef<OsStr>,
    value: impl AsRef<OsStr>,
) -> Assert {
    let mut cmd = Command::cargo_bin("ae").unwrap();
    cmd.env("AE_DATA_DIR", data_dir)
        .arg("kvs-set")
        .arg(key)
        .arg(value)
        .assert()
}

fn kvs_get_assert(data_dir: impl AsRef<OsStr>, key: impl AsRef<OsStr>) -> Assert {
    let mut cmd = Command::cargo_bin("ae").unwrap();
    cmd.env("AE_DATA_DIR", data_dir)
        .arg("kvs-get")
        .arg(key)
        .assert()
}

#[test]
fn set_and_get_string_value() {
    let tempdir = tempfile::tempdir().unwrap();

    let assert = kvs_set_assert(tempdir.path(), TEST_KEY, TEST_VALUE_TEXT);
    assert.success();

    let assert = kvs_get_assert(tempdir.path(), TEST_KEY);
    assert.success().stdout(TEST_VALUE_TEXT);
}

#[test]
fn set_and_get_json_value() {
    let tempdir = tempfile::tempdir().unwrap();

    let assert = kvs_set_assert(tempdir.path(), TEST_KEY, TEST_VALUE_JSON.to_string());
    assert.success();

    let assert = kvs_get_assert(tempdir.path(), TEST_KEY);
    assert.success().stdout(TEST_VALUE_JSON.to_string());
}

#[test]
fn get_unset_value_fails() {
    let tempdir = tempfile::tempdir().unwrap();

    let assert = kvs_get_assert(tempdir.path(), TEST_KEY);
    assert
        .failure()
        .stdout("")
        .stderr(predicate::str::contains(format!(
            "Error: No value set for key '{TEST_KEY}'"
        )));
}
