import logging
from kvstore import KVStore, keys
from .wifi_ap_snap_ctl import WifiAPSnapCtl
from time import sleep
import sys
import os
from dotenv import load_dotenv

# Set up logging
logging.basicConfig(
    format='%(name)s [%(levelname)s] %(message)s', level='INFO')
logger = logging.getLogger(__name__)

# Load additional environment variables from env file (set by snap configuration)
dotenv_path = os.path.join(os.environ.get('SNAP_COMMON', '.'), '.env')
load_dotenv(dotenv_path)

if os.environ.get('LOGGING_LEVEL'):
    try:
        logging.getLogger().setLevel(os.environ['LOGGING_LEVEL'])
    except Exception:
        logger.warn(
            f"Failed to set log level to {os.environ['LOGGING_LEVEL']}", exc_info=True)


def initialize(wifi_ap: WifiAPSnapCtl, kvs: KVStore) -> bool:
    wifi_ap_cfg = kvs.get(keys.WIFI_AP_CONFIG)
    return wifi_ap.configure(wifi_ap_cfg)


def monitor_and_update(wifi_ap: WifiAPSnapCtl, kvs: KVStore) -> None:
    while True:
        try:
            wifi_ap_cfg = kvs.waitfor(keys.WIFI_AP_CONFIG)
            wifi_ap.configure(wifi_ap_cfg)
        except Exception as e:
            logger.info(
                f"Exception while monitoring for new config: {type(e).__name__}: {e}")
            sleep(60)


def main() -> None:
    kvs = KVStore()

    try:
        wifi_ap = WifiAPSnapCtl()
        kvs.set(keys.WIFI_AP_AVAILABLE, True)
    except Exception as e:
        logger.warn(
            f"Exception while setting up Wifi access point control: {type(e).__name__}: {e}")
        kvs.set(keys.WIFI_AP_AVAILABLE, False)
        sys.exit(1)

    initialize(wifi_ap, kvs)
    monitor_and_update(wifi_ap, kvs)


if __name__ == '__main__':
    main()
