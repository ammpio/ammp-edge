import logging
from pyroute2 import NDB
from kvstore import KVStore

logger = logging.getLogger(__name__)

ndb = NDB()
kvs = KVStore()


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


def trigger_network_scan() -> None:
    from env_scan_svc import main as do_env_scan
    do_env_scan()


def set_host_from_mac(address: dict, retrying: bool = False) -> None:
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
            logger.info(f"MAC {mac} not found in ARP cache; looking in k-v store")
            ip = kvs.get(f"env:net:mac:{mac}", {}).get('ipv4')
            logger.debug(f"KVS cache: Obtained IP {ip} from MAC {mac}")

            if not ip:
                # Still no IP obtained
                return

            # Check IP from key-value store to make sure it does not contradict ARP cache
            mac_from_arp = arp_get_mac_from_ip(ip)
            if mac_from_arp and mac_from_arp != mac:
                logger.warn(f"MAC from ARP cache ({mac_from_arp}) for IP {ip} does not match requested MAC ({mac})")
                logger.info(f"Triggering network scan")
                trigger_network_scan()
                # Following the scan, try to set the host again. But skip if this is already a retry.
                if not retrying:
                    set_host_from_mac(address, retrying=True)
                return

        # Set the host IP
        logger.info(f"Setting host IP of MAC {mac} to {ip}")
        address['host'] = ip
