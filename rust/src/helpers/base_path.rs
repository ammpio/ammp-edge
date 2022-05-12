use std::env;

pub fn root_dir() -> String {
    env::var("AE_ROOT_DIR")
        .unwrap_or_else(|_| env::var("SNAP").unwrap_or_else(|_| String::from(".")))
}

pub fn data_dir() -> String {
    env::var("AE_DATA_DIR").unwrap_or_else(|_| {
        env::var("SNAP_COMMON").unwrap_or_else(|_| format!("{}/{}", root_dir(), "/data"))
    })
}

#[allow(dead_code)]
pub fn tmp_dir() -> String {
    String::from("/tmp")
}
