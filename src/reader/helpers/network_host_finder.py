import logging
from kvstore import KVStore

logger = logging.getLogger(__name__)

kvs = KVStore()


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
        mac = address['mac'].replace(':', '').lower()
        host_info = kvs.get(f"env:net:mac:{mac}")
        if host_info and host_info.get('ipv4'):
            address['host'] = host_info['ipv4']
