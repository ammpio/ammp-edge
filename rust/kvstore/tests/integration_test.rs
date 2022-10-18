use serde::{Deserialize, Serialize};
use tempfile::tempdir;

use kvstore::{KVDb, KVStoreError};

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Cat {
    name: String,
    lives: u64,
    siblings: Vec<String>,
}

#[test]
fn write_and_read_object() -> Result<(), KVStoreError> {
    env_logger::init();

    let sqlite_db = tempdir()?.path().join("kvstore.db");

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

    Ok(())
}
