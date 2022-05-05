use std::env;

use anyhow::Result;
use rusqlite::{Connection, OpenFlags, OptionalExtension};

const LEGACY_SQLITE_REL_PATH: &str = "config.db";
const BASE_PATH_ENV_VAR: &str = "SNAP_COMMON";

#[derive(Debug)]
pub struct LegacyConfig {
    pub node_id: String,
    pub access_key: String,
    pub config: String,
}

pub fn get_legacy_config() -> Result<Option<LegacyConfig>> {
    let conn = Connection::open_with_flags(&sqlite_db_path(), OpenFlags::SQLITE_OPEN_READ_ONLY)?;

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

fn sqlite_db_path() -> String {
    format!(
        "{}/{}",
        env::var(BASE_PATH_ENV_VAR).unwrap_or_else(|_| String::from(".")),
        LEGACY_SQLITE_REL_PATH
    )
}
