use getrandom::getrandom;
use nix::{ifaddrs::getifaddrs, sys::socket::SockAddr};

/// Uses the `getifaddrs` call to retrieve a list of network interfaces on the
/// host device. Iterates over them and returns the MAC address corresponding to
/// the primary interface (based on priority list). If there are no matches against
/// the priority list, returns the first non-zero MAC address.
fn get_primary_mac() -> Option<[u8; 6]> {
    // Try to get MAC address based on interface list (in order)
    const IFN_PRIORITY: &[&str] = &["eth0", "en0", "eth1", "en1", "wlan0", "wlan1"];
    let mut best_prio = 99;
    let mut best_mac: Option<[u8; 6]> = None;

    if let Ok(ifiter) = getifaddrs() {
        for interface in ifiter {
            if let Some(SockAddr::Link(link)) = interface.address {
                let mac = link.addr();
                if mac.iter().any(|&x| x != 0) {
                    if best_mac.is_none() {
                        best_mac = Some(mac);
                        log::debug!("Fallback MAC: {:?}", hex::encode(best_mac.unwrap()));
                    }

                    if let Some(prio) = IFN_PRIORITY
                        .iter()
                        .position(|&x| x == interface.interface_name)
                    {
                        if prio < best_prio {
                            best_mac = Some(mac);
                            best_prio = prio;
                            log::debug!("Found MAC {:?} with priority {:?}", hex::encode(best_mac.unwrap()), best_prio);
                        }
                    }
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
