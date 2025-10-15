//! Async wrapper around KVDb that uses tokio::task::spawn_blocking
//!
//! This module provides an async-safe interface to the synchronous KVDb,
//! ensuring that blocking SQLite operations don't block the tokio runtime.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::de::DeserializeOwned;
use serde::Serialize;
use tokio::sync::Mutex;

use crate::{KVDb, KVStoreError};

/// Async wrapper around KVDb
///
/// Uses tokio::task::spawn_blocking to ensure SQLite operations don't block
/// the async runtime. The underlying KVDb is protected by a Mutex to ensure
/// thread-safe access.
#[derive(Clone)]
pub struct AsyncKVDb {
    /// Path to the database file (stored for error messages and debugging)
    path: PathBuf,
    /// The underlying KVDb wrapped in Arc<Mutex<>> for safe sharing
    inner: Arc<Mutex<KVDb>>,
}

impl AsyncKVDb {
    /// Create a new AsyncKVDb by opening the database at the given path
    ///
    /// This performs the initial database open operation in a blocking context.
    pub async fn new(path: impl AsRef<Path>) -> Result<Self, KVStoreError> {
        let path = path.as_ref().to_path_buf();
        let path_clone = path.clone();

        let kvdb = tokio::task::spawn_blocking(move || KVDb::new(path_clone))
            .await
            .map_err(|e| {
                KVStoreError::IOError(std::io::Error::other(format!(
                    "Failed to spawn blocking task: {}",
                    e
                )))
            })??;

        Ok(Self {
            path,
            inner: Arc::new(Mutex::new(kvdb)),
        })
    }

    /// Get a value from the key-value store
    ///
    /// Deserializes the stored JSON value into type T.
    pub async fn get<T: DeserializeOwned + Send + 'static>(
        &self,
        key: impl AsRef<str> + Send + 'static,
    ) -> Result<Option<T>, KVStoreError> {
        let inner = self.inner.clone();
        let key = key.as_ref().to_string();

        tokio::task::spawn_blocking(move || {
            let kvdb = inner.blocking_lock();
            kvdb.get(&key)
        })
        .await
        .map_err(|e| {
            KVStoreError::IOError(std::io::Error::other(format!(
                "Failed to spawn blocking task: {}",
                e
            )))
        })?
    }

    /// Set a value in the key-value store
    ///
    /// Serializes the value to JSON before storing.
    pub async fn set<K, V>(&self, key: K, value: &V) -> Result<(), KVStoreError>
    where
        K: AsRef<str> + Send + 'static,
        V: Serialize + Send + Sync,
    {
        let inner = self.inner.clone();
        let key = key.as_ref().to_string();
        let value_json = serde_json::to_string(value)?;

        tokio::task::spawn_blocking(move || {
            let kvdb = inner.blocking_lock();
            kvdb.set_raw(&key, &value_json)
        })
        .await
        .map_err(|e| {
            KVStoreError::IOError(std::io::Error::other(format!(
                "Failed to spawn blocking task: {}",
                e
            )))
        })?
    }

    /// Get the path to the database file
    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_async_kvdb_basic_operations() -> Result<(), KVStoreError> {
        let db = AsyncKVDb::new(":memory:").await?;

        // Test set and get
        let test_value = "test_value".to_string();
        db.set("test_key", &test_value).await?;
        let value: Option<String> = db.get("test_key").await?;
        assert_eq!(value, Some(test_value));

        // Test get non-existent key
        let missing: Option<String> = db.get("missing_key").await?;
        assert_eq!(missing, None);

        Ok(())
    }

    #[tokio::test]
    async fn test_async_kvdb_json_serialization() -> Result<(), KVStoreError> {
        use serde::{Deserialize, Serialize};

        #[derive(Debug, Serialize, Deserialize, PartialEq)]
        struct TestStruct {
            field1: String,
            field2: i32,
        }

        let db = AsyncKVDb::new(":memory:").await?;

        let test_data = TestStruct {
            field1: "hello".to_string(),
            field2: 42,
        };

        db.set("struct_key", &test_data).await?;
        let result: Option<TestStruct> = db.get("struct_key").await?;
        assert_eq!(result, Some(test_data));

        Ok(())
    }

    #[tokio::test]
    async fn test_async_kvdb_concurrent_access() -> Result<(), KVStoreError> {
        let db = AsyncKVDb::new(":memory:").await?;

        // Spawn multiple tasks that access the DB concurrently
        let mut handles = vec![];

        for i in 0..10 {
            let db_clone = db.clone();
            let handle = tokio::spawn(async move {
                let key = format!("key_{}", i);
                let value = format!("value_{}", i);
                db_clone.set(key.clone(), &value).await?;
                let result: Option<String> = db_clone.get(key).await?;
                assert_eq!(result, Some(value));
                Ok::<_, KVStoreError>(())
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        for handle in handles {
            handle.await.unwrap()?;
        }

        Ok(())
    }
}
