import logging
logger = logging.getLogger(__name__)

import serial
import struct
import time
import re

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


    def read(self, query, pos, length, resp_template=None, resp_termination=None, **rdg):

        if query in self._stored_responses:
            resp = self._stored_responses[query]
        else:
            try:
                logger.debug(f"Writing {query} to serial port")
                self._conn.write(self.get_bytes(query))

                # If response termination is explicitly provided, use that. Otherwise attempt to read all.
                if resp_termination:
                    resp = self._conn.read_until(self.get_bytes(resp_termination))
                else:
                    # Allow time for response to be sent
                    time.sleep(1)
                    resp = self._conn.read_all()
                    logger.debug(f"Received {repr(resp)} from serial port")

                if resp == b'':
                    logger.warn("No response received from device")
                    return
                
                # If a template is defined, check whether the response matches it.
                if resp_template:
                    # Since resp is binary, the template needs to be also
                    template_b = resp_template.encode('utf-8')
                    if not re.match(template_b, resp):
                        logger.warn(f"Response {repr(resp)} does not match template {resp_template}. Discarding")
                        return
            
            except:
                logger.error(f"Exception while reading response to query {repr(query)}")
                raise

        # Save response in case other readings rely on the same query
        self._stored_responses[query] = resp

        try:
            # Extract the actual values requested
            val_b = resp[pos:pos+length]
        except:
            logger.error(f"Exception while processing value from response {repr(resp)}, position {pos}, length {length}")
            raise

        return val_b


    @staticmethod
    def get_bytes(string):
        if string.startswith('0x'):
            try:
                return bytes.fromhex(string[2:])
            except ValueError:
                logger.warn(f"String {string} starts with 0x but is not hexadecimal. Interpreting literally")

        return string.encode('utf-8')
