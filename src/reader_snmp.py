from easysnmp import Session
import builtins

class Reader(object):
    def __init__(self, d, host, port=161, community='public', version=2):

        self._d = d
        self._host = host
        self._port = port
        self._community = community
        self._version = version

    def __enter__(self):
        # Create an SNMP session to be used for all our requests
        self._session = Session(hostname=self._host, remote_port=self._port, timeout=self._d.params['rtimeout'],
            community=self._community, version=self._version)

        return self

    def __exit__(self, type, value, traceback):
        pass

    def read(self, oid):
        snmpval = self._session.get(oid)
        val = snmpval.value

        return val

    def process(self, rdg, val):

        # Functions to call based on defined datatype
        funcs = {
            'int16':  int,
            'int32':  int,
            'int':    int,
            'float':  float,
            'single': float,
            'double': float
        }

        if 'datatype' in rdg and rdg['datatype'] in funcs:
            value = funcs[rdg['datatype']](val)
        else:
            value = val

        if 'multiplier' in rdg and rdg['multiplier']:
            value = value * rdg['multiplier']

        return value
