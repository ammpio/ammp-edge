import logging

logger = logging.getLogger(__name__)

import builtins

from easysnmp import Session


class Reader(object):
    def __init__(self, host, port=161, community="public", version=2, timeout=60, **kwargs):

        self._host = host
        self._port = port
        self._community = community
        self._version = version
        self._timeout = timeout

    def __enter__(self):
        # Create an SNMP session to be used for all our requests
        self._session = Session(
            hostname=self._host,
            remote_port=self._port,
            timeout=self._timeout,
            community=self._community,
            version=self._version,
        )

        return self

    def __exit__(self, type, value, traceback):
        pass

    def read(self, oid, **kwargs):
        snmpval = self._session.get(oid)
        val = snmpval.value

        return val
