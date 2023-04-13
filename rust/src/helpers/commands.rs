use std::collections::HashSet;
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use once_cell::sync::Lazy;

use crate::helpers::base_path;

static CMD_BASE_DIR: Lazy<PathBuf> = Lazy::new(|| base_path::ROOT_DIR.join("bin/cmd"));

static VALID_COMMANDS: Lazy<HashSet<OsString>> = Lazy::new(|| {
    fs::read_dir(CMD_BASE_DIR.as_path())
        .unwrap()
        .map(|res| res.map(|e| e.file_name()).unwrap())
        .collect()
});

pub fn run_command(cmd: &str) -> String {
    if !VALID_COMMANDS.contains(&OsString::from(cmd)) {
        let message = format!("Unrecognized command: {cmd}");
        log::error!("{}", message);
        return message;
    }

    let cmd_path = CMD_BASE_DIR.join(cmd);

    match Command::new(cmd_path).output() {
        Ok(output) => {
            let message = format!(
                "Command: {}\n{}\nstdout: {}\nstderr: {}",
                cmd,
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            log::info!("{}", message);
            message
        }
        Err(e) => {
            let message = format!("Could not run {}; error: {}", cmd, e);
            log::error!("{}", message);
            message
        }
    }
}
