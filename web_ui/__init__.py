# Set up logging
import logging

from flask import Flask, render_template, request
from node_mgmt import NetworkEnv, EnvScanner
from db_model import NodeConfig
import os
from urllib.request import urlopen
from dotenv import load_dotenv

logging.basicConfig(format='%(name)s [%(levelname)s] %(message)s', level='INFO')
logger = logging.getLogger(__name__)

app = Flask(__name__)

try:
    nodeconf = NodeConfig.get()
    node_id = nodeconf.node_id
except NodeConfig.DoesNotExist:
    logger.info('No node configuration found in internal database.')
    node_id = 'Not yet initialized'
except ValueError:
    logger.warning('ValueError in node config.', exc_info=True)
    node_id = ''

# Load additional environment variables from env file
dotenv_path = os.path.join(os.environ.get('SNAP_COMMON', '.'), '.env')
load_dotenv(dotenv_path)

@app.route("/")
def index():

    device_online = test_online()
    snap_revision = os.getenv('SNAP_REVISION', 'N/A')

    try:
        net_env = NetworkEnv()
        # An ugly-ish way to combine two dicts, in order to get the interface names on the same level
        network_interfaces = [{**v, **{'name': k}} for k, v in net_env.interfaces.items()]
    except Exception:
        logger.exception("Exception while doing network scan")
        network_interfaces = []

    return render_template(
        'index.html',
        node_id=node_id,
        device_online=device_online,
        snap_revision=snap_revision,
        ssh_fingerprint=os.environ.get('SSH_FINGERPRINT'),
        network_interfaces=network_interfaces,
        )


@app.route("/env_scan")
def env_scan():
    try:
        scanner = EnvScanner()
        scan_result = scanner.do_scan()
    except Exception as e:
        logger.exception("Exception while doing scan")
        return f"Error: {e}"

    return render_template(
        'env_scan.html',
        node_id=node_id,
        scan_result=scan_result
        )


@app.route("/network_scan")
def network_scan():
    interface = request.args.get('interface')

    try:
        net_env = NetworkEnv(default_ifname=interface)
        network_scan_hosts = net_env.network_scan()
        # Grab it from the object, in case the interface name was overridden
        interface = net_env.default_ifname
    except Exception as e:
        logger.exception("Exception while doing network scan")
        return f"Error: {e}"

    return render_template(
        'network_scan.html',
        node_id=node_id,
        interface=interface,
        network_scan_hosts=network_scan_hosts
        )


def test_online():
    TEST_URL = 'https://www.ammp.io/'
    try:
        urlopen(TEST_URL, timeout=20)
        return True
    except Exception as e:
        logger.error(f"Error {e} while checking internet connectivity")
        return False
