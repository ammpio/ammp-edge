import logging
logger = logging.getLogger(__name__)

#from reader.pyModbusTCP_alt import ModbusClient_alt
from pyModbusTCP.client import ModbusClient

import os, socket
import struct

class Reader(object):
    def __init__(self, host, port=502, unit_id=1, timeout=10, conn_check=False, conn_retry=10, debug=False, **kwargs):

        self._host = host
        self._port = port
        self._unit_id = unit_id
        self._timeout = timeout
        self._conn_check = conn_check
        self._conn_retry = conn_retry
        self._debug = debug

    def __enter__(self):
        # Create a ModbusTCP connection to be used for all our requests
        try:
            self._conn = ModbusClient(
                host=self._host,
                port=self._port,
                unit_id=self._unit_id,
                timeout=self._timeout,
                debug=self._debug,
                auto_open=False,
                auto_close=False
            )
        except:
            logger.exception("Attempting to create ModbusTCP client raised exception:")
            raise
    
        try:
            # Make sure we have an open connection to device
            if self.__open_connection(self._conn_retry):
                logger.debug(f"Opened ModbusTCP connection to {self._host}:{self._port}/{self._unit_id}")
            else:
                logger.error(f"Unable to open ModbusTCP connection to {self._host}:{self._port}/{self._unit_id}")
                return None
        except:
            logger.error("Exception while attempting to open ModbusTCP connection:")
            raise

        return self

    def __exit__(self, type, value, traceback):
        if not hasattr(self, '_conn'): return

        try:
            self._conn.close()
        except:
            logger.warning(f"Exception while trying to close ModbusTCP connection", exc_info=True)


    def __open_connection(self, retries_left=0):
        # Make sure we have an open connection to server
        if not self._conn.is_open():
            if self._conn_check:
                # Do a quick ping check
                r = os.system('ping -c 1 %s' % self._host)
                if r == 0:
                    logger.debug(f"Host {self._host} appears to be up")
                else:
                    logger.error(f"Unable to ping host {self._host}")

                # Do a quick TCP socket open check
                sock = socket.socket()
                try:
                    sock.connect((self._host, self._port))
                    logger.debug(f"Successfully opened test connection to {self._host}:{self._port}")
                except:
                    logger.exception(f"Cannot open ModbusTCP socket on {self._host}:{self._port}")
                finally:
                    sock.close()

            self._conn.open()

        if self._conn.is_open():
            return True
        elif retries_left > 0:
            logger.warn(f"Connection attempt to {self._host}:{self._port}/{self._unit_id} failed. Retrying")
            return self.__open_connection(retries_left - 1)
        else:
            return False       


    def read(self, register, words, fncode=3, **kwargs):

        # Make sure connection is open
        if not self.__open_connection():
            logger.warning(f"Cannot open ModbusTCP connection to {self._host}:{self._port}/{self._unit_id} in order to take reading")
            return

        try:
            # If register is a string, assume that it's hex and convert to integer
            # (having a "0x" prefix is acceptable but optional)
            if type(register) is str:
                register = int(register, 16)
            
            if fncode == 3: # Default is fncode 3
                val_i = self._conn.read_holding_registers(register, words)
            elif fncode == 4:
                val_i = self._conn.read_input_registers(register, words)
            else:
                logger.warn(f"Unrecognized Modbus function code '{fncode}'")
        except:
            logger.error(f"Exception while processing register {register}")
            raise

        if val_i is None:
            return

        try:
            # The pyModbusTCP library helpfully converts the binary result to a list of integers, so
            # it's best to first convert it back to binary. We assume big-endian order - unless 'order'
            # parameter is set to 'lsr' = least significant register, in which case we reverse the order
            # of the registers.
            if kwargs.get('order') == 'lsr':
                val_i.reverse()

            val_b = struct.pack('>%sH' % len(val_i), *val_i)
            value = self.__process(val_b, **kwargs)

        except:
            logger.error(f"Exception while processing register {register}")
            raise

        return value


    def __process(self, val_b, **rdg):

        # Format identifiers used to unpack the binary result into desired format based on datatype
        fmt = {
            'int16':  'h',
            'uint16': 'H',
            'int32':  'i',
            'sint32': 'i',
            'uint32': 'I',
            'float':  'f',
            'single': 'f',
            'double': 'd'
        }
        # If datatype is not available, fall back on format characters based on data length (in bytes)
        fmt_fallback = [None, 'B', 'H', None, 'I', None, None, None, 'd']

        # Check for defined value mappings in the driver
        # NOTE: The keys for these mappings must be HEX strings
        if 'valuemap' in rdg:
            # NOTE: Currently only mapping against hex representations works
            # Get hex string representing byte reading
            val_h = '0x' + val_b.hex()

            # If the value exists in the map, return 
            if val_h in rdg['valuemap']:
                return rdg['valuemap'][val_h]

        # Get the right format character to convert from binary to the desired data type
        if rdg.get('datatype') in fmt:
            fmt_char = fmt[rdg['datatype']]
        else:
            fmt_char = fmt_fallback[len(val_b)]

        # Convert
        value = struct.unpack('>%s' % fmt_char, val_b)[0]

        # Apply a float multiplier if desired
        if rdg.get('multiplier'):
            value = float(value * rdg['multiplier'])

        # Apply an offset if desired
        if rdg.get('offset'):
            value = value + rdg['offset']

        return value
