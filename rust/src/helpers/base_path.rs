use std::{env, path::PathBuf};

use once_cell::sync::Lazy;

pub static ROOT_DIR: Lazy<PathBuf> = Lazy::new(|| {
    if let Ok(ae_root_dir) = env::var("AE_ROOT_DIR") {
        return ae_root_dir.into();
    }
    if let Ok(snap_dir) = env::var("SNAP") {
        return snap_dir.into();
    }
    PathBuf::from(".")
});

pub static DATA_DIR:Lazy<PathBuf> = Lazy::new(|| {
    if let Ok(ae_data_dir) = env::var("AE_DATA_DIR") {
        return ae_data_dir.into();
    }
    if let Ok(snap_common_dir) = env::var("SNAP_COMMON") {
        return snap_common_dir.into();
    }
    ROOT_DIR.join("data")
});

pub static TEMP_DIR: Lazy<PathBuf> = Lazy::new(|| PathBuf::from("/tmp"));
