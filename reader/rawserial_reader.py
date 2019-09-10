import logging
logger = logging.getLogger(__name__)

import serial
import struct
import time

class Reader(object):
    def __init__(self, device, baudrate=9600, bytesize=8, parity='none', stopbits=1, timeout=5, **kwargs):

        self._device = device
        self._baudrate = baudrate
        self._bytesize = bytesize
        self._stopbits = stopbits
        self._timeout = timeout

        paritysel = {'none': serial.PARITY_NONE, 'odd': serial.PARITY_ODD, 'even': serial.PARITY_EVEN}
        self._parity = paritysel[parity]

        self._stored_responses = {}

    def __enter__(self):
        # Create a Serial connection to be used for all our requests
        try:
            self._conn = serial.Serial(port=self._device,
                                    baudrate=self._baudrate,
                                    bytesize=self._bytesize,
                                    parity=self._parity,
                                    stopbits=self._stopbits,
                                    timeout=self._timeout)
        except:
            logger.error('Exception while attempting to create serial connection:')
            raise

        try:
            # Make sure we have an open connection to device
            if not self._conn.is_open:
                self._conn.open()
                if self._conn.is_open:
                    logger.debug(f"Opened serial connection to {self._device}")
                else:
                    logger.error(f"Unable to open serial connection to {self._device}")
                    return None
        except:
            logger.error("Exception while attempting to open serial connection:")
            raise

        return self

    def __exit__(self, type, value, traceback):
        if not hasattr(self, '_conn'): return

        try:
            self._conn.close()
        except:
            logger.warning("Could not close serial connection", exc_info=True)


    def read(self, query, pos, length, resp_termination=None, **rdg):

        if query in self._stored_responses:
            resp = self._stored_responses[query]
        else:
            try:
                self._conn.write(self.get_bytes(query))

                # If response termination is explicitly provided, use that. Otherwise attempt to read all.
                if resp_termination:
                    resp = self._conn.read_until(self.get_bytes(resp_termination))
                else:
                    # Allow time for response to be sent
                    time.sleep(1)
                    resp = self._conn.read_all()

                if resp == b'':
                    logger.warn("No response received from device")
                    return
            
            except:
                logger.error(f"Exception while reading response to query {repr(query)}")
                raise

        # Save response in case other readings rely on the same query
        self._stored_responses[query] = resp

        try:
            # Extract the actual values requested
            val_b = resp[pos:pos+length]
            value = self.process(val_b, **rdg)
        except:
            logger.error(f"Exception while processing value from response {repr(resp)}, position {pos}, length {length}")
            raise

        return value

    @classmethod
    def process(cls, val_b, **rdg):
        if rdg.get('parse_as') == 'str':
            try:
                string = val_b.decode('utf-8')
                value = float(string)
            except ValueError:
                logger.error(f"Could not parse {repr(val_b)} as the string representation of a numerical value")
                return
        elif rdg.get('parse_as') == 'hex':
            try:
                hex_string = val_b.decode('utf-8')
                val_b = bytes.fromhex(hex_string)
            except ValueError:
                logger.error(f"Could not parse {repr(val_b)} as a hex value")
                return
            value = cls.value_from_binary(val_b, **rdg)
        else:
            value = cls.value_from_binary(val_b, **rdg)

        return cls.process_value(value, **rdg)


    @staticmethod
    def value_from_binary(val_b, **rdg):

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
            # Get hex string representing byte reading
            val_h = val_b.hex()

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

        return value


    @staticmethod
    def process_value(value, **rdg):

        # Apply a float multiplier if desired
        if rdg.get('multiplier'):
            value = value * rdg['multiplier']

        # Apply an offset if desired
        if rdg.get('offset'):
            value = value + rdg['offset']

        return value

    @staticmethod
    def get_bytes(string):
        if string.startswith('0x'):
            try:
                return bytes.fromhex(string[2:])
            except ValueError:
                logger.warn(f"String {string} starts with 0x but is not hexadecimal. Interpreting literally")

        return string.encode('utf-8')
