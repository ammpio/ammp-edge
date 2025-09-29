//! Network host finder functionality
//!
//! This module provides functionality to resolve device IP addresses from MAC addresses
//! using the system ARP table and fallback mechanisms, equivalent to the Python
//! network_host_finder.py module.

use anyhow::{Result, anyhow};
use std::fs;
use std::net::Ipv4Addr;
use std::str::FromStr;

/// Path to the Linux ARP table
const ARP_TABLE_FILE: &str = "/proc/net/arp";
/// Invalid MAC address that should be ignored
const INVALID_MAC: &str = "00:00:00:00:00:00";

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
                    log::debug!("Skipping invalid ARP entry: {}", line);
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

/// Get IP address from MAC address using the ARP table
pub fn arp_get_ip_from_mac(mac: &str) -> Result<Option<String>> {
    let target_mac = mac.to_lowercase();

    let entries = parse_arp_table()?;

    for entry in entries {
        if entry.mac == target_mac {
            log::debug!("Mapped {} -> {} based on ARP table", mac, entry.ip);
            return Ok(Some(entry.ip.to_string()));
        }
    }

    log::info!("MAC {} not found in ARP table", target_mac);
    Ok(None)
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
