use std::process::Command;

use anyhow::Result;

fn strip_optional_newline(s: String) -> String {
    s.strip_suffix('\n').unwrap_or(&s).to_string()
}

pub fn get_ssh_fingerprint() -> Result<String> {
    let output = Command::new("get_ssh_fingerprint.sh").output()?;

    let fingerprint = String::from_utf8(output.stdout)?;
    Ok(strip_optional_newline(fingerprint))
}

pub fn get_node_arch() -> Result<String> {
    let output = Command::new("uname").arg("-srvm").output()?;

    let arch = String::from_utf8(output.stdout)?;
    Ok(strip_optional_newline(arch))
}