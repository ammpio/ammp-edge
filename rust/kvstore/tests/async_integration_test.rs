use serde::{Deserialize, Serialize};
use tempfile::tempdir;

use kvstore::{AsyncKVDb, KVStoreError};

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
pub struct Cat {
    name: String,
    lives: u64,
    siblings: Vec<String>,
}

/// Test basic async write and read of objects
#[tokio::test]
async fn async_write_and_read_object() -> Result<(), KVStoreError> {
    let sqlite_db = tempdir()?.path().join("async_kvstore.db");

    let db = AsyncKVDb::new(&sqlite_db).await?;

    let lulu = Cat {
        name: String::from("Lulu"),
        lives: 9,
        siblings: vec![String::from("Mollie"), String::from("Lilly")],
    };

    db.set("lulu", &lulu).await?;
    let lulu2: Cat = db.get("lulu").await?.unwrap();
    assert_eq!(lulu2, lulu);

    Ok(())
}

/// Test persistence: data written by one instance should be readable by another
#[tokio::test]
async fn async_persistence_across_instances() -> Result<(), KVStoreError> {
    let sqlite_db = tempdir()?.path().join("async_kvstore_persistence.db");

    let cat = Cat {
        name: String::from("Whiskers"),
        lives: 7,
        siblings: vec![String::from("Fluffy")],
    };

    // Write with first instance
    {
        let db1 = AsyncKVDb::new(&sqlite_db).await?;
        db1.set("cat1", &cat).await?;
    }

    // Read with second instance
    {
        let db2 = AsyncKVDb::new(&sqlite_db).await?;
        let cat2: Cat = db2.get("cat1").await?.unwrap();
        assert_eq!(cat2, cat);
    }

    Ok(())
}

/// Test concurrent reads from multiple tasks
#[tokio::test]
async fn async_concurrent_reads() -> Result<(), KVStoreError> {
    let sqlite_db = tempdir()?.path().join("async_kvstore_concurrent_reads.db");
    let db = AsyncKVDb::new(&sqlite_db).await?;

    // Write test data
    let cat = Cat {
        name: String::from("ConcurrentCat"),
        lives: 9,
        siblings: vec![String::from("Sibling1"), String::from("Sibling2")],
    };
    db.set("concurrent_cat", &cat).await?;

    // Spawn multiple concurrent read tasks
    let mut handles = vec![];
    for i in 0..20 {
        let db_clone = db.clone();
        let cat_clone = cat.clone();
        let handle = tokio::spawn(async move {
            for _ in 0..5 {
                let result: Cat = db_clone
                    .get("concurrent_cat")
                    .await
                    .expect("Read failed")
                    .expect("Value not found");
                assert_eq!(result, cat_clone, "Read mismatch in task {}", i);
            }
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.expect("Task panicked");
    }

    Ok(())
}

/// Test concurrent writes to different keys
#[tokio::test]
async fn async_concurrent_writes_different_keys() -> Result<(), KVStoreError> {
    let sqlite_db = tempdir()?.path().join("async_kvstore_concurrent_writes.db");
    let db = AsyncKVDb::new(&sqlite_db).await?;

    let mut handles = vec![];
    for i in 0..50 {
        let db_clone = db.clone();
        let handle = tokio::spawn(async move {
            let cat = Cat {
                name: format!("Cat_{}", i),
                lives: (i % 10) as u64,
                siblings: vec![format!("Sibling_{}", i)],
            };
            let key = format!("cat_{}", i);
            db_clone.set(key.clone(), &cat).await.expect("Write failed");

            // Verify the write
            let result: Cat = db_clone
                .get(key)
                .await
                .expect("Read failed")
                .expect("Value not found");
            assert_eq!(result, cat);
        });
        handles.push(handle);
    }

    // Wait for all writes to complete
    for handle in handles {
        handle.await.expect("Task panicked");
    }

    // Verify all values are still accessible
    for i in 0..50 {
        let key = format!("cat_{}", i);
        let result: Option<Cat> = db.get(key).await?;
        assert!(result.is_some(), "Cat {} not found", i);
        assert_eq!(result.unwrap().name, format!("Cat_{}", i));
    }

    Ok(())
}

/// Test mixed concurrent reads and writes
#[tokio::test]
async fn async_mixed_concurrent_operations() -> Result<(), KVStoreError> {
    let sqlite_db = tempdir()?.path().join("async_kvstore_mixed_ops.db");
    let db = AsyncKVDb::new(&sqlite_db).await?;

    // Initialize some data
    for i in 0..10 {
        let cat = Cat {
            name: format!("InitialCat_{}", i),
            lives: i as u64,
            siblings: vec![],
        };
        db.set(format!("cat_{}", i), &cat).await?;
    }

    let mut handles = vec![];

    // Spawn reader tasks
    for i in 0..30 {
        let db_clone = db.clone();
        let handle = tokio::spawn(async move {
            let key = format!("cat_{}", i % 10);
            let _result: Option<Cat> = db_clone.get(key).await.expect("Read failed");
            // Result might be old or new value depending on timing
        });
        handles.push(handle);
    }

    // Spawn writer tasks
    for i in 0..10 {
        let db_clone = db.clone();
        let handle = tokio::spawn(async move {
            let cat = Cat {
                name: format!("UpdatedCat_{}", i),
                lives: (i + 100) as u64,
                siblings: vec![format!("NewSibling_{}", i)],
            };
            db_clone
                .set(format!("cat_{}", i), &cat)
                .await
                .expect("Write failed");
        });
        handles.push(handle);
    }

    // Wait for all operations
    for handle in handles {
        handle.await.expect("Task panicked");
    }

    Ok(())
}

/// Test reading non-existent keys
#[tokio::test]
async fn async_read_nonexistent_key() -> Result<(), KVStoreError> {
    let sqlite_db = tempdir()?.path().join("async_kvstore_nonexistent.db");
    let db = AsyncKVDb::new(&sqlite_db).await?;

    let result: Option<Cat> = db.get("does_not_exist").await?;
    assert!(result.is_none());

    Ok(())
}

/// Test overwriting existing values
#[tokio::test]
async fn async_overwrite_values() -> Result<(), KVStoreError> {
    let sqlite_db = tempdir()?.path().join("async_kvstore_overwrite.db");
    let db = AsyncKVDb::new(&sqlite_db).await?;

    let cat_v1 = Cat {
        name: "Original".to_string(),
        lives: 9,
        siblings: vec![],
    };

    let cat_v2 = Cat {
        name: "Updated".to_string(),
        lives: 8,
        siblings: vec!["NewSibling".to_string()],
    };

    db.set("mutable_cat", &cat_v1).await?;
    let retrieved_v1: Cat = db.get("mutable_cat").await?.unwrap();
    assert_eq!(retrieved_v1.name, "Original");

    db.set("mutable_cat", &cat_v2).await?;
    let retrieved_v2: Cat = db.get("mutable_cat").await?.unwrap();
    assert_eq!(retrieved_v2.name, "Updated");
    assert_eq!(retrieved_v2.siblings.len(), 1);

    Ok(())
}

/// Test that AsyncKVDb can handle binary-like data through JSON serialization
#[tokio::test]
async fn async_binary_data_as_vec() -> Result<(), KVStoreError> {
    let sqlite_db = tempdir()?.path().join("async_kvstore_binary.db");
    let db = AsyncKVDb::new(&sqlite_db).await?;

    let data: Vec<u8> = vec![0x00, 0x01, 0x02, 0xff, 0xfe, 0xfd];
    db.set("binary_key", &data).await?;

    let retrieved: Vec<u8> = db.get("binary_key").await?.unwrap();
    assert_eq!(retrieved, data);

    Ok(())
}

/// Test high-volume sequential operations
#[tokio::test]
async fn async_high_volume_sequential() -> Result<(), KVStoreError> {
    let sqlite_db = tempdir()?.path().join("async_kvstore_high_volume.db");
    let db = AsyncKVDb::new(&sqlite_db).await?;

    // Write 1000 entries sequentially
    for i in 0..1000 {
        let cat = Cat {
            name: format!("Cat_{:04}", i),
            lives: (i % 9 + 1) as u64,
            siblings: vec![],
        };
        db.set(format!("cat_{:04}", i), &cat).await?;
    }

    // Read them back
    for i in 0..1000 {
        let cat: Cat = db
            .get(format!("cat_{:04}", i))
            .await?
            .unwrap_or_else(|| panic!("Cat {:04} not found", i));
        assert_eq!(cat.name, format!("Cat_{:04}", i));
    }

    Ok(())
}

/// Test that multiple AsyncKVDb instances can coexist and share data via WAL
#[tokio::test]
async fn async_multiple_instances_wal() -> Result<(), KVStoreError> {
    let sqlite_db = tempdir()?.path().join("async_kvstore_wal.db");

    let db1 = AsyncKVDb::new(&sqlite_db).await?;
    let db2 = AsyncKVDb::new(&sqlite_db).await?;

    let cat1 = Cat {
        name: "WrittenByDB1".to_string(),
        lives: 5,
        siblings: vec![],
    };

    let cat2 = Cat {
        name: "WrittenByDB2".to_string(),
        lives: 7,
        siblings: vec![],
    };

    // Write with db1
    db1.set("cat_from_db1", &cat1).await?;

    // Write with db2
    db2.set("cat_from_db2", &cat2).await?;

    // Read from db1 what db2 wrote
    let retrieved_from_db1: Cat = db1.get("cat_from_db2").await?.unwrap();
    assert_eq!(retrieved_from_db1, cat2);

    // Read from db2 what db1 wrote
    let retrieved_from_db2: Cat = db2.get("cat_from_db1").await?.unwrap();
    assert_eq!(retrieved_from_db2, cat1);

    Ok(())
}
