import logging
import os
import sys
from time import sleep

from dotenv import load_dotenv

from kvstore import KVStore

from .wifi_ap_snap_ctl import WifiAPSnapCtl

# Set up logging
logging.basicConfig(format="%(name)s [%(levelname)s] %(message)s", level="INFO")
logger = logging.getLogger(__name__)

# Load additional environment variables from env file (set by snap configuration)
dotenv_path = os.path.join(os.environ.get("SNAP_COMMON", "."), ".env")
load_dotenv(dotenv_path)

if os.environ.get("LOG_LEVEL"):
    try:
        logging.getLogger().setLevel(os.environ["LOG_LEVEL"])
    except Exception:
        logger.warn(f"Failed to set log level to {os.environ['LOG_LEVEL']}", exc_info=True)


KVS_CONFIG_KEY = "wifi_ap_config"
KVS_AVAILABLE_KEY = "wifi_ap_available"


def initialize(wifi_ap: WifiAPSnapCtl, kvs: KVStore) -> bool:
    wifi_ap_cfg = kvs.get(KVS_CONFIG_KEY)
    return wifi_ap.configure(wifi_ap_cfg)


def monitor_and_update(wifi_ap: WifiAPSnapCtl, kvs: KVStore) -> None:
    wifi_ap_cfg = kvs.get(KVS_CONFIG_KEY)
    while True:
        try:
            new_wifi_ap_cfg = kvs.get(KVS_CONFIG_KEY)
            if new_wifi_ap_cfg != wifi_ap_cfg:
                wifi_ap_cfg = new_wifi_ap_cfg
                wifi_ap.configure(wifi_ap_cfg)
        except Exception as e:
            logger.info(f"Exception while monitoring for new config: {type(e).__name__}: {e}")
        sleep(60)


def main() -> None:
    kvs = KVStore()

    try:
        wifi_ap = WifiAPSnapCtl()
        kvs.set(KVS_AVAILABLE_KEY, True)
    except Exception as e:
        logger.warn(f"Exception while setting up Wifi access point control: {type(e).__name__}: {e}")
        kvs.set(KVS_AVAILABLE_KEY, False)
        sys.exit(1)

    initialize(wifi_ap, kvs)
    monitor_and_update(wifi_ap, kvs)


if __name__ == "__main__":
    main()
