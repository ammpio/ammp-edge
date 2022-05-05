use std::env;

pub fn load_dotenv() {
    if dotenv::dotenv().is_ok() {
        println!("Loaded local .env")
    }
    // Also load $SNAP_COMMON/.env if exists
    if let Ok(snap_common) = env::var("SNAP_COMMON") {
        let snap_common_dotenv = format!("{snap_common}/.env");
        if dotenv::from_path(&snap_common_dotenv).is_ok() {
            println!("Loaded {snap_common_dotenv}");
        }
    }
}