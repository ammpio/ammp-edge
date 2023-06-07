DEFAULT_NMAP_SCAN_OPTS = ['--disable-arp-ping', '-p', '22,80,443,502']
DEFAULT_SERIAL_DEV = '/dev/ttyAMA0'

MODTCP_PORT = 502
SMA_MODTCP_UNIT_IDS = [3]
DSE_MODTCP_UNIT_IDS = [1]
MODTCP_TIMEOUT = 1
MODTCP_UNIT_ID_KEY = 'unit_id'
MODTCP_FIELD_KEY = 'field'
MODTCP_REGISTER_KEY = 'register'
MODTCP_WORDS_KEY = 'words'
MODTCP_DATATYPE_KEY = 'uint32'
SMA_MODTCP_SCAN_ITEMS = [
    {
        MODTCP_FIELD_KEY: 'sma_device_class',
        MODTCP_REGISTER_KEY: 30051,
        MODTCP_WORDS_KEY: 2,
        MODTCP_DATATYPE_KEY: 'uint32'
    },
    {
        MODTCP_FIELD_KEY: 'sma_device_type',
        MODTCP_REGISTER_KEY: 30053,
        MODTCP_WORDS_KEY: 2,
        MODTCP_DATATYPE_KEY: 'uint32'
    },
    {
        MODTCP_FIELD_KEY: 'sma_inverter_serial',
        MODTCP_REGISTER_KEY: 30057,
        MODTCP_WORDS_KEY: 2,
        MODTCP_DATATYPE_KEY: 'uint32'
    }
]
DSE_MODTCP_SCAN_ITEMS = [
    {
        MODTCP_FIELD_KEY: 'dse_model_number',
        MODTCP_REGISTER_KEY: 769,
        MODTCP_WORDS_KEY: 1,
        MODTCP_DATATYPE_KEY: 'uint16'
    },
    {
        MODTCP_FIELD_KEY: 'dse_control_mode',
        MODTCP_REGISTER_KEY: 772,
        MODTCP_WORDS_KEY: 1,
        MODTCP_DATATYPE_KEY: 'uint16'
    },
    {
        MODTCP_FIELD_KEY: 'dse_serial',
        MODTCP_REGISTER_KEY: 770,
        MODTCP_WORDS_KEY: 2,
        MODTCP_DATATYPE_KEY: 'uint32'
    },
]
MODTCP_RESULT_KEY = 'modbustcp'

SERIAL_SCAN_SIGNATURES = [{
    'name': 'Gamicos ultrasonic sensor',
    'slave_id': 1,
    'reading': 'fuel level (distance from sensor (m))',
    'register': 1,
    'words': 2,
    'datatype': 'float',
    'fncode': 3
}, {
    'name': 'IMT irradiation sensor',
    'slave_id': 2,
    'reading': 'irradiance (W/m2)',
    'register': 0,
    'words': 1,
    'datatype': 'uint16',
    'fncode': 4
}, {
    'name': 'APM303 genset controller',
    'slave_id': 5,
    'reading': 'oil pressure (bar)',
    'register': 28,
    'words': 1,
    'datatype': 'int16',
    'fncode': 4
}, {
    'name': 'APM303 genset controller',
    'slave_id': 6,
    'reading': 'oil pressure (bar)',
    'register': 28,
    'words': 1,
    'datatype': 'int16',
    'fncode': 4
}, {
    'name': 'Cummins PS0600',
    'slave_id': 4,
    'reading': 'genset state',
    'register': 10,
    'words': 1,
    'datatype': 'uint16',
    'fncode': 3
}, {
    'name': 'Cummins PS0600',
    'slave_id': 3,
    'reading': 'genset state',
    'register': 10,
    'words': 1,
    'datatype': 'uint16',
    'fncode': 3
}, {
    'name': 'Holykell HPT604',
    'slave_id': 7,
    'reading': 'fuel level (distance from sensor (mm))',
    'register': 2,
    'words': 1,
    'datatype': 'uint16',
    'fncode': 3
}, {
    'name': 'Holykell HPT604',
    'slave_id': 8,
    'reading': 'fuel level (distance from sensor (mm))',
    'register': 2,
    'words': 1,
    'datatype': 'uint16',
    'fncode': 3
}, {
    'name': 'Holykell HPT604-LT',
    'slave_id': 7,
    'reading': 'fuel level (distance from sensor (mm))',
    'register': 2,
    'words': 2,
    'datatype': 'float',
    'fncode': 3
}, {
    'name': 'Holykell HPT604-LT',
    'slave_id': 8,
    'reading': 'fuel level (distance from sensor (mm))',
    'register': 2,
    'words': 2,
    'datatype': 'float',
    'fncode': 3
}, {
    'name': 'Carlo Gavazzi meter EM24',
    'slave_id': 9,
    'reading': 'Frequency (Hz*10)',
    'register': 55,
    'words': 1,
    'datatype': 'int16',
    'fncode': 3
}, {
    'name': 'Carlo Gavazzi meter EM330',
    'slave_id': 10,
    'reading': 'Frequency (Hz*10)',
    'register': 51,
    'words': 1,
    'datatype': 'int16',
    'fncode': 3
}]

DEFAULT_SERIAL_BAUD_RATE = 9600

NMAP_ADDR_KEY = 'addr'
NMAP_ADDR_TYPE_KEY = 'addrtype'
NMAP_IP_ADDR_TYPE = 'ipv4'
NMAP_MAC_ADDR_TYPE = 'mac'
NMAP_VENDOR_KEY = 'vendor'
NMAP_PORTS_KEY = 'ports'
NMAP_PORT_KEY = 'port'
NMAP_PORT_STATE_KEY = 'state'
NMAP_PORT_OPEN = 'open'
NMAP_PORTID_KEY = 'portid'

HOST_IP_KEY = 'ipv4'
HOST_MAC_KEY = 'mac'
HOST_VENDOR_KEY = 'vendor'
HOST_PORTS_KEY = 'ports'

MIN_NETMASK_BITS_TO_SCAN = 22