import pyModbusTCP.client
from pyModbusTCP.client import ModbusClient
import struct

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

