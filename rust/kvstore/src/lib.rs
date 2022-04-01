use anyhow::Result;
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::path::Path;

const TABLENAME: &str = "kvstore";
const KEY_FIELD: &str = "key";
const VALUE_FIELD: &str = "value";

pub struct AccessRO;
pub struct AccessRW;

pub struct Db<AccessTag>(rusqlite::Connection, AccessTag);

pub type DbRW = Db<AccessRW>;
pub type DbRO = Db<AccessRO>;

// Methods common to read-only and read-write connections
impl<AccessTag> Db<AccessTag> {
    fn select<K: AsRef<str>>(&self, key: K) -> Result<Option<Vec<u8>>> {
        self.0
            .query_row(
                &format!("SELECT {VALUE_FIELD} FROM '{TABLENAME}' WHERE {KEY_FIELD} = ?1"),
                [key.as_ref()],
                |r| r.get::<_, Vec<u8>>(0),
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn get<T: DeserializeOwned>(&self, key: impl AsRef<str>) -> Result<Option<T>> {
        self.select(key)?
            .map(|v| serde_json::from_slice::<T>(&v))
            .transpose()
            .map_err(Into::into)
    }
}

// Methods specific to read-only connection
impl DbRO {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        println!("Connecting in read-only mode");
        let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        Ok(Db(connection, AccessRO))
    }
}

// Methods specific to read-write connection
impl DbRW {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        println!("Connecting in read-write mode");
        // Create directory for DB if it doesn't already exist
        std::fs::create_dir_all(path.as_ref().parent().unwrap_or(Path::new("")))?;
        let connection = Connection::open(path)?;
        connection.execute(
            &format!(
                "CREATE TABLE IF NOT EXISTS '{TABLENAME}' (
                key TEXT PRIMARY KEY NOT NULL,
                value BLOB NOT NULL
                )"
            ),
            [],
        )?;

        Ok(Db(connection, AccessRW))
    }

    fn upsert<K: AsRef<str>, V: AsRef<[u8]>>(&self, key: K, value: V) -> Result<()> {
        let mut stmt = self.0.prepare(&format!(
            "INSERT INTO '{TABLENAME}' ({KEY_FIELD}, {VALUE_FIELD}) values (?1, ?2)
            ON CONFLICT({KEY_FIELD}) DO UPDATE SET {VALUE_FIELD}=?2",
        ))?;
        let res = stmt.execute(params![key.as_ref(), value.as_ref()])?;
        println!("Inserted: {:?}", res);
        Ok(())
    }

    pub fn set<K: AsRef<str>, V: Serialize>(&self, key: K, value: V) -> Result<()> {
        self.upsert(key, serde_json::to_vec(&value)?)?;
        Ok(())
    }
}


#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
