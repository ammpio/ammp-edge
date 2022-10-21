use std::process::Command;

use crate::helpers::base_path;

const ALLOWED_COMMANDS: &[&str] = &[
    "snap_refresh",
    "snap_refresh_stable",
    "snap_refresh_beta",
    "snap_refresh_edge",
    "trigger_config_generation",
    "imt_sensor_address",
    "holykell_sensor_address_7",
    "holykell_sensor_address_8",
    "sys_reboot",
    "sys_start_snapd",
    "sys_stop_snapd",
    "sys_remount_rw",
    "sys_remount_ro",
];

pub fn run_command(cmd: String) -> String {
    if !ALLOWED_COMMANDS.contains(&cmd.as_str()) {
        let message = format!("Unrecognized command: {cmd}");
        log::error!("{}", message);
        return message;
    }
    let cmd_path = base_path::ROOT_DIR.join("bin/cmd").join(&cmd);
    match Command::new(cmd_path).output() {
        Ok(output) => {
            let message = format!(
                "Command: {}\n{}\nstdout: {}stderr: {}",
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
