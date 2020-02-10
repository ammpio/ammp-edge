import logging
from kvstore import KVStore
from .wifi_ap_snap_ctl import WifiAPSnapCtl
from time import sleep

# Set up logging
logging.basicConfig(format='%(threadName)s:%(name)s [%(levelname)s] %(message)s', level='INFO')
logger = logging.getLogger(__name__)

KVS_CONFIG_KEY = 'node:wifi_ap_config'


def initialize(wifi_ap: WifiAPSnapCtl, kvs: KVStore) -> bool:
    wifi_ap_cfg = kvs.get(KVS_CONFIG_KEY)
    return wifi_ap.configure(wifi_ap_cfg)


def monitor_and_update(wifi_ap: WifiAPSnapCtl, kvs: KVStore) -> None:
    while True:
        try:
            wifi_ap_cfg = kvs.waitfor(KVS_CONFIG_KEY)
            wifi_ap.configure(wifi_ap_cfg)
        except Exception as e:
            logger.info(f"Exception while monitoring for new config: {type(e).__name__}: {e}")
            sleep(60)


def main() -> None:
    wifi_ap = WifiAPSnapCtl()
    kvs = KVStore()

    initialize(wifi_ap, kvs)
    monitor_and_update(wifi_ap, kvs)


if __name__ == '__main__':
    main()
