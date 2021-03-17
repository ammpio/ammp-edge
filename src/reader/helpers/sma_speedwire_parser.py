import logging
from collections import defaultdict
from typing import Tuple

logger = logging.getLogger(__name__)

BYTEORDER_BIG = 'big'

OBIS_TYPE_ACTUAL = 4
OBIS_TYPE_COUNTER = 8
OBIS_TYPE_VERSION = 0
OBIS_CHANNEL_VERSION = 36864


def decode_obis(obis: bytes) -> Tuple[int, int]:
    obis_channel = int.from_bytes(obis[0:2], byteorder=BYTEORDER_BIG)
    obis_type = int.from_bytes(obis[2:3], byteorder=BYTEORDER_BIG)
    return obis_channel, obis_type


def parse_datagram_response(response: bytes) -> Tuple[int, dict]:
    data_length = int.from_bytes(response[12:14], byteorder=BYTEORDER_BIG) + 16
    serial_number = int.from_bytes(response[20:24], byteorder=BYTEORDER_BIG)

    # top-level key is channel, second-level key is type
    values = defaultdict(dict)

    # initial position for relevant data in datagram
    position = 28
    while position < data_length:
        (obis_channel, obis_type) = decode_obis(
            response[position:position + 4])
        # spot values
        if obis_type == OBIS_TYPE_ACTUAL:
            value = response[position + 4:position + 8]
            position += 8
        # counter values
        elif obis_type == OBIS_TYPE_COUNTER:
            value = response[position + 4:position + 12]
            position += 12
        # version value
        elif obis_type == OBIS_TYPE_VERSION and obis_channel == OBIS_CHANNEL_VERSION:
            position += 8
            value = response[position + 4:position + 8]
        else:
            logger.error(
                f"Cannot parse OBIS channel and type {obis_channel}.{obis_type}; stopping parse")
            value = None
            break

        values[obis_channel][obis_type] = value

    return serial_number, dict(values)
