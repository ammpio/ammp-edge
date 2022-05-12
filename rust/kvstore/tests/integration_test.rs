use anyhow::Result;
use kvstore::KVDb;
use serde::{Deserialize, Serialize};
use std::fs;

const SQLITE_DIR: &str = "/tmp/testdb";
const SQLITE_FILE: &str = "kvstore.db";

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct Cat {
    name: String,
    lives: u64,
    siblings: Vec<String>,
}

#[test]
fn write_and_read_object() -> Result<()> {
    env_logger::init();
    let sqlite_db = format!("{}/{}", SQLITE_DIR, SQLITE_FILE);

    let db = KVDb::new(&sqlite_db)?;

    let lulu = Cat {
        name: String::from("Lulu"),
        lives: 9,
        siblings: vec![String::from("Mollie"), String::from("Lilly")],
    };

    db.set("lulu", &lulu)?;
    let lulu2: Cat = db.get("lulu").expect("Error reading KV store").unwrap();
    assert_eq!(lulu2, lulu);

    let db2 = KVDb::new(&sqlite_db)?;
    let lulu3: Cat = db2.get("lulu").expect("Error reading KV store").unwrap();
    assert_eq!(lulu3, lulu);

    log::info!("Deleting DB directory {SQLITE_DIR}");
    fs::remove_dir_all(SQLITE_DIR)?;
    Ok(())
}
