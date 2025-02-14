import logging
import os
import socket
import struct

from pyModbusTCP.client import ModbusClient

logger = logging.getLogger(__name__)


class Reader(object):
    def __init__(
        self, host, port=502, unit_id=1, register_offset=0, timeout=10, conn_check=False, conn_retry=10, **kwargs
    ):

        self._host = host
        self._port = port
        self._unit_id = unit_id
        self._register_offset = register_offset
        self._timeout = timeout
        self._conn_check = conn_check
        self._conn_retry = conn_retry

    def __enter__(self):
        # Create a ModbusTCP connection to be used for all our requests
        try:
            self._conn = ModbusClient(
                host=self._host,
                port=self._port,
                unit_id=self._unit_id,
                timeout=self._timeout,
                auto_open=False,
                auto_close=False,
            )
        except Exception:
            logger.exception("Attempting to create ModbusTCP client raised exception:")
            raise

        try:
            # Make sure we have an open connection to device
            if self.__open_connection(self._conn_retry):
                logger.debug(f"Opened ModbusTCP connection to {self._host}:{self._port}/{self._unit_id}")
            else:
                logger.error(f"Unable to open ModbusTCP connection to {self._host}:{self._port}/{self._unit_id}")
                return None
        except Exception:
            logger.error("Exception while attempting to open ModbusTCP connection:")
            raise

        return self

    def __exit__(self, type, value, traceback):
        if not hasattr(self, "_conn"):
            return

        try:
            self._conn.close()
        except Exception:
            logger.warning("Exception while trying to close ModbusTCP connection", exc_info=True)

    def __open_connection(self, retries_left=0):
        # Make sure we have an open connection to server
        if not self._conn.is_open:
            if self._conn_check:
                # Do a quick ping check
                r = os.system("ping -c 1 %s" % self._host)
                if r == 0:
                    logger.debug(f"Host {self._host} appears to be up")
                else:
                    logger.error(f"Unable to ping host {self._host}")

                # Do a quick TCP socket open check
                sock = socket.socket()
                try:
                    sock.connect((self._host, self._port))
                    logger.debug(f"Successfully opened test connection to {self._host}:{self._port}")
                except Exception:
                    logger.exception(f"Cannot open ModbusTCP socket on {self._host}:{self._port}")
                finally:
                    sock.close()

            self._conn.open()

        if self._conn.is_open:
            return True
        elif retries_left > 0:
            logger.warn(f"Connection attempt to {self._host}:{self._port}/{self._unit_id} failed. Retrying")
            return self.__open_connection(retries_left - 1)
        else:
            return False

    def read(self, register, words, fncode=3, **kwargs):

        # Make sure connection is open
        if not self.__open_connection():
            logger.warning(f"Cannot open ModbusTCP connection to {self._host}:{self._port}/{self._unit_id}")
            return

        try:
            # If register is a string, assume that it's hex and convert to integer
            # (having a "0x" prefix is acceptable but optional)
            if type(register) is str:
                register = int(register, 16)

            register_to_read = self._register_offset + register
            logger.debug(f"Reading {register_to_read=} with {fncode=}")

            if fncode == 3:  # Default is fncode 3
                val_i = self._conn.read_holding_registers(register_to_read, words)
            elif fncode == 4:
                val_i = self._conn.read_input_registers(register_to_read, words)
            else:
                logger.warn(f"Unrecognized Modbus function code '{fncode}'")
                val_i = None
        except Exception:
            logger.error(f"Exception while reading register {register_to_read}")
            raise

        if val_i is None:
            return

        try:
            # The pyModbusTCP library helpfully converts the binary result to a list of integers, so
            # it's best to first convert it back to binary. We assume big-endian order - unless 'order'
            # parameter is set to 'lsr' = least significant register, in which case we reverse the order
            # of the registers.
            if kwargs.get("order") == "lsr":
                val_i.reverse()

            val_b = struct.pack(">%sH" % len(val_i), *val_i)

        except Exception:
            logger.error(f"Exception while processing register {register}")
            raise

        return val_b
