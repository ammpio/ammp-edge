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

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_ROOT_DIR: &str = "/opt/ae";
    const SAMPLE_DATA_DIR: &str = "/opt/ae/data";
    const SAMPLE_TEMP_DIR: &str = "/opt/ae/tmp";

    #[test]
    fn with_ae_vars_set() {
        temp_env::with_vars(
            vec![
                (envvars::ROOT_DIR, Some(SAMPLE_ROOT_DIR)),
                (envvars::DATA_DIR, Some(SAMPLE_DATA_DIR)),
                (envvars::TEMP_DIR, Some(SAMPLE_TEMP_DIR)),
            ],
            || {
                assert_eq!(ROOT_DIR.as_os_str(), SAMPLE_ROOT_DIR);
                assert_eq!(DATA_DIR.as_os_str(), SAMPLE_DATA_DIR);
                assert_eq!(TEMP_DIR.as_os_str(), SAMPLE_TEMP_DIR);
            }
        );
    }

    // Only one test can be run at a time - since once the value
    // of any of the Lazy statics is set, it will remain the same
    // while the other tests are run (which will probably fail)
    #[test]
    #[ignore]
    fn with_snap_vars_set() {
        temp_env::with_vars(
            vec![
                (envvars::SNAP, Some(SAMPLE_ROOT_DIR)),
                (envvars::SNAP_COMMON, Some(SAMPLE_DATA_DIR)),
            ],
            || {
                assert_eq!(ROOT_DIR.as_os_str(), SAMPLE_ROOT_DIR);
                assert_eq!(DATA_DIR.as_os_str(), SAMPLE_DATA_DIR);
            }
        );
    }

    #[test]
    #[ignore]
    fn without_vars_set() {
        assert_eq!(ROOT_DIR.as_os_str(), ".");
        assert_eq!(DATA_DIR.as_os_str(), "./data");
        assert_eq!(TEMP_DIR.as_os_str(), "/tmp");
    }
}