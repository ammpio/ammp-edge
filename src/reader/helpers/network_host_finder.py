import logging
from threading import Thread, Lock
from time import sleep
from pyroute2 import NDB
from kvstore import KVStore

logger = logging.getLogger(__name__)

ndb = NDB()
kvs = KVStore()

scan_in_progress = Lock
# Time to pause after a scan, before the next scan can be triggered
WAIT_AFTER_SCAN = 900


def arp_get_mac_from_ip(ip: str) -> str:
    try:
        mac = ndb.neighbours[{'dst': ip}]['lladdr'].lower()
        logger.debug(f"Mapped {ip} -> {mac} based on ARP table")
        return mac
    except KeyError as e:
        logger.info(f"IP {ip} not found in ARP table")
        logger.debug(f"Error: {e}", exc_info=True)
        return None
    except Exception:
        logger.exception(f"Exception while looking for IP {ip} in ARP table")
        return None


def arp_get_ip_from_mac(mac: str) -> str:
    if not isinstance(mac, str):
        logger.warn(f"MAC must be string. Received {mac}")
        return None

    try:
        ip = ndb.neighbours[{'lladdr': mac.lower()}]['dst']
        logger.debug(f"Mapped {mac} -> {ip} based on ARP table")
        return ip
    except KeyError as e:
        logger.info(f"MAC {mac} not found in ARP table")
        logger.debug(f"Error: {e}", exc_info=True)
        return None
    except Exception:
        logger.exception(f"Exception while looking for MAC {mac} in ARP table")
        return None


def network_scan_thread() -> None:
    from env_scan_svc import main as do_env_scan
    do_env_scan()
    logger.info(f"Scan complete. Sleeping {WAIT_AFTER_SCAN}")
    sleep(WAIT_AFTER_SCAN)


def trigger_network_scan() -> None:
    global scan_in_progress
    if scan_in_progress.locked():
        logger.info(f"Scan is in progress or completed within last {WAIT_AFTER_SCAN} seconds. Not scanning again.")
        return

    with scan_in_progress:
        scan_thread = Thread(
                target=network_scan_thread,
                name='network_scan',
                daemon=True
                )
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

    if address.get('mac'):

        mac = address['mac'].lower()

        # First try ARP cache:
        ip = arp_get_ip_from_mac(mac)

        if not ip:
            logger.info(f"Could not get IP for MAC {mac} from ARP cache. Triggering network scan and returning.")
            trigger_network_scan()
            return

        # Set the host IP
        logger.info(f"Setting host IP of MAC {mac} to {ip}")
        address['host'] = ip
