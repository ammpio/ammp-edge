import logging
from threading import Lock, Thread
from time import sleep

from kvstore import KVCache, keys

logger = logging.getLogger(__name__)

kvc = KVCache()

scan_in_progress = Lock()
# Time to pause after a scan, before the next scan can be triggered
WAIT_AFTER_SCAN = 900
ARP_TABLE_FILE = "/proc/net/arp"
INVALID_MAC = "00:00:00:00:00:00"

# Note that we expect /proc/net/arp to look like this. 6 columns, with IP and MAC in 1st and 4th col:
# IP address       HW type     Flags       HW address            Mask     Device
# 192.168.12.31    0x1         0x2         00:09:6b:00:02:03     *        eth0
# 192.168.12.70    0x1         0x2         00:01:02:38:4c:85     *        eth0


def arp_get_mac_from_ip(ip: str) -> str:
    try:
        with open(ARP_TABLE_FILE, "r") as arp_table:
            # Skip header row
            next(arp_table)
            for l in arp_table:
                try:
                    this_ip, _, _, this_mac, _, _ = l.split()
                except ValueError:
                    logger.warning(f"Malformed ARP table entry: {l}. Skipping")
                    continue
                if this_mac == INVALID_MAC:
                    logger.debug(f"Ignoring MAC address with only zeros for IP: {this_ip}, consider flushing ARP cache")
                    continue
                if this_ip == ip:
                    logger.debug(f"Mapped {ip} -> {this_mac} based on ARP table")
                    return this_mac
            else:
                logger.info(f"IP {ip} not found in ARP table")

    except FileNotFoundError:
        logger.warning(f"Unable to load ARP table from {ARP_TABLE_FILE}")
    except Exception:
        logger.exception(f"Exception while looking for IP {ip} in ARP table")


def arp_get_ip_from_mac(mac: str) -> str:
    if not isinstance(mac, str):
        logger.warning(f"MAC must be string. Received {mac}")
        return None

    try:
        with open(ARP_TABLE_FILE, "r") as arp_table:
            # Skip header row
            next(arp_table)
            for arp_line in arp_table:
                try:
                    this_ip, _, _, this_mac, _, _ = arp_line.split()
                except ValueError:
                    logger.warning(f"Malformed ARP table entry: {arp_line}. Skipping")
                    continue
                if this_mac == INVALID_MAC:
                    logger.debug(f"Ignoring MAC address with only zeros for IP: {this_ip}, consider flushing ARP cache")
                    continue
                if this_mac == mac.lower():
                    logger.debug(f"Mapped {mac} -> {this_ip} based on ARP table")
                    return this_ip
            else:
                logger.info(f"MAC {mac.lower()} not found in ARP table")

    except FileNotFoundError:
        logger.warning(f"Unable to load ARP table from {ARP_TABLE_FILE}")
    except Exception:
        logger.exception(f"Exception while looking for IP {mac} in ARP table")


def network_scan_thread() -> None:
    from env_scan_svc import main as do_env_scan

    with scan_in_progress:
        do_env_scan()
        logger.info(f"Scan complete. Sleeping {WAIT_AFTER_SCAN}")
        sleep(WAIT_AFTER_SCAN)


def trigger_network_scan() -> None:
    global scan_in_progress
    if scan_in_progress.locked():
        logger.info(f"Scan is in progress or completed within last {WAIT_AFTER_SCAN} seconds. Not scanning again.")
        return

    scan_thread = Thread(target=network_scan_thread, name="network_scan", daemon=True)
    scan_thread.start()


def set_host_from_mac(address: dict) -> None:
    """
    Obtains the IP address of a host.
    The passed `address` dict should contain at least one of 'host' (an IP address)
    or 'mac' (a MAC address). The logic for getting the right IP is as follows:
    If a MAC is defined, we check for it in the network scan in the key-value store.
    If the MAC is found there, we set the 'host' to the associated IP address.
    If the MAC is not defined, or is not found in the network scan, we do not touch
    the defined 'host' (if any).

    Note that we (potentially) modify the 'host' value of the dict that is passed
    here "in place". I.e. no value is returned to the caller.
    """

    if address.get("mac"):

        mac = address["mac"].lower()

        # First try ARP cache:
        ip = arp_get_ip_from_mac(mac)

        # If not available in ARP cache, look in key-value store
        if not ip:
            logger.info(f"MAC {mac} not found in ARP cache; looking in k-v store")
            ip = kvc.get(f"{keys.ENV_NET_MAC_PFX}/{mac}", {}).get("ipv4")
            logger.debug(f"KVS cache: Obtained IP {ip} from MAC {mac}")

            if not ip:
                logger.info(f"Could not get IP for MAC {mac} from ARP cache or k-v store; triggering network scan")
                trigger_network_scan()
                return

        # Set the host IP
        logger.info(f"Setting host IP of MAC {mac} to {ip}")
        address["host"] = ip


def check_host_vs_mac(address: dict) -> bool:
    """
    Checks if the IP and MAC address provided match up.
    Idea is to carry out this check following a readout, which would have triggered an ARP update.
    """

    if address.get("mac") and address.get("host"):
        # Only carry out check if both MAC and IP are set
        set_mac = address["mac"].lower()
        set_ip = address["host"]

        logger.debug(f"Checking {set_mac} vs {set_ip}")

        # Get MAC from ARP cache. Worth having in mind that the readout operation would (if needed) have updated
        # the MAC record corresponding to the IP being read, not vice versa - so we need to use the IP as key
        mac_for_set_ip = arp_get_mac_from_ip(set_ip)

        if mac_for_set_ip is None:
            logger.warn(f"No MAC obtained for {set_ip} from ARP cache. ARP malfunction?")
            # This is weird, but let's trigger a scan and return True so we're not discarding data that's probably fine
            trigger_network_scan()
            return True

        if set_mac == mac_for_set_ip:
            logger.debug(f"Confirmed set MAC {set_mac} is correct for IP {set_ip}")
            return True
        else:
            logger.warn(f"Mismatch between set MAC ({set_mac}) and actual MAC ({mac_for_set_ip}) for IP {set_ip}")
            trigger_network_scan()
            return False
    else:
        # Nothing to check
        return True
