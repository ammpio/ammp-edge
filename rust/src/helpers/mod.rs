mod backoff_retry;
mod commands;
mod load_dotenv;
mod node_meta;
mod time;

pub use backoff_retry::backoff_retry;
pub use commands::run_command;
pub use load_dotenv::load_dotenv;
pub use node_meta::{get_node_arch, get_ssh_fingerprint};
pub use time::{now_epoch, now_iso};

pub mod base_path;

use getrandom::getrandom;

pub fn rand_hex(bytes: usize) -> String {
    let mut rand = vec![0u8; bytes];
    getrandom(&mut rand).unwrap();
    hex::encode(rand)
}
