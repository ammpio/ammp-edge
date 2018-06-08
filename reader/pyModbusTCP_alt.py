import pyModbusTCP.client
from pyModbusTCP.client import ModbusClient
import struct
import socket
import time

import logging
logger = logging.getLogger(__name__)

class ModbusClient_alt(ModbusClient):

    def _recv_all(self, count):
        buf = b''
        while count:
            newbuf = self._recv(count)
            if not newbuf: return None
            buf += newbuf
            count -= len(newbuf)
        return buf

    def _recv_mbus(self):
        """Receive a modbus frame

        :returns: modbus frame body or None if error
        :rtype: str (Python2) or class bytes (Python3) or None
        """

        const = pyModbusTCP.client.const

        # receive
        # modbus TCP receive
        if self._ModbusClient__mode == const.MODBUS_TCP:
            # 7 bytes header (mbap)
            rx_buffer = self._recv(7)
            # check recv
            if not (rx_buffer and len(rx_buffer) == 7):
                self._ModbusClient__last_error = const.MB_RECV_ERR
                self._ModbusClient__debug_msg('_recv MBAP error')
                self.close()
                return None
            rx_frame = rx_buffer
            # decode header
            (rx_hd_tr_id, rx_hd_pr_id,
             rx_hd_length, rx_hd_unit_id) = struct.unpack('>HHHB', rx_frame)
            # check header
            if not ((rx_hd_tr_id == self._ModbusClient__hd_tr_id) and
                    (rx_hd_pr_id == 0) and
                    (rx_hd_length < 256) and
                    (rx_hd_unit_id == self._ModbusClient__unit_id)):
                self._ModbusClient__last_error = const.MB_RECV_ERR
                self._ModbusClient__debug_msg('MBAP format error')
                self.close()
                return None
            # end of frame
##### SB modification
            rx_buffer = self._recv_all(rx_hd_length - 1)
##### End SB modification
            if not (rx_buffer and
                    (len(rx_buffer) == rx_hd_length - 1) and
                    (len(rx_buffer) >= 2)):
                self._ModbusClient__last_error = const.MB_RECV_ERR
                self._ModbusClient__debug_msg('_recv frame body error')
                self.close()
                return None
            rx_frame += rx_buffer
            # dump frame
            if self._ModbusClient__debug:
                self._pretty_dump('Rx', rx_frame)
            # body decode
            rx_bd_fc = struct.unpack('B', rx_buffer[0:1])[0]
            f_body = rx_buffer[1:]
        # modbus RTU receive
        elif self._ModbusClient__mode == const.MODBUS_RTU:
            # receive modbus RTU frame (max size is 256 bytes)
            rx_buffer = self._recv(256)
            # on _recv error
            if not rx_buffer:
                return None
            rx_frame = rx_buffer
            # dump frame
            if self._ModbusClient__debug:
                self._pretty_dump('Rx', rx_frame)
            # RTU frame min size is 5 bytes
            if len(rx_buffer) < 5:
                self._ModbusClient__last_error = const.MB_RECV_ERR
                self._ModbusClient__debug_msg('short frame error')
                self.close()
                return None
            # check CRC
            if not self._crc_is_ok(rx_frame):
                self._ModbusClient__last_error = const.MB_CRC_ERR
                self._ModbusClient__debug_msg('CRC error')
                self.close()
                return None
            # body decode
            (rx_unit_id, rx_bd_fc) = struct.unpack("BB", rx_frame[:2])
            # check
            if not (rx_unit_id == self._ModbusClient__unit_id):
                self._ModbusClient__last_error = const.MB_RECV_ERR
                self._ModbusClient__debug_msg('unit ID mismatch error')
                self.close()
                return None
            # format f_body: remove unit ID, function code and CRC 2 last bytes
            f_body = rx_frame[2:-2]
        # for auto_close mode, close socket after each request
        if self._ModbusClient__auto_close:
            self.close()
        # check except
        if rx_bd_fc > 0x80:
            # except code
            exp_code = struct.unpack('B', f_body[0:1])[0]
            self._ModbusClient__last_error = const.MB_EXCEPT_ERR
            self._ModbusClient__last_except = exp_code
            self._ModbusClient__debug_msg('except (code ' + str(exp_code) + ')')
            return None
        else:
            # return
            return f_body

    def open(self, try_conn=1, conn_retry_delay=1):
        """Connect to modbus server (open TCP connection)
        :param try_conn: number of times to try connecting if unsuccessful (1=just once)
        :type try_conn: int
        :param conn_retry_delay: delay between connection retries if unsuccessful (seconds)
        :type conn_retry_delay: int

        :returns: connect status (True if open)
        :rtype: bool
        """
        # restart TCP if already open
        if self.is_open():
            logger.debug('Connection was already opened. Closing and reopening.')
            self.close()
        # init socket and connect
        # list available sockets on the target host/port
        # AF_xxx : AF_INET -> IPv4, AF_INET6 -> IPv6,
        #          AF_UNSPEC -> IPv6 (priority on some system) or 4
        # list available socket on target host
        for res in socket.getaddrinfo(self._ModbusClient__hostname, self._ModbusClient__port,
                                      socket.AF_UNSPEC, socket.SOCK_STREAM):
            af, sock_type, proto, canon_name, sa = res
            try:
                self._ModbusClient__sock = socket.socket(af, sock_type, proto)
                self._ModbusClient__sock.settimeout(self._ModbusClient__timeout)
            except socket.error:
                logger.exception('Could not set up socket for connection to %s:%s' % sa)
                self._ModbusClient__sock = None
                continue

            connected = False
            while connected is False and try_conn > 0:
                try:
                    logger.debug('Trying to open connection to %s:%s' % sa)
                    self._ModbusClient__sock.connect(sa)
                    logger.debug('Succeeded')
                    connected = True
                except socket.error:
                    try_conn = try_conn - 1
                    logger.exception('Connection to %s:%s failed. %i retries remaining' % (sa[0], sa[1], try_conn))
                    if try_conn > 0:
                        time.sleep(conn_retry_delay)
                    pass

            if not connected:
                logger.error('Did not manage to connect. Giving up.')
                self._ModbusClient__sock.close()
                self._ModbusClient__sock = None
                continue
            break

        # check connect status
        if self._ModbusClient__sock is not None:
            return True
        else:
            self._ModbusClient__last_error = const.MB_CONNECT_ERR
            logger.error('connect error')
            return False
