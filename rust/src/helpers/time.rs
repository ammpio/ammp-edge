use chrono::{DateTime, Utc};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
pub fn now_iso() -> String {
    let now: DateTime<Utc> = SystemTime::now().into();
    now.to_rfc3339()
}
