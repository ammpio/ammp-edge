use std::process::Command;

use anyhow::Result;

pub fn get_ssh_fingerprint() -> Result<String> {
    let output = Command::new("get_ssh_fingerprint.sh").output()?;

    String::from_utf8(output.stdout).map_err(Into::into)
}
