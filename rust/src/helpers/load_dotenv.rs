use crate::helpers::base_path;

pub fn load_dotenv() {
    dotenv::dotenv().ok();

    // Also load .env from data dir if exists
    dotenv::from_path(base_path::DATA_DIR.join(".env")).ok();
}
