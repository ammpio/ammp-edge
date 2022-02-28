import logging

import minimalmodbus
import serial
import struct

logger = logging.getLogger(__name__)


class Reader(object):
    def __init__(self, device, slaveaddr, baudrate=9600, bytesize=8, parity='none', stopbits=1, timeout=5, debug=False,
                 **kwargs):

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
        except Exception:
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
        except Exception:
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
        except Exception:
            logger.error('Exception while attempting to open serial connection:')
            raise

        return self

    def __exit__(self, type, value, traceback):
        if not hasattr(self, '_conn'):
            return

        try:
            self._conn.serial.close()
        except Exception:
            logger.warning('Could not close serial connection', exc_info=True)

    def read(self, register, words, fncode, **kwargs):

        try:
            val_i = self._conn.read_registers(register, words, fncode)
        except minimalmodbus.NoResponseError:
            logger.error(f"No response when trying to read {self._device}: slave {self._slaveaddr}: register {register}")
            raise
        except Exception:
            logger.error(f"Exception while reading {self._device}: slave {self._slaveaddr}: register {register}")
            raise

        try:
            # The minimalmodbus library helpfully converts the binary result to a list of integers, so
            # it's best to first convert it back to binary. We assume big-endian order - unless 'order'
            # parameter is set to 'lsr' = least significant register, in which case we reverse the order
            # of the registers.
            if kwargs.get('order') == 'lsr':
                val_i.reverse()
                
            val_b = struct.pack('>%sH' % len(val_i), *val_i)
        except Exception:
            logger.error('Exception while processing value from register %d' % register)
            raise

        return val_b
