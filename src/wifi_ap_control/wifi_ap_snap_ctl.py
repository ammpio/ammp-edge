import logging
import os
import requests_unixsocket
from requests.exceptions import ConnectionError
from time import sleep
from urllib.parse import quote

logger = logging.getLogger(__name__)

SOCKET_PATH: str = os.path.join(os.getenv('SNAP_DATA', ''), 'sockets', 'wifi-ap', 'control')
INIT_RETRY_HOLDOFF: int = 10
INIT_RETRY_COUNT: int = 30

DEFAULT_CONFIG = {
    'disabled': False,
    'debug': False,
    'wifi.interface': 'wlan0',
    'wifi.address': '192.168.4.1',
    'wifi.netmask': '255.255.255.0',
    'wifi.interface-mode': 'direct',
    'wifi.hostapd-driver': 'nl80211',
    'wifi.ssid': 'ammp-edge',
    'wifi.security': 'wpa2',
    'wifi.security-passphrase': 'ammp12345',
    'wifi.channel': 6,
    'wifi.country-code': '',
    'wifi.operation-mode': 'virtual',
    'share.disabled': True,
    'share.network-interface': 'eth0',
    'dhcp.range-start': '192.168.4.100',
    'dhcp.range-stop': '192.168.4.200',
    'dhcp.lease-time': '12h'
}


class WifiAPSnapCtl(object):

    def __init__(self) -> None:

        # Make sure we are able to read status from the wifi-ap API
        retries = 0
        # while True:
        #     try:
        #         with requests_unixsocket.Session() as s:
        #             res = s.get(f"http+unix://{quote(SOCKET_PATH, safe='')}/v1/status")
        #         logger.info(f"Response from wifi-ap snap API: Status {res.status_code} / {res.text}")
        #         break
        #     except ConnectionError as e:
        #         logger.error(f"Connection error while making wifi-ap API socket request: {e}")
        #         if retries < INIT_RETRY_COUNT:
        #             sleep(INIT_RETRY_HOLDOFF)
        #             retries += 1
        #             logger.info(f"Retrying ({retries}/{INIT_RETRY_COUNT})")
        #         else:
        #             raise(e)

    def configure(self, config: dict) -> bool:

        # For any parameters that are not explicitly defined in the submitted config, use defaults
        conf_payload = DEFAULT_CONFIG.copy()
        if isinstance(config, dict):
            conf_payload.update(config)
        elif config is None:
            logger.info("No Wifi AP config provided; using default")
        else:
            logger.warn(f"Wifi AP config type must be dict. Provided: {type(config)}. Using default config")

        try:
            with requests_unixsocket.Session() as s:
                res = s.post(f"http+unix://{quote(SOCKET_PATH, safe='')}/v1/configuration", json=conf_payload)
                logger.info(f"Response from wifi-ap snap API: Status {res.status_code} / {res.text}")
                return res.status_code == 200
        except ConnectionError as e:
            logger.error(f"Connection error while making wifi-ap API socket request: {e}")
        except Exception as e:
            logger.exception(f"Exception while making wifi-ap API socket request: {e}")
