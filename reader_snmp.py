from pysnmp.hlapi import *
import builtins

class Reader(object):
    def __init__(self, d, host, port=161, community='public', mpmodel=0):

        self._d = d
        self._host = host
        self._port = port
        self._community = community
        self._mpmodel = mpmodel

    def __enter__(self):
        return self

    def __exit__(self, type, value, traceback):
        pass

    def read(self, oid):
        errorIndication, errorStatus, errorIndex, varBind = next(
            getCmd(SnmpEngine(),
                   CommunityData(self._community, mpModel=self._mpmodel),
                   UdpTransportTarget((self._host, self._port), timeout=self._d.params['rtimeout']),
                   ContextData(),
                   ObjectType(ObjectIdentity(oid)))
            )

        if errorIndication:
            raise Exception(errorIndication)
        elif errorStatus:
            raise Exception('%s at %s' % (errorStatus.prettyPrint(), errorIndex and varBinds[int(errorIndex) - 1][0] or '?'))
        elif len(varBind) == 0:
            raise Exception('No data returned from SNMP query')

#        self._d.logfile.debug('READ: SNMP: ' + ' = '.join([x.prettyPrint() for x in varBind[0]]))

        return varBind[0][1]

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
