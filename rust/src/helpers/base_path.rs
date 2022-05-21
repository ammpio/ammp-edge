use std::{env, path::PathBuf};

use once_cell::sync::Lazy;

use crate::constants::envvars;

pub static ROOT_DIR: Lazy<PathBuf> = Lazy::new(|| {
    if let Ok(ae_root_dir) = env::var(envvars::ROOT_DIR) {
        return ae_root_dir.into();
    }
    if let Ok(snap_dir) = env::var(envvars::SNAP) {
        return snap_dir.into();
    }
    PathBuf::from(".")
});

pub static DATA_DIR:Lazy<PathBuf> = Lazy::new(|| {
    if let Ok(ae_data_dir) = env::var(envvars::DATA_DIR) {
        return ae_data_dir.into();
    }
    if let Ok(snap_common_dir) = env::var(envvars::SNAP_COMMON) {
        return snap_common_dir.into();
    }
    ROOT_DIR.join("data")
});

#[allow(dead_code)]
pub static TEMP_DIR: Lazy<PathBuf> = Lazy::new(|| {
    if let Ok(ae_temp_dir) = env::var(envvars::TEMP_DIR) {
        return ae_temp_dir.into();
    }
    PathBuf::from("/tmp")
});
