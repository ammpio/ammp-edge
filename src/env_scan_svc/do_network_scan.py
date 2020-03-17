import logging
from time import sleep

from node_mgmt.env_scan import NetworkEnv

logger = logging.getLogger(__name__)

INIT_MAX_RETRIES = 12
INIT_RETRY_HOLDOFF = 5


def initialize_network(retries_left: int = INIT_MAX_RETRIES) -> NetworkEnv:
    net_env = NetworkEnv()

    if net_env and not net_env.default_ip.startswith('169.254'):
        return net_env
    else:
        logger.warn(f"Default interface {net_env.default_ifname} has automatic private IP {net_env.default_ip}")
        if retries_left > 0:
            logger.info(f"Will retry scan in {INIT_RETRY_HOLDOFF} seconds ({retries_left} retries left)")
            sleep(INIT_RETRY_HOLDOFF)
            return initialize_network(retries_left-1)
        else:
            logger.error("No more retries left")
            return None


def main() -> None:
    net_env = initialize_network()

    if not net_env:
        return

    # TODO: Scan all interfaces (within reason). The below will only scan the default interface.
    res = net_env.network_scan(nmap_scan_opts=['-sn', '-n'])
    logger.info(f"Scan result: {res}")


if __name__ == '__main__':
    main()
