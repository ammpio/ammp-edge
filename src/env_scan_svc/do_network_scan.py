import logging

from node_mgmt.env_scan import NetworkEnv

logger = logging.getLogger(__name__)


def main() -> None:
    net_env = NetworkEnv()
    # TODO: Scan all interfaces (within reason). The below will only scan the default interface.
    net_env.network_scan(nmap_scan_opts=['-sn'])


if __name__ == '__main__':
    main()
