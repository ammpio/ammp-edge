use crate::helpers::base_path;

pub fn load_dotenv() {
    dotenv::dotenv().ok();

    // Also load .env from data dir if exists
    dotenv::from_path(&format!("{}/.env", base_path::data_dir())).ok();
}
