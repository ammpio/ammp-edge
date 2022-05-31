use std::path::Path;

use anyhow::Result;
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde_json::Value;

#[derive(Debug)]
pub struct LegacyConfig {
    pub node_id: String,
    pub access_key: String,
    pub config: Value,
}

pub fn get_legacy_config(path: impl AsRef<Path>) -> Result<Option<LegacyConfig>> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;

    conn.query_row(
        "SELECT node_id, access_key, config FROM 'nodeconfig' LIMIT 1",
        [],
        |r| {
            Ok(LegacyConfig {
                node_id: r.get(0)?,
                access_key: r.get(1)?,
                config: r.get(2)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}
