use getrandom::getrandom;
use nix::ifaddrs::getifaddrs;

// Uses the `getifaddrs` call to retrieve a list of network interfaces on the
// host device. Iterates over them and returns the MAC address corresponding to
// the primary interface (based on priority list). If there are no matches against
// the priority list, returns the first non-zero MAC address.

fn mac_is_non_zero(mac: &[u8; 6]) -> bool {
    mac.iter().any(|&x| x != 0)
}

fn get_interface_priority(interface_name: &str) -> Option<usize> {
    // Try to get MAC address based on interface list (in order)
    const IFN_PRIORITY: &[&str] = &["eth0", "en0", "eth1", "en1", "wlan0", "wlan1"];

    IFN_PRIORITY.iter().position(|&x| x == interface_name)
}

fn get_primary_mac() -> Option<[u8; 6]> {
    let mut best_prio = 99;
    let mut best_mac: Option<[u8; 6]> = None;

    if let Ok(ifiter) = getifaddrs() {
        for interface in ifiter {
            if let Some(link) = interface.address
            && let Some(link_addr) = link.as_link_addr()
            && let Some(mac) = link_addr.addr()
            && mac_is_non_zero(&mac) {
                if best_mac.is_none() {
                    best_mac = Some(mac);
                    log::debug!("Fallback MAC: {:?}", hex::encode(best_mac.unwrap()));
                }
                if let Some(prio) = get_interface_priority(&interface.interface_name)
                && prio < best_prio {
                    best_mac = Some(mac);
                    best_prio = prio;
                    log::debug!("Found MAC {:?} with priority {:?}", hex::encode(best_mac.unwrap()), best_prio);
                }
            }
        }
    }
    best_mac
}

pub fn generate_node_id() -> String {
    if let Some(mac) = get_primary_mac() {
        // Return ID based on MAC of primary address
        hex::encode(mac)
    } else {
        // If not available, generate random node ID with "ff" prefix
        log::warn!("Could not obtain node ID based on network interface; generating random");
        let mut randmac = [0u8; 5];
        getrandom(&mut randmac).unwrap();
        format!("ff{}", hex::encode(randmac))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_mac_and_node_id() {
        // Unless there is an unexpected (permission?) issue,
        // get_primary_mac() should always return _some_ MAC address
        assert!(get_primary_mac().is_some());
        assert_eq!(generate_node_id().len(), 12);
    }

    #[test]
    fn interface_priorities() {
        assert_eq!(get_interface_priority("eth0"), Some(0));
        assert_eq!(get_interface_priority("wlan0"), Some(4));
        assert_eq!(get_interface_priority("en3"), None);
    }

    #[test]
    fn non_zero_mac() {
        assert!(mac_is_non_zero(&[1, 2, 3, 4, 5, 6]));
        assert!(!mac_is_non_zero(&[0, 0, 0, 0, 0, 0]));
    }
}
