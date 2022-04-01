use anyhow::Result;
use kvstore::{DbRO, DbRW};
use serde_derive::{Deserialize, Serialize};
use std::env;

const SQLITE_REL_PATH: &str = "db/kvstore.db";
const BASE_PATH_ENV_VAR: &str = "SNAP_COMMON";

#[derive(Debug, Deserialize, Serialize)]
pub struct Cat {
    name: String,
    lives: u64,
    siblings: Vec<String>,
}

fn main() -> Result<()> {
    let sqlite_db = format!(
        "{}/{}",
        env::var(BASE_PATH_ENV_VAR).unwrap_or_default(),
        SQLITE_REL_PATH
    );

    let db = DbRW::open(&sqlite_db)?;

    let lulu = Cat {
        name: String::from("Lulu"),
        lives: 9,
        siblings: vec![String::from("Mollie"), String::from("Lilly")],
    };

    db.set("lulu", &lulu)?;
    let newlu: Cat = db.get("lulu").expect("Error reading KV store").unwrap();
    println!("Newlu is {:?}", newlu);

    let db2 = DbRO::open(&sqlite_db)?;
    let newnewlu: Cat = db2.get("lulu").expect("Error reading KV store").unwrap();
    println!("Newnewlu is {:?}", newnewlu);

    // db2.set("lulu", &lulu)?; // wouldn't compile

    Ok(())
}
