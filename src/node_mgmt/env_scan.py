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
from reader.modbusrtu_reader import Reader as ModbusRTUReader
from reader.modbustcp_reader import Reader as ModbusTCPReader
from reader.sma_speedwire_reader import Reader as SpeedWireReader
from processor import process_reading

logger = logging.getLogger(__name__)

DEFAULT_NMAP_SCAN_OPTS = ['--disable-arp-ping', '-p', '22,80,443,502']
DEFAULT_SERIAL_DEV = '/dev/ttyAMA0'

MODTCP_PORT = 502
SMA_MODTCP_UNIT_IDS = [1, 3]
DSE_MODTCP_UNIT_IDS = [1]
MODTCP_TIMEOUT = 1
MODTCP_UNIT_ID_KEY = 'unit_id'
MODTCP_FIELD_KEY = 'field'
MODTCP_REGISTER_KEY = 'register'
MODTCP_WORDS_KEY = 'words'
MODTCP_DATATYPE_KEY = 'uint32'
MODTCP_SCAN_ITEMS = [
    {
        MODTCP_FIELD_KEY: 'sma_device_class',
        MODTCP_REGISTER_KEY: 30051,
        MODTCP_WORDS_KEY: 2,
        MODTCP_DATATYPE_KEY: 'uint32'
    },
    {
        MODTCP_FIELD_KEY: 'sma_device_type',
        MODTCP_REGISTER_KEY: 30053,
        MODTCP_WORDS_KEY: 2,
        MODTCP_DATATYPE_KEY: 'uint32'
    },
    {
        MODTCP_FIELD_KEY: 'sma_serial',
        MODTCP_REGISTER_KEY: 30057,
        MODTCP_WORDS_KEY: 2,
        MODTCP_DATATYPE_KEY: 'uint32'
    },
]
MODTCP_RESULT_KEY = 'modbustcp'

SERIAL_SCAN_SIGNATURES = [{
    'name': 'Gamicos ultrasonic sensor',
    'slave_id': 1,
    'reading': 'fuel level (distance from sensor (m))',
    'register': 1,
    'words': 2,
    'datatype': 'float',
    'fncode': 3
}, {
    'name': 'IMT irradiation sensor',
    'slave_id': 2,
    'reading': 'irradiance (W/m2)',
    'register': 0,
    'words': 1,
    'datatype': 'uint16',
    'fncode': 4
}, {
    'name': 'APM303 genset controller',
    'slave_id': 5,
    'reading': 'oil pressure (bar)',
    'register': 28,
    'words': 1,
    'datatype': 'int16',
    'fncode': 4
}, {
    'name': 'Cummins PS0600',
    'slave_id': 4,
    'reading': 'genset state',
    'register': 10,
    'words': 1,
    'datatype': 'uint16',
    'fncode': 3
}]

SERIAL_SCAN_BAUD_RATE = 9600

NMAP_ADDR_KEY = 'addr'
NMAP_ADDR_TYPE_KEY = 'addrtype'
NMAP_IP_ADDR_TYPE = 'ipv4'
NMAP_MAC_ADDR_TYPE = 'mac'
NMAP_VENDOR_KEY = 'vendor'
NMAP_PORTS_KEY = 'ports'
NMAP_PORT_KEY = 'port'
NMAP_PORT_STATE_KEY = 'state'
NMAP_PORT_OPEN = 'open'
NMAP_PORTID_KEY = 'portid'

HOST_IP_KEY = 'ipv4'
HOST_MAC_KEY = 'mac'
HOST_VENDOR_KEY = 'vendor'
HOST_PORTS_KEY = 'ports'


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
            f"Initialized network env on {self.default_ifname} with IP {self.default_ip}/{self.default_netmask_bits}")

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
                     save_to_kvs: bool = True) -> dict:
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
                    if a.get(NMAP_ADDR_TYPE_KEY) == NMAP_IP_ADDR_TYPE:
                        this_host[HOST_IP_KEY] = a.get(NMAP_ADDR_KEY)
                    elif a.get(NMAP_ADDR_TYPE_KEY) == NMAP_MAC_ADDR_TYPE:
                        this_host[HOST_MAC_KEY] = a.get(NMAP_ADDR_KEY)
                        if NMAP_VENDOR_KEY in a:
                            this_host[HOST_VENDOR_KEY] = a[NMAP_VENDOR_KEY]

                if h['hostnames']:
                    this_host['hostname'] = h['hostnames']['hostname'][0].get('name')

                if NMAP_PORTS_KEY in h:
                    this_host[HOST_PORTS_KEY] = []
                    for p in h[NMAP_PORTS_KEY].get(NMAP_PORT_KEY, []):
                        if p[NMAP_PORT_STATE_KEY][NMAP_PORT_STATE_KEY] == NMAP_PORT_OPEN:
                            this_host[HOST_PORTS_KEY].append(p[NMAP_PORTID_KEY])

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
            nmap_path = os.path.join(os.getenv('SNAP'), 'bin', 'nmap')
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

    @staticmethod
    def modbus_read(host_vendor, host_ip):
        if 'SMA' in host_vendor:
            unit_id = SMA_MODTCP_UNIT_IDS
        elif 'Deep Sea Electronics' in host_vendor:
            unit_id = DSE_MODTCP_UNIT_IDS
        else:
            return None
        result_for_unit = {
            MODTCP_UNIT_ID_KEY: unit_id,
        }
        try:
            with ModbusTCPReader(
                    host=host_ip,
                    port=MODTCP_PORT,
                    unit_id=unit_id,
                    timeout=MODTCP_TIMEOUT,
            ) as r:
                for rdg in MODTCP_SCAN_ITEMS:
                    val_b = r.read(**rdg)
                    if val_b is None:
                        continue
                    try:
                        value = process_reading(val_b, **rdg)
                    except Exception as e:
                        logger.error(f"Could not process reading: {e}\nval_b={val_b}\nrdg={rdg}")
                        continue
                    result_for_unit[rdg[MODTCP_FIELD_KEY]] = value
                return result_for_unit
        except Exception as e:
            logger.info(f"Error: {e}")

    def modbus_scan(self, hosts: list) -> None:
        if hosts is None:
            return

        for h in hosts:
            if HOST_IP_KEY not in h \
                or MODTCP_PORT not in \
                    [int(p) for p in h.get(HOST_PORTS_KEY, [])]:
                continue
            host_vendor = h[HOST_VENDOR_KEY]
            host_ip = h[HOST_IP_KEY]
            h[MODTCP_RESULT_KEY] = []
            result_for_unit = self.modbus_read(host_vendor, host_ip)
            h[MODTCP_RESULT_KEY].append(result_for_unit)


class SerialEnv():
    def __init__(self, default_serial_dev=None):
        self.serial_devices = self.get_serial_devices()

        if default_serial_dev is not None:
            self.default_serial_dev = default_serial_dev
        elif self.serial_devices and DEFAULT_SERIAL_DEV in self.serial_devices:
            # If the global default device is present, use that
            self.default_serial_dev = DEFAULT_SERIAL_DEV
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

        result = []

        for sig in SERIAL_SCAN_SIGNATURES:
            test = f"Testing slave ID {sig['slave_id']} for {sig['name']} at baud rate {SERIAL_SCAN_BAUD_RATE}"
            with ModbusRTUReader(device, sig['slave_id'], SERIAL_SCAN_BAUD_RATE, timeout=1, debug=True) as r:
                try:
                    response = process_reading(r.read(register=sig['register'], words=sig['words'],
                                                      fncode=sig['fncode']), datatype=sig['datatype'])
                    res = f"Got test response for {sig['reading']} = {response}"
                    if response is not None:
                        res += f" ==> SUCCESS: Device {sig['name']} present as ID {sig['slave_id']} " \
                               f"at baud rate = {SERIAL_SCAN_BAUD_RATE}"
                except Exception as e:
                    res = f"Error: {e}"
                    res += f". {sig['name']} doesn't show up, make sure the configuration is correct:" \
                           f" slave id = {sig['slave_id']}, baud rate = {SERIAL_SCAN_BAUD_RATE}"
            result.append([test, res])
        return result


class EnvScanner(object):
    def __init__(self, ifname=None, serial_dev=None):

        self.net_env = NetworkEnv(default_ifname=ifname)
        self.serial_env = SerialEnv(default_serial_dev=serial_dev)
        self.speedwire_env = SpeedWireReader()

    def do_scan(self):
        network_hosts = self.net_env.network_scan()
        try:
            self.net_env.modbus_scan(network_hosts)
        except Exception:
            logger.exception("Exception while running ModbusTCP scan")
        serial_devices = self.serial_env.serial_scan()
        with self.speedwire_env:
            speedwire_serials = self.speedwire_env.scan_serials()

        scan_result = {
            'time':
            arrow.utcnow().strftime('%Y-%m-%dT%H:%M:%SZ'),
            'network_scan': [{
                'ifname': self.net_env.default_ifname,
                HOST_IP_KEY: self.net_env.default_ip,
                'netmask': self.net_env.default_netmask_bits,
                'hosts': network_hosts
            }],
            'serial_scan': serial_devices,
            'speedwire_serials': speedwire_serials
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
