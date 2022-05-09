use crate::helpers::base_path;

pub fn load_dotenv() {
    if dotenv::dotenv().is_ok() {
        println!("Loaded local .env")
    }

    let data_dir_dotenv = format!("{}/.env", base_path::data_dir());
    // Also load .env from data dir if exists
    if dotenv::from_path(&data_dir_dotenv).is_ok() {
        println!("Loaded {data_dir_dotenv}");
    }
}