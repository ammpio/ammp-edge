use chrono::{DateTime, Utc};
use std::time::SystemTime;

pub fn now_iso() -> String {
    let now: DateTime<Utc> = SystemTime::now().into();
    now.to_rfc3339()
}