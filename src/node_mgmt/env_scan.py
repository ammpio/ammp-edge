import logging

import socket
import arrow
import subprocess
import os
from psutil import net_if_addrs
import serial.tools.list_ports
import xmltodict
from collections import defaultdict

from kvstore import KVStore

logger = logging.getLogger(__name__)

DEFAULT_NMAP_SCAN_OPTS = ['-p', '22,80,443,502']
DEFAULT_SERIAL_DEV = '/dev/ttyAMA0'


class NetworkEnv():

    def __init__(self, default_ifname=None, default_ip=None, default_netmask_bits=None):

        # Define the socket address families that may contain MAC addresses
        self.__mac_socket_family = []
        if hasattr(socket, 'AF_PACKET'):
            self.__mac_socket_family.append(socket.AF_PACKET)
        if hasattr(socket, 'AF_LINK'):
            self.__mac_socket_family.append(socket.AF_LINK)

        self.interfaces = self.get_interfaces()

        if default_ifname is not None:
            self.default_ifname = default_ifname
            self.default_ip = default_ip or \
                self.interfaces.get(self.default_ifname, {}).get('ip')
            self.default_netmask_bits = default_netmask_bits or \
                self.interfaces.get(self.default_ifname, {}).get('netmask_bits')

            # If we've obtained an IP and netmask for the selected interface, then we can stop here
            if self.default_ip is not None and self.default_netmask_bits is not None:
                return

        # Otherwise we get the IP corresponding to the default route, and the associated interface and netmask
        # Note that in this case the provided interface name will be overridden
        self.default_ip = self.get_default_ip()
        self.default_ifname = self.get_interface_from_ip(self.default_ip)
        self.default_netmask_bits = self.interfaces[self.default_ifname].get('netmask_bits')
        logger.info(
            f"Initialized network env on {self.default_ifname} with IP {self.default_ip}/{self.default_netmask_bits}"
            )

    def get_interfaces(self):

        all_interfaces = net_if_addrs()
        interfaces = defaultdict(dict)
        for if_name, if_addrs in all_interfaces.items():
            # Skip loopback interface(s)
            if if_name[:2] == 'lo':
                continue

            # Note: in the below, the last available address will be used for each interface.
            # This should not be a problem, unless an interface has multiple addresses
            # of a single type, and all of them need to be kept
            for addr in if_addrs:
                if addr.family == socket.AF_INET:
                    # It's an IPv4 address
                    interfaces[if_name]['ip'] = addr.address
                    interfaces[if_name]['netmask'] = addr.netmask
                    interfaces[if_name]['netmask_bits'] = self.get_netmask_bits_from_string(addr.netmask)
                elif addr.family == socket.AF_INET6:
                    # It's an IPv6 address
                    interfaces[if_name]['ipv6'] = addr.address
                elif addr.family in self.__mac_socket_family:
                    # It's a MAC address
                    interfaces[if_name]['mac'] = addr.address

        return interfaces

    @staticmethod
    def get_default_ip():
        with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as s:
            # Note that no actual connection is made here - we're just opening a socket
            # in order to identify the local source IP that's used to reach a public IP
            s.connect(('1.1.1.1', 1))
            ip_addr = s.getsockname()[0]
        return ip_addr

    def get_interface_from_ip(self, ip):
        # Get the first interface with an address matching the requested one
        if_name = [name for name, addrs in self.interfaces.items() if addrs.get('ip') == ip][0]
        return if_name

    @staticmethod
    def get_netmask_bits_from_string(netmask_str):
        if netmask_str is None:
            return None

        try:
            netmask_bits = sum(bin(int(x)).count('1') for x in netmask_str.split('.'))
        except Exception:
            return None

        return netmask_bits

    def network_scan(self,
                     this_ip_addr: str = None,
                     netmask_bits: int = None,
                     nmap_scan_opts: list = DEFAULT_NMAP_SCAN_OPTS,
                     save_to_kvs: bool = True
                     ) -> dict:
        if not this_ip_addr:
            this_ip_addr = self.default_ip
        if not netmask_bits:
            netmask_bits = self.default_netmask_bits

        net_to_scan = this_ip_addr + '/' + str(netmask_bits)

        nmap_args = [net_to_scan] + nmap_scan_opts
        logger.info(f"Running nmap scan on {' '.join(nmap_args)}")
        scan_res = self.run_nmap(nmap_args)
        logger.debug(f"nmap scan result: {scan_res}")

        if scan_res is None:
            return

        hosts = []
        for h in scan_res['nmaprun'].get('host', []):
            if h['status']['state'] == 'up':
                this_host = {}

                for a in h['address']:
                    if a.get('addrtype') == 'ipv4':
                        this_host['ipv4'] = a.get('addr')
                    elif a.get('addrtype') == 'mac':
                        this_host['mac'] = a.get('addr')
                        if 'vendor' in a:
                            this_host['vendor'] = a['vendor']

                if h['hostnames']:
                    this_host['hostname'] = h['hostnames']['hostname'][0].get('name')

                if 'ports' in h:
                    this_host['ports'] = []
                    for p in h['ports'].get('port', []):
                        if p['state']['state'] == 'open':
                            this_host['ports'].append(p['portid'])

                hosts.append(this_host)

        # The default behavior is to save the results to the key-value store,
        # for potential use by other processes
        if save_to_kvs:
            try:
                kvs = KVStore()
                for h in hosts:
                    if h.get('mac'):  # Skip any hosts without MAC addresses
                        this_mac = h['mac'].lower()
                        kvs.set(f"env:net:mac:{this_mac}", h)
            except Exception as e:
                logger.error(f"Cannot save scan results to key-value store: {e}")

        return hosts

    @staticmethod
    def run_nmap(args):
        """
        args is a list of arguments
        """

        if not isinstance(args, list):
            args = [args]

        if os.getenv('SNAP'):
            nmap_path = os.path.join(os.getenv('SNAP'), 'usr', 'bin', 'nmap')
        else:
            nmap_path = 'nmap'

        cmd = [nmap_path, '-oX', '-'] + args

        try:
            res = subprocess.run(cmd, stdout=subprocess.PIPE)
        except FileNotFoundError:
            logger.error(f"Executable {cmd[0]} not found. Ensure that nmap is installed")
            return None

        res_str = res.stdout.decode('utf-8').rstrip()

        if not res_str:
            return None
        else:
            try:
                return xmltodict.parse(res_str, attr_prefix='', force_list=('host', 'address', 'hostname', 'port'))
            except Exception:
                logger.error(f"Nmap did not return valid XML: {res_str}")
                return None


class SerialEnv():

    def __init__(self, default_serial_dev=None):
        self.serial_devices = self.get_serial_devices()

        if default_serial_dev is not None:
            self.default_serial_dev = default_serial_dev
        elif self.serial_devices:
            # Only do this if devices are actually present
            if DEFAULT_SERIAL_DEV in self.serial_devices:
                # If the global default device is present, use that
                self.default_serial_dev = DEFAULT_SERIAL_DEV
            else:
                # Otherwise use the first available device
                self.default_serial_dev = self.serial_devices[0]
        else:
            self.default_serial_dev = None

    @staticmethod
    def get_serial_devices():
        comports = serial.tools.list_ports.comports()
        devices = [c.device for c in comports]
        return devices

    def serial_scan(self, device=None):

        if not device:
            if self.default_serial_dev:
                device = self.default_serial_dev
            else:
                return []

        from reader.modbusrtu_reader import Reader

        SIGNATURES = [
            {
                'name': 'Gamicos ultrasonic sensor',
                'readings': [
                    {
                        'register': 1,
                        'words': 2,
                        'fncode': 3
                    }
                ]
            },
            {
                'name': 'IMT irradiation sensor',
                'readings': [
                    {
                        'register': 0,
                        'words': 1,
                        'fncode': 4
                    }
                ]
            }
        ]

        BAUD_RATES = [9600, 2400]
        SLAVE_IDS = [1, 2]

        result = []

        for br in BAUD_RATES:
            for slave in SLAVE_IDS:
                for sig in SIGNATURES:
                    test = f"Testing slave ID {slave} for '{sig['name']}' at baud rate {br}"
                    with Reader(device, slave, br, timeout=1, debug=True) as r:
                        success = True
                        for rdg in sig['readings']:
                            try:
                                response = r.read(**rdg)
                                res = f"Got response {response}"
                                if response is None:
                                    success = False
                            except Exception as e:
                                res = f"Error: {e}"
                                success = False
                        if success:
                            res = res + f"==> SUCCESS: Device '{sig['name']}' present as ID {slave} at baud rate {br}"

                    result.append([test, res])

        return result


class EnvScanner(object):

    def __init__(self, ifname=None, serial_dev=None):

        self.net_env = NetworkEnv(default_ifname=ifname)
        self.serial_env = SerialEnv(default_serial_dev=serial_dev)

    def do_scan(self):
        network_hosts = self.net_env.network_scan()
        serial_devices = self.serial_env.serial_scan()

        scan_result = {
            'time': arrow.utcnow().strftime('%Y-%m-%dT%H:%M:%SZ'),
            'network_scan': [
                {
                    'ifname': self.net_env.default_ifname,
                    'ipv4': self.net_env.default_ip,
                    'netmask': self.net_env.default_netmask_bits,
                    'hosts': network_hosts
                }
            ],
            'serial_scan': serial_devices
        }

        return scan_result


def get_ssh_fingerprint():
    if os.getenv('SNAP'):
        cmd = os.path.join(os.getenv('SNAP'), 'bin', 'get_ssh_fingerprint.sh')
    else:
        cmd = 'get_ssh_fingerprint.sh'

    try:
        res = subprocess.run([cmd], stdout=subprocess.PIPE)
    except FileNotFoundError:
        logger.error(f"Executable {cmd} not found. Ensure that it is available")
        return None

    res_str = res.stdout.decode('utf-8').rstrip()

    return res_str


def serial_scan():
    SE = SerialEnv()
    res = SE.serial_scan()
    print('\n'.join(res))
