# Set up logging
import logging

from flask import Flask, render_template, request
from node_mgmt import NetworkEnv, SerialEnv, EnvScanner
from db_model import NodeConfig

logging.basicConfig(format='%(name)s [%(levelname)s] %(message)s', level='INFO')
logger = logging.getLogger(__name__)

app = Flask(__name__)


try:
    nodeconf = NodeConfig.get()
    node_id = nodeconf.node_id
except NodeConfig.DoesNotExist:
    logger.info('No node configuration found in internal database.')
    node_id = ''
except ValueError:
    logger.warning('ValueError in node config.', exc_info=True)
    node_id = ''


@app.route("/")
def index():

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

    return render_template('env_scan.html', node_id=node_id, scan_result=scan_result)


@app.route("/network_scan")
def net_scan():
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
