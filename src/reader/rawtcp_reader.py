import logging
import socket

from .helpers import generate_request, parse_response

logger = logging.getLogger(__name__)

DEFAULT_RECV_BUFFER_SIZE = 1024


class Reader(object):
    def __init__(
                self,
                host: str = None,
                port: str = 502,
                recv_buffer_size: int = DEFAULT_RECV_BUFFER_SIZE,
                timeout: int = 5,
                **kwargs
                ):

        self._host = host
        self._port = port
        self._recv_buffer_size = recv_buffer_size
        self._timeout = timeout

        self._stored_responses = {}

    def __enter__(self):

        self._conn = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self._conn.settimeout(self._timeout)
        try:
            self._conn.connect((self._tcp.host, self._tcp.port))
        except Exception:
            logger.error('Exception while attempting to create TCP connection:')
            raise

        return self

    def __exit__(self, type, value, traceback):
        if not hasattr(self, '_conn'):
            return

        try:
            self._conn.close()
        except Exception:
            logger.warning("Could not close TCP connection", exc_info=True)

    def read(self, schema, **rdg):

        request = generate_request(schema['request'], **rdg)

        if request in self._stored_responses:
            response = self._stored_responses[request]
        else:
            try:
                logger.debug(f"Writing {repr(request)} to TCP port")
                self._conn.send(request)
                # TODO: We may need to do something more intelligent here in cases where the full reponse
                # doesn't get sent in one go
                response = self._conn.recv(self._recv_buffer_size)
                logger.debug(f"Received {repr(response)} from serial port")

                if response == b'':
                    logger.warn("No response received from device")
                    return

            except Exception:
                logger.error(f"Exception while reading response to query {repr(request)}")
                raise

        # Save response in case other readings rely on the same query
        self._stored_responses[request] = response

        try:
            # Parse the response to obtain the actual value
            val_b = parse_response(response, schema['response'], **rdg)
        except Exception:
            logger.error(
                f"Exception while processing value from response {repr(response)}"
                )
            raise

        return val_b
