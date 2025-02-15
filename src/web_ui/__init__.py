# Set up logging
import datetime
import logging
import os
import socket

from flask import Flask, render_template, request

from kvstore import KVCache, KVStore, keys
from node_mgmt import EnvScanner, NetworkEnv, Node, get_ssh_fingerprint
from node_mgmt.commands import (
    holykell_sensor_address_7,
    holykell_sensor_address_8,
    imt_sensor_address,
    trigger_config_generation,
)

logging.basicConfig(format="%(name)s [%(levelname)s] %(message)s", level="INFO")
logger = logging.getLogger(__name__)

app = Flask(__name__)

kvs = KVStore()

ACTIONS = {
    "imt_sensor_address": imt_sensor_address,
    "holykell_sensor_address_7": holykell_sensor_address_7,
    "holykell_sensor_address_8": holykell_sensor_address_8,
}

node_id = kvs.get(keys.NODE_ID)
if not node_id:
    logger.info("No node configuration found in internal database.")
    node_id = "Not yet initialized"


@app.route("/")
def index():

    device_online = test_online()
    snap_revision = os.getenv("SNAP_REVISION", "N/A")

    try:
        ssh_fingerprint = get_ssh_fingerprint()
    except Exception:
        logger.exception("Exception while getting SSH fingerprint")
        ssh_fingerprint = "N/A"

    try:
        net_env = NetworkEnv()
        # An ugly-ish way to combine two dicts, in order to get the interface names on the same level
        network_interfaces = [{**v, **{"name": k}} for k, v in net_env.interfaces.items()]
    except Exception:
        logger.exception("Exception while doing network scan")
        network_interfaces = []

    return render_template(
        "index.html",
        node_id=node_id,
        device_online=device_online,
        snap_revision=snap_revision,
        ssh_fingerprint=ssh_fingerprint,
        network_interfaces=network_interfaces,
    )


@app.route("/env-scan", methods=["GET", "POST"])
def env_scan():
    template = "env_scan.html"
    scan_result = None
    tank_dimensions = None
    if request.method == "GET":
        try:
            scanner = EnvScanner()
            scan_result = scanner.do_scan()
        except Exception as e:
            logger.exception("Exception while doing scan")
            return f"Error: {e}"
        return render_template(template, node_id=node_id, scan_result=scan_result)
    elif request.method == "POST":
        tank_input_option = request.form["showTankInputOptions"]
        if tank_input_option == "yes":
            try:
                width = float(request.form["width"])
                length = float(request.form["length"])
                height = float(request.form["height"])
            except ValueError:
                width, length, height = None, None, None
            if not all([width, length, height]):
                return render_template(
                    template,
                    node_id=node_id,
                    status={"code": "ERROR", "desc": "Please input valid numbers in all Tank Dimensions fields"},
                )
            tank_dimensions = {"width": width, "length": length, "height": height}

        trigger_config_generation(Node(), tank_dimensions)
        return render_template(
            template,
            node_id=node_id,
            status={
                "code": "SUCCESS",
                "desc": "Your request has been submitted successfully!",
                "info": "It might take a few minutes for the config to be generated. <br>After that, you can verify the\
                    config at <a href='/configuration'>View Configuration</a>",
            },
            scan_result=scan_result,
        )


@app.route("/configuration")
def configuration():
    config = kvs.get(keys.CONFIG)
    if config is not None:
        devices = config.get("devices", {})
        config_ts = config.get("timestamp")
    else:
        devices = {}
        config_ts = None
    return render_template("configuration.html", node_id=node_id, devices=devices, timestamp=config_ts)


@app.route("/realtime-readings")
def realtime_readings():
    is_loaded = False
    timestamp = None
    with KVCache() as kvc:
        device_readings = kvc.get(keys.LAST_READINGS)
        last_reading_ts = kvc.get(keys.LAST_READINGS_TS)
        if last_reading_ts is not None:
            timestamp = datetime.datetime.fromtimestamp(last_reading_ts)
        if device_readings is not None:
            is_loaded = True

    return render_template(
        "realtime_readings.html", node_id=node_id, readings=device_readings, is_loaded=is_loaded, timestamp=timestamp
    )


@app.route("/network-scan")
def network_scan():
    interface = request.args.get("interface")

    try:
        net_env = NetworkEnv(default_ifname=interface)
        network_scan_hosts = net_env.network_scan()
        # Grab it from the object, in case the interface name was overridden
        interface = net_env.default_ifname
    except Exception as e:
        logger.exception("Exception while doing network scan")
        return f"Error: {e}"

    return render_template(
        "network_scan.html", node_id=node_id, interface=interface, network_scan_hosts=network_scan_hosts
    )


@app.route("/wifi-ap")
def wifi_ap():
    args = dict(disabled=request.args.get("disabled", type=int))

    logger.info(f"Arguments received: {args}")

    # Carry out disable/enable command if set
    if args["disabled"] == 1:
        kvs.set(keys.WIFI_AP_CONFIG, {"disabled": True})
    elif args["disabled"] == 0:
        kvs.set(keys.WIFI_AP_CONFIG, {"disabled": False})

    wifi_ap_available = kvs.get(keys.WIFI_AP_AVAILABLE)
    wifi_ap_cfg = kvs.get(keys.WIFI_AP_CONFIG)

    if wifi_ap_available:
        if wifi_ap_cfg is None:
            wifi_ap_cfg = {"disabled": False}
        elif not isinstance(wifi_ap_cfg, dict):
            wifi_ap_cfg = {"": "Invalid configuration stored"}

    return render_template(
        "wifi_ap.html", node_id=node_id, wifi_ap_available=wifi_ap_available, wifi_ap_cfg=wifi_ap_cfg
    )


@app.route("/custom-actions")
def wifi_ap_status():
    args = dict(action=request.args.get("action", type=str))

    logger.info(f"Arguments received: {args}")

    # Carry out action command if set
    if args["action"] in ACTIONS:
        action_result = ACTIONS[args["action"]]()
    else:
        action_result = {"Error": "Unknown action"}

    return render_template(
        "custom_actions.html", node_id=node_id, action_requested=args.get("action"), action_result=action_result
    )


def test_online():
    TEST_HOST_PORT = ("edge.ammp.io", 80)
    try:
        conn = socket.create_connection(TEST_HOST_PORT, timeout=15)
        conn.close()
        return True
    except Exception as e:
        logger.error(f"Error {e} while checking internet connectivity")
        return False
