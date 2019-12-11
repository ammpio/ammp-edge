# Set up logging
import logging
logging.basicConfig(format='%(name)s [%(levelname)s] %(message)s', level='INFO')
logger = logging.getLogger(__name__)


from flask import Flask, render_template
from node_mgmt import EnvScanner
import json
from db_model import NodeConfig

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
    return render_template('index.html', node_id=node_id)


@app.route("/env_scan")
def env_scan():
    scanner = EnvScanner()
    scan_result = scanner.do_scan()

    return render_template('env_scan.html', node_id=node_id, scan_result=scan_result)