import logging
logger = logging.getLogger(__name__)

import minimalmodbus, serial
import struct

class Reader(object):
    def __init__(self, device, slaveaddr, baudrate=9600, bytesize=8, parity='none', stopbits=1, timeout=5, debug=False, **kwargs):

        self._device = device
        self._slaveaddr = slaveaddr
        self._baudrate = baudrate
        self._bytesize = bytesize
        self._stopbits = stopbits
        self._timeout = timeout
        self._debug = debug

        paritysel = {'none': serial.PARITY_NONE, 'odd': serial.PARITY_ODD, 'even': serial.PARITY_EVEN}
        self._parity = paritysel[parity]


    def __enter__(self):
        # Create a Serial connection to be used for all our requests
        try:
            self._conn = minimalmodbus.Instrument(port=self._device, slaveaddress=self._slaveaddr)
        except:
            logger.error('Exception while attempting to create serial connection:')
            raise

        try:
            self._conn.serial.debug = self._debug
            self._conn.serial.timeout = self._timeout

            # Set up serial connection parameters according to device driver
            self._conn.serial.baudrate = self._baudrate
            self._conn.serial.bytesize = self._bytesize
            self._conn.serial.parity = self._parity
            self._conn.serial.stopbits = self._stopbits
        except:
            logger.error('Exception while attempting to configure serial connection:')
            raise

        try:
            # Make sure we have an open connection to device
            if not self._conn.serial.is_open:
                self._conn.serial.open()
                if self._conn.serial.is_open:
                    logger.debug('Opened serial connection to %s:%s' % (self._device, self._slaveaddr))
                else:
                    logger.error('Unable to open serial connection to %s:%s' % (self._device, self._slaveaddr))
                    return None
        except:
            logger.error('Exception while attempting to open serial connection:')
            raise

        return self

    def __exit__(self, type, value, traceback):
        if not hasattr(self, '_conn'): return

        try:
            self._conn.serial.close()
        except:
            logger.warning('Could not close serial connection', exc_info=True)


    def read(self, register, words, fncode, **kwargs):

        try:
            val_i = self._conn.read_registers(register, words, fncode)
        except Exception:
            logger.error('Exception while reading register %d' % register)
            raise

        try:
            # The minimalmodbus library helpfully converts the binary result to a list of integers, so
            # it's best to first convert it back to binary (assuming big-endian)
            val_b = struct.pack('>%sH' % len(val_i), *val_i)
            value = self.__process(val_b, **kwargs)
        except:
            logger.error('Exception while processing value from register %d' % register)
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
            value = value * rdg['multiplier']

        # Apply an offset if desired
        if rdg.get('offset'):
            value = value + rdg['offset']

        return value
