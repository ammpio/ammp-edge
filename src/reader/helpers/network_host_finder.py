import logging
from pyroute2 import IPRoute
from kvstore import KVStore

logger = logging.getLogger(__name__)

kvs = KVStore()
ipr = IPRoute()


def arp_get_mac_from_ip(ip: str) -> str:
    if not isinstance(ip, str):
        logger.error(f"IP address must be string. Provided: {ip}")
        return None

    try:
        arp_for_ip = ipr.get_neighbours(family=2, dst=ip)
    except Exception:
        logger.exception(f"Exception while ipr.get_neighbours for IP {ip}")
        return None

    if len(arp_for_ip) == 1:
        mac = arp_for_ip[0].get_attr('NDA_LLADDR')
        if isinstance(mac, str):
            mac = mac.lower()
            logger.debug(f"ARP table: Obtained MAC {mac} from IP {ip}")
            return mac

    logger.debug(f"Cannot get MAC based on ARP table entries when looking for IP {ip}: {arp_for_ip}")
    return None


def arp_get_ip_from_mac(mac: str) -> str:
    if not isinstance(mac, str):
        logger.error(f"MAC address must be string. Provided: {mac}")
        return None

    try:
        arp_for_mac = ipr.get_neighbours(family=2, lladdr=mac.lower())
    except Exception:
        logger.exception(f"Exception while running ipr.get_neighbours for MAC {mac}")
        return None

    if len(arp_for_mac) == 1:
        ip = arp_for_mac[0].get_attr('NDA_DST')
        if isinstance(ip, str):
            logger.debug(f"ARP table: Obtained IP {ip} from MAC {mac}")
            return ip

    logger.debug(f"Cannot get IP based on ARP table entries when looking for MAC {mac}: {arp_for_mac}")
    return None


def trigger_network_scan() -> None:
    from env_scan_svc import main as do_env_scan
    do_env_scan()


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

        # If not available in ARP cache, look in key-value store
        if not ip:
            ip = kvs.get(f"env:net:mac:{mac}").get('ipv4')
            logger.debug(f"KVS cache: Obtained IP {ip} from MAC {mac}")

            if not ip:
                # Still no IP obtained
                return

            # Check IP from key-value store to make sure it does not contradict ARP cache
            mac_from_arp = arp_get_mac_from_ip(ip)
            if mac_from_arp and mac_from_arp != mac:
                logger.debug(f"MAC from cache ({mac_from_arp}) does not match requested MAC ({mac})")
                logger.debug(f"Triggering network scan")
                trigger_network_scan()
                return

        # Set the host IP
        logger.debug(f"Setting host IP of MAC {mac} to {ip}")
        address['host'] = ip
