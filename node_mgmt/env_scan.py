import logging
logger = logging.getLogger(__name__)

import socket
from fcntl import ioctl
#from scapy.all import Ether, ARP, srp
#from ipaddress import ip_interface

import struct
import arrow

import subprocess
import os
import xmltodict
import json

import threading, queue


DEFAULT_IFNAME = 'eth0'
DEFAULT_PORTS = '22,80,443,502'

SIOCGIFADDR = 0x8915
SIOCGIFNETMASK = 0x891b


class EnvScanner(object):

    def __init__(self, ifname=DEFAULT_IFNAME, ip_addr=None, netmask_bits=None):

        self.ifname = ifname
        if not ip_addr: ip_addr = self.get_interface_ip()
        self.ip_addr = ip_addr
        if not netmask_bits: netmask_bits = self.get_interface_netmask()
        self.netmask_bits = netmask_bits
        # self.scan_q = queue.Queue()
        # self.scan_in_progress = threading.Lock()


    def do_scan(self):
        start_time = arrow.utcnow().strftime('%Y-%m-%dT%H:%M:%SZ'),

        network_hosts = self.network_scan()

        scan_result = {
            'start_time': start_time,
            'end_time': arrow.utcnow().strftime('%Y-%m-%dT%H:%M:%SZ'),
            'network_scan': [
                {
                    'ifname': self.ifname,
                    'ipv4': self.ip_addr,
                    'netmask': self.netmask_bits,
                    'hosts': network_hosts
                }
            ]
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
            nmap_path = os.path.join(os.getenv('SNAP'), 'bin', 'nmap')
        else:
            nmap_path = 'nmap'

        cmd = [nmap_path, '-oX', '-'] + args
        
        try:
            res = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
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


    # def scapy_network_scan(self, this_ip_addr=None, netmask_bits=None):

    #     if not this_ip_addr: this_ip_addr = self.ip_addr
    #     if not netmask_bits: netmask_bits = self.netmask_bits

    #     hosts = []
    #     hosts_to_scan = ip_interface(
    #                         this_ip_addr + '/' + str(netmask_bits)
    #                             ).network.hosts()

    #     jobs = []

    #     for ip_addr in hosts_to_scan:
    #         addr = str(ip_addr)
    #         logger.info(f"Scanning ip_addr")
    #         ip_scan_thread = threading.Thread(
    #                 target=self.get_mac_of_ip,
    #                 name='IP-scan-' + addr,
    #                 args=(addr,),
    #                 daemon=True
    #                 )
            
    #         jobs.append(ip_scan_thread)

    #     self.scan_in_progress.acquire()

    #     for j in jobs: j.start()

    #     for j in jobs: j.join(timeout=5)

    #     # Get the results for each device and append them to the readout structure
    #     for j in jobs:
    #         try:
    #             hosts.extend(self.scan_q.get(block=False))
    #         except queue.Empty:
    #             logger.warning('Not all scan processes completed')

    #     self.scan_in_progress.release()

    #     return hosts


    # def get_mac_of_ip(self, ip_addr):
    #     hosts = []
    #     pkt = Ether(dst="ff:ff:ff:ff:ff:ff") / ARP(pdst=ip_addr)
    #     answered, _ = srp(pkt, iface=self.ifname, timeout=5, verbose=False)
    #     for _, recv in answered:
    #         if recv:
    #             hosts.append((recv[ARP].psrc, recv[Ether].src))

    #     self.scan_q.put(hosts)


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

