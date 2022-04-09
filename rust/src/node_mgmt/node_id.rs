use hex;
use nix::{ifaddrs::getifaddrs, sys::socket::SockAddr};
use getrandom::getrandom;

/// Uses the `getifaddrs` call to retrieve a list of network interfaces on the
/// host device and returns the first MAC address listed that isn't
/// local-loopback or if a name was specified, that name.
fn get_mac(name: Option<&str>) -> Option<[u8; 6]> {
    if let Ok(ifiter) = getifaddrs() {
        for interface in ifiter {
            if let Some(SockAddr::Link(link)) = interface.address {
                let bytes = link.addr();

                // If interface name is specified, only return corresponding MAC
                // Otherwise return first non-zero MAC
                if let Some(name) = name {
                    if interface.interface_name == name {
                        return Some(bytes);
                    }
                } else if bytes.iter().any(|&x| x != 0) {
                    return Some(bytes);
                }
            }
        }
    }

    None
}

pub fn generate_node_id() -> String {
    // Try to get MAC address based on interface list (in order)
    const IFN_PRIORITY: &[&str] = &["eth0", "en0", "eth1", "en1", "em0", "em1", "wlan0", "wlan1"];

    for ifn in IFN_PRIORITY {
        if let Some(address) = get_mac(Some(ifn)) {
            return hex::encode(address);
        }
    }

    // If not successful, get MAC of any available interface
    if let Some(address) = get_mac(None) {
        return hex::encode(address);
    }

    // If not available, generate random node ID with "ff" prefix
    let mut randmac = [0u8, 5];
    getrandom(&mut randmac).unwrap();
    format!("ff{}", hex::encode(randmac))
}
