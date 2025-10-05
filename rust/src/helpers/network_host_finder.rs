//! Network host finder functionality
//!
//! This module provides functionality to resolve device IP addresses from MAC addresses
//! using the system ARP table and fallback mechanisms, equivalent to the Python
//! network_host_finder.py module.

use std::fs;
use std::net::Ipv4Addr;
use std::process::Command;
use std::str::FromStr;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use anyhow::{Result, anyhow};
use kvstore::KVDb;
use once_cell::sync::Lazy;
use serde::Deserialize;

use crate::constants::keys;
use crate::helpers::base_path;
use crate::interfaces::kvpath;

/// Path to the Linux ARP table
const ARP_TABLE_FILE: &str = "/proc/net/arp";
/// Invalid MAC address that should be ignored
const INVALID_MAC: &str = "00:00:00:00:00:00";
/// Time to wait after a scan before allowing another
const WAIT_AFTER_SCAN: Duration = Duration::from_mins(15);

/// Global mutex to track if a network scan is in progress or recently completed
static SCAN_IN_PROGRESS: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

/// Network scan cache entry structure
#[derive(Debug, Deserialize)]
struct NetworkScanEntry {
    ipv4: Option<String>,
}

/// Represents an entry in the ARP table
#[derive(Debug, Clone, PartialEq)]
struct ArpEntry {
    pub ip: Ipv4Addr,
    pub mac: String,
    pub device: String,
}

/// Parse the system ARP table from /proc/net/arp
///
/// The ARP table format is expected to be:
/// ```text
/// IP address       HW type     Flags       HW address            Mask     Device
/// 192.168.12.31    0x1         0x2         00:09:6b:00:02:03     *        eth0
/// 192.168.12.70    0x1         0x2         00:01:02:38:4c:85     *        eth0
/// ```
fn parse_arp_table() -> Result<Vec<ArpEntry>> {
    let contents = fs::read_to_string(ARP_TABLE_FILE)
        .map_err(|e| anyhow!("Unable to load ARP table from {}: {}", ARP_TABLE_FILE, e))?;

    let mut entries = Vec::new();
    let mut lines = contents.lines();

    // Skip header line
    if let Some(_header) = lines.next() {
        for line in lines {
            match parse_arp_line(line) {
                Ok(Some(entry)) => entries.push(entry),
                Ok(None) => {
                    log::trace!("Skipping invalid ARP entry: {}", line);
                }
                Err(e) => {
                    log::warn!("Malformed ARP table entry '{}': {}. Skipping", line, e);
                }
            }
        }
    }

    Ok(entries)
}

/// Parse a single line from the ARP table
fn parse_arp_line(line: &str) -> Result<Option<ArpEntry>> {
    let parts: Vec<&str> = line.split_whitespace().collect();

    if parts.len() != 6 {
        return Err(anyhow!("Expected 6 columns, found {}", parts.len()));
    }

    let ip_str = parts[0];
    let mac_str = parts[3].to_lowercase();
    let device_str = parts[5];

    // Skip invalid MAC addresses
    if mac_str == INVALID_MAC {
        log::debug!(
            "Ignoring MAC address with only zeros for IP: {}, consider flushing ARP cache",
            ip_str
        );
        return Ok(None);
    }

    let ip = Ipv4Addr::from_str(ip_str)
        .map_err(|e| anyhow!("Invalid IP address '{}': {}", ip_str, e))?;

    Ok(Some(ArpEntry {
        ip,
        mac: mac_str,
        device: device_str.to_string(),
    }))
}

/// Get MAC address from IP address using the ARP table
pub fn arp_get_mac_from_ip(ip: &str) -> Result<Option<String>> {
    let target_ip =
        Ipv4Addr::from_str(ip).map_err(|e| anyhow!("Invalid IP address '{}': {}", ip, e))?;

    let entries = parse_arp_table()?;

    for entry in entries {
        if entry.ip == target_ip {
            log::debug!("Mapped {} -> {} based on ARP table", ip, entry.mac);
            return Ok(Some(entry.mac));
        }
    }

    log::info!("IP {} not found in ARP table", ip);
    Ok(None)
}

/// Get IP address from MAC address using the ARP table, with fallback to cache
pub fn arp_get_ip_from_mac(mac: &str) -> Result<Option<String>> {
    let target_mac = mac.to_lowercase();

    // First try ARP table
    let entries = parse_arp_table()?;

    for entry in entries {
        if entry.mac == target_mac {
            log::debug!("Mapped {} -> {} based on ARP table", mac, entry.ip);
            return Ok(Some(entry.ip.to_string()));
        }
    }

    log::info!("MAC {} not found in ARP table", target_mac);

    // If not in ARP table, try key-value cache
    log::info!(
        "MAC {} not found in ARP cache; looking in k-v store",
        target_mac
    );

    match try_get_ip_from_cache(&target_mac) {
        Ok(Some(ip)) => {
            log::debug!("Obtained IP {} from MAC {} (KV cache)", ip, target_mac);
            Ok(Some(ip))
        }
        Ok(None) => {
            log::info!(
                "Could not get IP for MAC {} from ARP cache or KV cache; triggering network scan",
                target_mac
            );
            trigger_network_scan();
            Ok(None)
        }
        Err(e) => {
            log::warn!("Error reading from KV cache: {}", e);
            trigger_network_scan();
            Ok(None)
        }
    }
}

/// Try to get IP from MAC using the key-value cache
fn try_get_ip_from_cache(mac: &str) -> Result<Option<String>> {
    let cache = KVDb::new(kvpath::SQLITE_CACHE.as_path())?;
    let key = format!("{}/{}", keys::ENV_NET_MAC_PFX, mac);

    let entry: Option<NetworkScanEntry> = cache.get(&key)?;

    Ok(entry.and_then(|e| e.ipv4))
}

/// Trigger a network scan in a background thread
///
/// If a scan is already in progress or recently completed (within 15 minutes),
/// this function will not trigger a new scan.
fn trigger_network_scan() {
    // Try to acquire the lock without blocking
    if SCAN_IN_PROGRESS.try_lock().is_err() {
        log::info!(
            "Scan is in progress or completed within last {:?}. Not scanning again.",
            WAIT_AFTER_SCAN
        );
        return;
    }

    // Spawn a thread to run the scan
    thread::spawn(|| {
        network_scan_thread();
    });
}

/// Run the network scan and sleep to prevent rapid re-scanning
fn network_scan_thread() {
    // Acquire the lock - this will hold it for the duration of the scan + wait period
    let _guard = match SCAN_IN_PROGRESS.lock() {
        Ok(guard) => guard,
        Err(e) => {
            log::error!("Failed to acquire scan lock: {}", e);
            return;
        }
    };

    log::info!("Starting network scan");

    // Run the env_scan_svc binary
    let scan_binary = base_path::ROOT_DIR.join("bin/env_scan_svc");

    match Command::new(&scan_binary).output() {
        Ok(output) => {
            if output.status.success() {
                log::info!("Network scan completed successfully");
            } else {
                log::warn!("Network scan exited with status: {}", output.status);
                if !output.stderr.is_empty() {
                    log::warn!("Scan stderr: {}", String::from_utf8_lossy(&output.stderr));
                }
            }
        }
        Err(e) => {
            log::error!(
                "Failed to execute network scan at {}: {}",
                scan_binary.display(),
                e
            );
        }
    }

    // Sleep for the wait period before releasing the lock
    log::info!("Scan complete. Sleeping {:?}", WAIT_AFTER_SCAN);
    thread::sleep(WAIT_AFTER_SCAN);

    // Lock is automatically released when _guard goes out of scope
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_arp_line() {
        // Valid ARP line
        let line = "192.168.1.100    0x1         0x2         aa:bb:cc:dd:ee:ff     *        eth0";
        let result = parse_arp_line(line).unwrap();
        assert!(result.is_some());

        let entry = result.unwrap();
        assert_eq!(entry.ip.to_string(), "192.168.1.100");
        assert_eq!(entry.mac, "aa:bb:cc:dd:ee:ff");
        assert_eq!(entry.device, "eth0");

        // Invalid MAC (all zeros)
        let line_invalid_mac =
            "192.168.1.100    0x1         0x2         00:00:00:00:00:00     *        eth0";
        let result = parse_arp_line(line_invalid_mac).unwrap();
        assert!(result.is_none());

        // Malformed line (too few columns)
        let line_malformed = "192.168.1.100    0x1         0x2";
        assert!(parse_arp_line(line_malformed).is_err());
    }

    // Mock test data for testing parsing logic
    const MOCK_ARP_TABLE: &str =
        "IP address       HW type     Flags       HW address            Mask     Device
192.168.1.1      0x1         0x2         aa:bb:cc:dd:ee:ff     *        eth0
192.168.1.2      0x1         0x2         11:22:33:44:55:66     *        eth0
192.168.1.3      0x1         0x0         00:00:00:00:00:00     *        eth0";

    #[test]
    fn test_parse_mock_arp_data() {
        // Test parsing logic with controlled data
        let lines: Vec<&str> = MOCK_ARP_TABLE.lines().collect();

        // Skip header and test each line
        for line in &lines[1..] {
            let result = parse_arp_line(line);

            if line.contains("00:00:00:00:00:00") {
                // Should skip invalid MAC
                assert!(result.unwrap().is_none());
            } else if line.contains("192.168.1.1") {
                // Should parse valid entry
                let entry = result.unwrap().unwrap();
                assert_eq!(entry.ip.to_string(), "192.168.1.1");
                assert_eq!(entry.mac, "aa:bb:cc:dd:ee:ff");
                assert_eq!(entry.device, "eth0");
            }
        }
    }
}
