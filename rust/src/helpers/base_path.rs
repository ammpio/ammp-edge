use std::{env, path::PathBuf};

use lazy_static::lazy_static;

lazy_static! {
    pub static ref ROOT_DIR: PathBuf = {
        if let Ok(ae_root_dir) = env::var("AE_ROOT_DIR") {
            return ae_root_dir.into();
        }
        if let Ok(snap_dir) = env::var("SNAP") {
            return snap_dir.into();
        }
        PathBuf::from(".")
    };

    pub static ref DATA_DIR: PathBuf = {
        if let Ok(ae_data_dir) = env::var("AE_DATA_DIR") {
            return ae_data_dir.into();
        }
        if let Ok(snap_common_dir) = env::var("SNAP_COMMON") {
            return snap_common_dir.into();
        }
        ROOT_DIR.join("data")
    };

    pub static ref TEMP_DIR: PathBuf = PathBuf::from("/tmp");
}
