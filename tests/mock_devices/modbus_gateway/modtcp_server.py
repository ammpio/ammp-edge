import logging
from socketserver import TCPServer
from argparse import ArgumentParser

from umodbus.server.tcp import RequestHandler, get_server
from umodbus.utils import log_to_stream

from stubs import EmsMockResponses, SmaStpMockResponses

logger = logging.getLogger(__name__)
logging.basicConfig(level=logging.DEBUG)

# Add stream handler to logger 'uModbus'.
log_to_stream(level=logging.DEBUG)

ALL_ADDRESSES = list(range(0, 65535))
READ_REGISTER_FUNCTION_CODES = [3, 4]

# Parse command line arguments
parser = ArgumentParser()
parser.add_argument("-b", "--bind", default="localhost:502")

args = parser.parse_args()
if ":" not in args.bind:
    args.bind += ":502"
host, port = args.bind.rsplit(":", 1)
port = int(port)

TCPServer.allow_reuse_address = True
try:
    app = get_server(TCPServer, (host, port), RequestHandler)
except PermissionError:
    print("You don't have permission to bind on {}".format(args.bind))
    print("Hint: try with a different port (ex: --bind localhost:50200)")
    exit(1)


@app.route(slave_ids=[EmsMockResponses.SLAVE_ID], function_codes=READ_REGISTER_FUNCTION_CODES, addresses=ALL_ADDRESSES)
def read_ems(slave_id: int, function_code: int, address: int) -> int:
    """" Return value of address. """
    return EmsMockResponses.REGISTER_MAP.get(address, EmsMockResponses.DEFAULT_RESPONSE)


@app.route(slave_ids=[SmaStpMockResponses.SLAVE_ID], function_codes=READ_REGISTER_FUNCTION_CODES, addresses=ALL_ADDRESSES)
def read_sma_stp(slave_id: int, function_code: int, address: int) -> int:
    """" Return value of address. """
    return SmaStpMockResponses.REGISTER_MAP.get(address, SmaStpMockResponses.DEFAULT_RESPONSE)


if __name__ == '__main__':
    try:
        # logger.info("Starting ModbusTCP server")
        app.serve_forever()
    finally:
        app.shutdown()
        app.server_close()
