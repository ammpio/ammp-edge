use anyhow::Result;
use getrandom::getrandom;
use kvstore::DbRW;
use std::thread::sleep;
use std::time::Duration;

use crate::helpers::{base_path, now_iso};

pub fn test_sqlite() -> Result<()> {
    let db = DbRW::open(&format!("{}/test.db", base_path::tmp_dir()))?;
    loop {
        let now = now_iso();
        println!("Setting time to {}", &now);
        db.set("time", now)?;
        let mut rand_delay = [0u8; 1];
        getrandom(&mut rand_delay).unwrap();
        let time_to_sleep = rand_delay[0];
        println!("Sleeping {}ms", &time_to_sleep);
        sleep(Duration::from_millis(time_to_sleep.into()));
    }
}
