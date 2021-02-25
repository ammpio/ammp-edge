import logging
import socket
import struct

from .helpers import parse_datagram_response

logger = logging.getLogger(__name__)

DEFAULT_RECV_BUFFER_SIZE = 1024
MULTICAST_GRP = "239.12.255.254"
MULTICAST_PORT = 9522


class Reader(object):
    def __init__(
                self,
                group: str = MULTICAST_GRP,
                port: int = MULTICAST_PORT,
                recv_buffer_size: int = DEFAULT_RECV_BUFFER_SIZE,
                timeout: int = 5,
                **kwargs
                ):

        self._group= group
        self._port = port
        self._recv_buffer_size = recv_buffer_size
        self._timeout = timeout

    def __enter__(self):

        self._conn = socket.socket(socket.AF_INET, socket.SOCK_DGRAM, socket.IPPROTO_UDP)
        self._conn.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self._conn.settimeout(self._timeout)
        try:
            self._conn.bind(("", MULTICAST_PORT))
        except Exception:
            logger.error('Exception while attempting to create UDP connection:')
            raise

        return self

    def __exit__(self, type, value, traceback):
        if not hasattr(self, '_conn'):
            return

        try:
            self._conn.close()
        except Exception:
            logger.warning("Could not close UDP connection", exc_info=True)

    def read(self, schema, **rdg):

        request = struct.pack("4sl", socket.inet_aton(MULTICAST_GRP), socket.INADDR_ANY)

        try:
            logger.debug(f"Writing {repr(request)} to UDP port")
            self._conn.setsockopt(socket.IPPROTO_IP, socket.IP_ADD_MEMBERSHIP, request)
            response = self._conn.recv(self._recv_buffer_size)
            logger.debug(f"Received {repr(response)} from serial port")

            if response == b'':
                logger.warning("No response received from device")
                return

        except Exception:
            logger.error(f"Exception while reading response to query {repr(request)}")
            raise

        try:
            # Parse the response to obtain the actual values
            em_data = parse_datagram_response(response)
        except Exception:
            logger.error(f"Exception while processing value from response {repr(response)}")
            raise

        return em_data
