import logging
import socket
import struct

from .helpers.sma_speedwire_parser import parse_datagram_response

logger = logging.getLogger(__name__)

DEFAULT_RECV_BUFFER_SIZE = 10240
MULTICAST_GROUP = '239.12.255.254'
MULTICAST_PORT = 9522
DATA_REQUEST_STR = '4sl'
MAX_RESPONSES = 5


class Reader(object):
    def __init__(
        self,
        serial: int,
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
        self._stored_response = None

    def __enter__(self):
        self._conn = socket.socket(
            socket.AF_INET, socket.SOCK_DGRAM, socket.IPPROTO_UDP)
        self._conn.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self._conn.settimeout(self._timeout)
        try:
            self._conn.bind(('', self._port))
        except Exception:
            logger.error(
                'Exception while attempting to create UDP connection:')
            raise

        return self

    def __exit__(self, type, value, traceback):
        if not hasattr(self, '_conn'):
            return

        try:
            self._conn.close()
        except Exception:
            logger.warning("Could not close UDP connection", exc_info=True)

    def __broadcast_request(self):
        request = struct.pack(DATA_REQUEST_STR,
                              socket.inet_aton(self._group),
                              socket.INADDR_ANY
                              )
        logger.debug(f"Writing {repr(request)} to multicast")
        self._conn.setsockopt(
            socket.IPPROTO_IP, socket.IP_ADD_MEMBERSHIP, request)

    def __get_response(self):
        try:
            response = self._conn.recv(self._recv_buffer_size)
        except socket.timeout:
            logger.warning("Timed out while waiting for multicast response")
            return None

        logger.debug(f"Received {repr(response)} from multicast")
        if response == b'':
            logger.warning("No response received from multicast")
            return None
        return response

    def read(self, obis_channel: int, obis_type: int, **rdg) -> bytes:
        if self._stored_response is None:
            # If there is no stored response, put out a request
            self.__broadcast_request()

            # Collect responses and try to match them to the desired serial
            for _ in range(self._max_responses):
                response = self.__get_response()
                if response is None:
                    break
                serial_number, values = parse_datagram_response(response)
                if serial_number == self._serial:
                    self._stored_response = response
                    break

        return self.__get_value(self._stored_response, obis_channel, obis_type)

    @staticmethod
    def __get_value(values: dict, obis_channel: int, obis_type: int) -> bytes:
        if not isinstance(values, dict):
            return None
        return values.get(obis_channel, {}).get(obis_type)
