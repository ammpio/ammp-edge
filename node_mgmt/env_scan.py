import logging
logger = logging.getLogger(__name__)

import socket
from fcntl import ioctl
import struct
import arrow
import subprocess
import os
import xmltodict
import json


DEFAULT_IFNAME = 'eth0'
DEFAULT_PORTS = '22,80,443,502'
DEFAULT_SERIAL_DEV = '/dev/ttyAMA0'

SIOCGIFADDR = 0x8915
SIOCGIFNETMASK = 0x891b


class EnvScanner(object):

    def __init__(self, ifname=DEFAULT_IFNAME, ip_addr=None, netmask_bits=None, serial_dev=DEFAULT_SERIAL_DEV):

        self.ifname = ifname
        self.serial_dev = serial_dev
        if not ip_addr: ip_addr = self.get_interface_ip()
        self.ip_addr = ip_addr
        if not netmask_bits: netmask_bits = self.get_interface_netmask()
        self.netmask_bits = netmask_bits


    def do_scan(self):
        network_hosts = self.network_scan()
        serial_devices = self.serial_scan()

        scan_result = {
            'time': arrow.utcnow().strftime('%Y-%m-%dT%H:%M:%SZ'),
            'network_scan': [
                {
                    'ifname': self.ifname,
                    'ipv4': self.ip_addr,
                    'netmask': self.netmask_bits,
                    'hosts': network_hosts
                }
            ],
            'serial_scan': serial_devices
        }

        return scan_result


    def network_scan(self, this_ip_addr=None, netmask_bits=None):

        if not this_ip_addr: this_ip_addr = self.ip_addr
        if not netmask_bits: netmask_bits = self.netmask_bits

        net_to_scan = this_ip_addr + '/' + str(netmask_bits)

        scan_res = self.run_nmap([net_to_scan, '-p', DEFAULT_PORTS])

        if scan_res == None: return

        hosts = []
        for h in scan_res['nmaprun'].get('host', []):
            if h['status']['state'] == 'up':
                this_host = {'ports': []}

                for a in h['address']:
                    if a.get('addrtype') == 'ipv4':
                        this_host['ipv4'] = a.get('addr')
                    elif a.get('addrtype') == 'mac':
                        this_host['mac'] = a.get('addr')
                        if 'vendor' in a: this_host['vendor'] = a['vendor']

                if h['hostnames']:
                    this_host['hostname'] = h['hostnames']['hostname'][0].get('name')

                for p in h['ports']['port']:
                    if p['state']['state'] == 'open':
                        this_host['ports'].append(p['portid'])
                
                hosts.append(this_host)

        return hosts


    @classmethod
    def run_nmap(cls, args):
        """
        args is a list of arguments
        """

        if not isinstance(args, list): args = [args]

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
            except:
                logger.error(f"Nmap did not return valid XML: {res_str}")
                return None


    def get_interface_ip(self):
        with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as s:
            info = ioctl(s.fileno(), SIOCGIFADDR, struct.pack('256s', self.ifname.encode('utf-8')))
            ip_addr = socket.inet_ntoa(info[20:24])

        return ip_addr


    def get_interface_netmask(self):
        with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as s:
            info = ioctl(s.fileno(), SIOCGIFNETMASK, struct.pack('256s', self.ifname.encode('utf-8')))
            netmask = info[20:24]
            netmask_bits = bin(int.from_bytes(netmask, byteorder='big')).count('1')

        return netmask_bits


    def get_default_ip(self):
        with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as s:
            # Note that no actual connection is made here - we're just opening a socket
            s.connect(('1.1.1.1', 1))
            ip_addr = s.getsockname()[0]

        return ip_addr


    def serial_scan(self, device=None):

        if not device: device = self.serial_dev

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

        BAUD_RATES = [2400, 9600]
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
                                if response == None:
                                    success = False
                            except Exception as e:
                                res = f"Error: {e}"
                                success = False
                        if success:
                            res = res + f"==> SUCCESS: Device '{sig['name']}' present as ID {slave} at baud rate {br}"

                    result.append([test, res])

        return result

def serial_scan():
    ES = EnvScanner()
    res = ES.serial_scan()
    print('\n'.join(res))