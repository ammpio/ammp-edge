import logging

logger = logging.getLogger(__name__)

"""
This module is currently used by the rawudp reader.
"""

# Parsing SMA energy meter protocol
# Adapted from https://www.sma.de/fileadmin/content/global/Partner/Documents/SMA_Labs/EMETER-Protokoll-TI-en-10.pdf

# specific channels in the datagram for the Energy-meter
SMA_CHANNELS = {
    # totals
    1: ('total_P_consumption', 'W', 'total_E_consumption', 'kWh'),
    2: ('total_P_supply', 'W', 'total_E_supply', 'kWh'),
    14: ('frequency', 'Hz'),
    # phase 1
    21: ('L1_P_consumption', 'W', 'L1_E_consumption', 'kWh'),
    22: ('L1_P_supply', 'W', 'L1_E_supply', 'kWh'),
    31: ('L1_current', 'A'),
    32: ('L1_voltage', 'V'),
    # phase 2
    41: ('L2_P_consumption', 'W', 'L2_E_consumption', 'kWh'),
    42: ('L2_P_supply', 'W', 'L2_E_supply', 'kWh'),
    51: ('L2_current', 'A'),
    52: ('L2_voltage', 'V'),
    # phase 3
    61: ('L3_P_consumption', 'W', 'L2_E_consumption', 'kWh'),
    62: ('L3_P_supply', 'W', 'L2_E_supply', 'kWh'),
    71: ('L3_current', 'A'),
    72: ('L3_voltage', 'V'),
    # common
    36864: ('speedwire-version', ''),
}
CHANNEL_NAME_ACTUAL = 0
CHANNEL_UNIT_ACTUAL = 1
CHANNEL_NAME_CUMULATIVE = 2
CHANNEL_UNIT_CUMULATIVE = 3

# unit definitions with scaling
SMA_UNIT_CONVERSION = {
    'W': 10,
    'VA': 10,
    'VAr': 10,
    'kWh': 3600000,
    'kVAh': 3600000,
    'kVArh': 3600000,
    'A': 1000,
    'V': 1000,
    'Hz': 1000,
}

BYTEORDER_BIG = 'big'

DATATYPE_ACTUAL = 'actual'
DATATYPE_CUMULATIVE = 'cumulative'
DATATYPE_VERSION = 'version'
DATATYPE_UNKNOWN = 'unknown'


def decode_OBIS(obis: bytes) -> tuple:
    measurement = int.from_bytes(obis[0:2], byteorder=BYTEORDER_BIG)
    raw_type = int.from_bytes(obis[2:3], byteorder=BYTEORDER_BIG)
    if raw_type == 4:
        datatype = DATATYPE_ACTUAL
    elif raw_type == 8:
        datatype = DATATYPE_CUMULATIVE
    elif raw_type == 0 and measurement == 36864:
        datatype = DATATYPE_VERSION
    else:
        datatype = DATATYPE_UNKNOWN
        logger.error(
            f"unknown datatype: measurement {measurement} raw_type {raw_type}")
    return measurement, datatype


def parse_datagram_response(response: bytes) -> tuple:
    values = {}
    data_length = int.from_bytes(response[12:14], byteorder=BYTEORDER_BIG) + 16
    serial_number = int.from_bytes(response[20:24], byteorder=BYTEORDER_BIG)

    # initial position for relevant data in datagram
    position = 28
    while position < data_length:
        (measurement, datatype) = decode_OBIS(response[position:position + 4])
        # spot values
        if datatype == DATATYPE_ACTUAL:
            value = int.from_bytes(
                response[position + 4:position + 8], byteorder=BYTEORDER_BIG)
            position += 8
            if measurement in SMA_CHANNELS:
                channel_spec = SMA_CHANNELS[measurement]
                conversion_factor = SMA_UNIT_CONVERSION[channel_spec[CHANNEL_UNIT_CUMULATIVE]]
                values[channel_spec[CHANNEL_NAME_ACTUAL]
                       ] = value / conversion_factor

        # counter values
        elif datatype == DATATYPE_CUMULATIVE:
            value = int.from_bytes(
                response[position + 4:position + 12], byteorder=BYTEORDER_BIG)
            position += 12
            if measurement in SMA_CHANNELS:
                channel_spec = SMA_CHANNELS[measurement]
                conversion_factor = SMA_UNIT_CONVERSION[channel_spec[CHANNEL_UNIT_CUMULATIVE]]
                values[channel_spec[CHANNEL_NAME_CUMULATIVE]
                       ] = value / conversion_factor

        elif datatype == DATATYPE_VERSION:
            position += 8

    return serial_number, values
