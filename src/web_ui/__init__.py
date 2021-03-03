# Set up logging
import logging

from flask import Flask, render_template, request
from node_mgmt import NetworkEnv, EnvScanner, get_ssh_fingerprint
from db_model import NodeConfig
import os
from urllib.request import urlopen
from kvstore import KVStore, keys

logging.basicConfig(
    format='%(name)s [%(levelname)s] %(message)s', level='INFO')
logger = logging.getLogger(__name__)

app = Flask(__name__)

kvs = KVStore()

try:
    nodeconf = NodeConfig.get()
    node_id = nodeconf.node_id
except NodeConfig.DoesNotExist:
    logger.info('No node configuration found in internal database.')
    node_id = 'Not yet initialized'
except ValueError:
    logger.warning('ValueError in node config.', exc_info=True)
    node_id = ''


@app.route("/")
def index():

    device_online = test_online()
    snap_revision = os.getenv('SNAP_REVISION', 'N/A')

    try:
        ssh_fingerprint = get_ssh_fingerprint()
    except Exception:
        logger.exception("Exception while getting SSH fingerprint")
        ssh_fingerprint = 'N/A'

    try:
        net_env = NetworkEnv()
        # An ugly-ish way to combine two dicts, in order to get the interface names on the same level
        network_interfaces = [{**v, **{'name': k}}
                              for k, v in net_env.interfaces.items()]
    except Exception:
        logger.exception("Exception while doing network scan")
        network_interfaces = []

    return render_template(
        'index.html',
        node_id=node_id,
        device_online=device_online,
        snap_revision=snap_revision,
        ssh_fingerprint=ssh_fingerprint,
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


@app.route("/wifi_ap")
def wifi_ap():
    args = dict(
        disabled=request.args.get('disabled', type=int)
    )

    logger.info(f"Arguments received: {args}")

    # Carry out disable/enable command if set
    if args['disabled'] == 1:
        kvs.set(keys.WIFI_AP_CONFIG, {'disabled': True})
    elif args['disabled'] == 0:
        kvs.set(keys.WIFI_AP_CONFIG, {'disabled': False})

    wifi_ap_available = kvs.get(keys.WIFI_AP_AVAILABLE)
    wifi_ap_cfg = kvs.get(keys.WIFI_AP_CONFIG)

    if wifi_ap_available:
        if wifi_ap_cfg is None:
            wifi_ap_cfg = {'disabled': False}
        elif not isinstance(wifi_ap_cfg, dict):
            wifi_ap_cfg = {'': 'Invalid configuration stored'}

    return render_template(
        'wifi_ap.html',
        node_id=node_id,
        wifi_ap_available=wifi_ap_available,
        wifi_ap_cfg=wifi_ap_cfg
    )


@app.route("/custom_actions")
def wifi_ap_status():
    args = dict(
        action=request.args.get('action', type=str)
    )

    logger.info(f"Arguments received: {args}")

    # Carry out action command if set
    if args['action'] == 'imt_sensor_address':
        from node_mgmt.commands import imt_sensor_address
        action_result = imt_sensor_address(None)
    else:
        action_result = None

    return render_template(
        'custom_actions.html',
        node_id=node_id,
        action_requested=args.get('action'),
        action_result=action_result
    )


def test_online():
    TEST_URL = 'https://www.ammp.io/'
    try:
        urlopen(TEST_URL, timeout=30)
        return True
    except Exception as e:
        logger.error(f"Error {e} while checking internet connectivity")
        return False
