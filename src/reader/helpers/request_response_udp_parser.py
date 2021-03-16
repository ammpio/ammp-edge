import logging

logger = logging.getLogger(__name__)

"""
This module is currently used by the rawudp reader.
"""

# Parsing SMA energy meter protocol
# Taken from https://www.sma.de/fileadmin/content/global/Partner/Documents/SMA_Labs/EMETER-Protokoll-TI-en-10.pdf

# specific channels in the datagram for the Energy-meter
sma_channels = {
    # totals
    1: ('total_P_consumption', 'W', 'kWh'),
    2: ('total_P_supply', 'W', 'kWh'),
    14: ('frequency', 'Hz'),
    # phase 1
    21: ('L1_P_consumption', 'W', 'kWh'),
    22: ('L1_P_supply', 'W', 'kWh'),
    31: ('L1_current', 'A'),
    32: ('L1_voltage', 'V'),
    # phase 2
    41: ('L2_P_consumption', 'W', 'kWh'),
    42: ('L2_P_supply', 'W', 'kWh'),
    51: ('L2_current', 'A'),
    52: ('L2_voltage', 'V'),
    # phase 3
    61: ('L3_P_consumption', 'W', 'kWh'),
    62: ('L3_P_supply', 'W', 'kWh'),
    71: ('L3_current', 'A'),
    72: ('L3_voltage', 'V'),
    # common
    36864: ('speedwire-version', ''),
}

# unit definitions with scaling
sma_units = {
    "W": 10,
    "VA": 10,
    "VAr": 10,
    "kWh": 3600000,
    "kVAh": 3600000,
    "kVArh": 3600000,
    "A": 1000,
    "V": 1000,
    "°": 1000,
    "Hz": 1000,
}


def decode_OBIS(obis):
    measurement = int.from_bytes(obis[0:2], byteorder='big')
    raw_type = int.from_bytes(obis[2:3], byteorder='big')
    if raw_type == 4:
        datatype = 'actual'
    elif raw_type == 8:
        datatype = 'cumulative'
    elif raw_type == 0 and measurement == 36864:
        datatype = 'version'
    else:
        datatype = 'unknown'
        print(
            f"unknown datatype: measurement {measurement} datatype {datatype} raw_type {raw_type}")
    return (measurement, datatype)


def parse_datagram_response(response: bytes) -> dict:
    values = {}
    data_length = int.from_bytes(response[12:14], byteorder='big')+16
    serial_number = response[20:24]
    # initial position for relevant data in datagram
    position = 28
    while position < data_length:
        (measurement, datatype) = decode_OBIS(response[position:position + 4])
        if datatype == 'actual':
            value = int.from_bytes(
                response[position + 4:position + 8], byteorder='big')
            position += 8
            if measurement in sma_channels.keys():
                values[sma_channels[measurement][0]] = value / \
                    sma_units[sma_channels[measurement][1]]
                values[sma_channels[measurement][0] +
                       'unit'] = sma_channels[measurement][1]
        # counter values
        elif datatype == 'cumulative':
            value = int.from_bytes(
                response[position + 4:position + 12], byteorder='big')
            position += 12
            if measurement in sma_channels.keys():
                values[sma_channels[measurement][0] + 'counter'] = value / \
                    sma_units[sma_channels[measurement][2]]
                values[sma_channels[measurement][0] +
                       'counterunit'] = sma_channels[measurement][2]
        elif datatype == 'version':
            position += 8

    return serial_number, values
