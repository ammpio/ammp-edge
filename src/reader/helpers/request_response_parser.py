import logging
import struct

logger = logging.getLogger(__name__)

"""
This module is currently used by the rawtcp reader.
TODO: Make necessary extensions to also be usable by rawserial reader.
"""


def generate_request(request_schema: dict, **rdg: dict) -> bytes:
    """
    Inputs:
        'schema' is the request schema
        'rdg' contains additional (driver) parameters used for generating
            the request based on this schema
    Output:
        bytes object (request)
    """

    request = b''

    for c in request_schema['sequence']:

        if c['type'] == 'input':
            if c.get('byte_order', 'msb') in ['lsb', 'little']:
                byte_order = 'little'
            else:
                byte_order = 'big'

            input_value = rdg.get(c['name'], c.get('default_value'))
            if input_value is None:
                logger.warn(f"Input value '{c['name']}' not provided. Skipping")
                continue

            if c['input_datatype'] == 'uint':
                if not isinstance(input_value, int):
                    logger.error(f"Expected input value of type int. Got {type(input_value)}. Skipping")
                    continue

                component = input_value.to_bytes(c['num_bytes'], byte_order)

        elif c['type'] == 'const':
            component = get_bytes(c['value'])

        elif c['type'] == 'crc':
            if c['num_bytes'] == 2:
                component = crc16(request)
            else:
                logger.warn("Only CRC16 currently implemented. Omitting CRC.")
                component = b''

        request += component

    return request


def parse_response(response: bytes, response_schema: dict, **rdg: dict) -> bytes:
    """
    Inputs:
        'response' is the response being parsed
        'response_schema' is the response schema
        'rdg' contains additional (driver) parameters used for parsing the
            response based on this schema
    Output:
        bytes object (value from response)
    """

    if response_schema.get('check_crc16'):
        response_body = response[:-2]
        response_crc = response[-2:]
        if crc16(response_body) != response_crc:
            logger.warn("Response CRC doesn't match calculated CRC. Ignoring")
            return None

    if isinstance(response_schema['pos'], int):
        pos = response_schema['pos']

    if isinstance(response_schema['length'], int):
        length = response_schema['length']
    elif isinstance(response_schema['length'], dict):
        length = rdg[response_schema['length']['name']] * response_schema['length'].get('multiplier', 1)

    return response[pos:pos+length]


def crc16(input_bytes: bytes) -> bytes:
    crc = 0xFFFF
    for byte in input_bytes:
        crc ^= byte
        for i in range(8):
            lsb = crc & 1
            crc >>= 1
            if lsb:
                crc ^= 0xA001
    return struct.pack('<H', crc)


def get_bytes(string: str) -> bytes:
    if string.startswith('0x'):
        try:
            return bytes.fromhex(string[2:])
        except ValueError:
            logger.warn(f"String {string} starts with 0x but is not hexadecimal. Interpreting literally")

    return string.encode('utf-8')


# Example schema:
# example_schema = {
#     'request': {
#         'sequence': [
#             {
#                 'name': 'unit_id',
#                 'type': 'input',
#                 'input_datatype': 'uint',
#                 'num_bytes': 4,
#                 'byte_order': 'msb'
#             },
#             {
#                 'name': '_reserved',
#                 'type': 'const',
#                 'value': '0x01',
#             },
#             {
#                 'name': 'fn_code',
#                 'type': 'input',
#                 'input_datatype': 'uint',
#                 'num_bytes': 1,
#                 'byte_order': 'msb'
#             },
#             {
#                 'name': 'register',
#                 'type': 'input',
#                 'input_datatype': 'uint',
#                 'num_bytes': 2,
#                 'byte_order': 'msb'
#             },
#             {
#                 'name': 'words',
#                 'type': 'input',
#                 'input_datatype': 'uint',
#                 'num_bytes': 2,
#                 'byte_order': 'msb'
#             },
#             {
#                 'name': '_crc',
#                 'type': 'crc',
#                 'num_bytes': 2,
#                 'byte_order': 'msb',
#             }
#         ]
#     },
#     'response': {
#         'check_crc16': True,
#         'pos': 6,
#         'length': {
#             'name': 'words',
#             'multiplier': 2
#         }
#     }
# }
#
# example_inputs = {
#     'unit_id': 234234,
#     'fn_code': 4,
#     'register': 2,
#     'words': 2
# }
