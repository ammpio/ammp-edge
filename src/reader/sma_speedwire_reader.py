import logging
import socket
import struct

from .helpers.sma_speedwire_parser import parse_datagram

logger = logging.getLogger(__name__)

DEFAULT_RECV_BUFFER_SIZE = 10240
MULTICAST_GROUP = '239.12.255.254'
MULTICAST_PORT = 9522
MAX_RESPONSES = 5


class Reader(object):
    def __init__(
        self,
        serial: int = None,
        group: str = MULTICAST_GROUP,
        port: int = MULTICAST_PORT,
        recv_buffer_size: int = DEFAULT_RECV_BUFFER_SIZE,
        max_responses: int = MAX_RESPONSES,
        timeout: int = 5,
        **kwargs
    ):
        self._group = group
        self._port = port
        self._recv_buffer_size = recv_buffer_size
        self._max_responses = max_responses
        self._timeout = timeout

        self._serial = serial
        self._stored_values = None

    def __enter__(self):
        self._conn = socket.socket(
            socket.AF_INET, socket.SOCK_DGRAM, socket.IPPROTO_UDP)
        self._conn.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self._conn.settimeout(self._timeout)
        self._conn.bind(('', self._port))
        mreq = struct.pack('4sl',
                           socket.inet_aton(self._group),
                           socket.INADDR_ANY
                           )
        logger.debug(f"Joining multicast group {self._group}")
        self._conn.setsockopt(
            socket.IPPROTO_IP, socket.IP_ADD_MEMBERSHIP, mreq)

        return self

    def __exit__(self, type, value, traceback):
        if not hasattr(self, '_conn'):
            return

        try:
            self._conn.close()
        except Exception:
            logger.warning("Could not close UDP connection", exc_info=True)

    def __get_datagram(self):
        try:
            datagram = self._conn.recv(self._recv_buffer_size)
        except socket.timeout:
            logger.warning("Timed out while waiting for multicast datagram")
            return None

        logger.debug(f"Received {repr(datagram)} from multicast")
        if datagram == b'':
            logger.warning("Empty datagram received from multicast")
            return None
        return datagram

    def read(self, obis_channel: int, obis_type: int, **rdg) -> bytes:
        if self._stored_values is None:
            # If there is no stored response, collect responses and try to
            # find the one that matched the desired serial
            for _ in range(self._max_responses):
                datagram = self.__get_datagram()
                if datagram is None:
                    break
                serial_number, values = parse_datagram(datagram)
                if serial_number == self._serial:
                    self._stored_values = values
                    break

        return self.__get_value(self._stored_values, obis_channel, obis_type)

    def scan_serials(self) -> list:
        serials = []
        for _ in range(self._max_responses):
            datagram = self.__get_datagram()
            if datagram is None:
                break
            serial_number, _ = parse_datagram(datagram)
            if serial_number not in serials:
                serials.append(serial_number)
        return serials

    @staticmethod
    def __get_value(values: dict, obis_channel: int, obis_type: int) -> bytes:
        if not isinstance(values, dict):
            return None
        return values.get(obis_channel, {}).get(obis_type)
