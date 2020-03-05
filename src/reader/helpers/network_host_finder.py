import logging
from pyroute2 import IPRoute
from kvstore import KVStore

logger = logging.getLogger(__name__)

with IPRoute() as ipr:
    try:
        neigh = ipr.get_neighbours(2)
        arp_table_by_mac = {n.get_attr("NDA_LLADDR"): n.get_attr("NDA_DST") for n in neigh}
        arp_table_by_ip = {n.get_attr("NDA_DST"): n.get_attr("NDA_LLADDR") for n in neigh}
        logger.debug(f"ARP Table: {arp_table_by_ip}")
    except Exception:
        logger.exception(f"Could not get ARP table")
        arp_table = {}


def arp_get_mac_from_ip(ip: str) -> str:
    global arp_table_by_ip
    if ip in arp_table_by_ip:
        logger.debug(f"ARP table: IP {ip} mapped")
        return arp_table_by_ip[ip]


def arp_get_ip_from_mac(mac: str) -> str:
    global arp_table_by_mac
    if mac in arp_table:
        logger.debug(f"ARP table: MAC {mac} mapped")
        return arp_table_by_mac[mac]


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
            with KVStore() as kvs:
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
