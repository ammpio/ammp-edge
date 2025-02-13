import logging
import os
from time import sleep
from urllib.parse import quote

import requests_unixsocket
from requests.exceptions import ConnectionError

logger = logging.getLogger(__name__)

DEFAULT_SOCKET_PATH: str = os.path.join(os.getenv("SNAP_DATA", ""), "sockets", "wifi-ap", "control")
SOCKET_RETRY_COUNT: int = 30
SOCKET_RETRY_HOLDOFF: int = 10

DEFAULT_CONFIG = {
    "disabled": False,
    "debug": False,
    "wifi.interface": "wlan0",
    "wifi.address": "192.168.4.1",
    "wifi.netmask": "255.255.255.0",
    "wifi.interface-mode": "direct",
    "wifi.hostapd-driver": "nl80211",
    "wifi.ssid": "ammp-edge",
    "wifi.security": "wpa2",
    "wifi.security-passphrase": "ammp12345",
    "wifi.channel": "6",
    "wifi.country-code": "US",
    "share.disabled": True,
    "share.network-interface": "eth0",
    "dhcp.range-start": "192.168.4.100",
    "dhcp.range-stop": "192.168.4.200",
    "dhcp.lease-time": "12h",
}


class WifiAPSnapCtl(object):

    def __init__(self, socket_path: str = DEFAULT_SOCKET_PATH) -> None:

        self.socket_path = socket_path
        # Make sure we are able to read status from the wifi-ap API
        logger.info(f"Testing socket connection to {self.socket_path}")
        # If a connection exception is raised here, it will be passed through to the invoking
        # function, which will terminate. This is intentional
        res = self.__socket_get("v1/status")
        logger.info(f"Response from wifi-ap snap API: Status {res.status_code} / {res.text}")

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
            # In order to minimize unnecessary reconfiguration, we first obtain the existing config and
            # compare against it. If they're the same, no action is taken
            res = self.__socket_get("v1/configuration")
            # We check if a config dict is returned, and if our new dict is a subset of that existing config
            if isinstance(res.json().get("result"), dict) and conf_payload.items() <= res.json()["result"].items():
                logger.info("Config is already applied. Not applying")
                return True

            logger.info("Sending new configuration to Wifi AP API")
            self.__socket_post("v1/configuration", conf_payload)
            logger.info(f"Response from wifi-ap snap API: Status {res.status_code} / {res.text}")
            return res.status_code == 200

        except ConnectionError as e:
            logger.error(f"Connection error while making wifi-ap API socket request: {e}")
        except Exception as e:
            logger.exception(f"Exception while making wifi-ap API socket request: {e}")

    def __socket_get(self, path: str):
        retries = 0
        while True:
            try:
                res = requests_unixsocket.get(f"http+unix://{quote(self.socket_path, safe='')}/{path}")
                logger.debug(f"Response from wifi-ap snap API: Status {res.status_code} / {res.text}")
                return res
            except ConnectionError as e:
                logger.error(f"Connection error while doing wifi-ap API socket GET request: {e}")
                if retries < SOCKET_RETRY_COUNT:
                    sleep(SOCKET_RETRY_HOLDOFF)
                    retries += 1
                    logger.info(f"Retrying ({retries}/{SOCKET_RETRY_COUNT})")
                else:
                    raise (e)

    def __socket_post(self, path: str, payload: dict):
        retries = 0
        while True:
            try:
                res = requests_unixsocket.post(f"http+unix://{quote(self.socket_path, safe='')}/{path}", json=payload)
                logger.debug(f"Response from wifi-ap snap API: Status {res.status_code} / {res.text}")
                return res
            except ConnectionError as e:
                logger.error(f"Connection error while doing wifi-ap API socket POST request: {e}")
                if retries < SOCKET_RETRY_COUNT:
                    sleep(SOCKET_RETRY_HOLDOFF)
                    retries += 1
                    logger.info(f"Retrying ({retries}/{SOCKET_RETRY_COUNT})")
                else:
                    raise (e)
