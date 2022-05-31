import logging
import os
from time import sleep
import serial
import minimalmodbus

import requests_unixsocket

from node_mgmt import EnvScanner
from node_mgmt.constants import (
    DEFAULT_SERIAL_DEV,
    DEFAULT_SERIAL_BAUD_RATE
)
from reader.modbusrtu_reader import Reader

logger = logging.getLogger(__name__)

MQTT_STATE_SUBTOPIC = 'state/env_scan'
GENERATE_NEW_CONFIG_FLAG = 'generate_new_config'


def snap_refresh(node):
    __snapd_socket_post({'action': 'refresh'})


def snap_switch_stable(node):
    __snapd_socket_post({'action': 'refresh', 'channel': 'stable'})


def snap_switch_candidate(node):
    __snapd_socket_post({'action': 'refresh', 'channel': 'candidate'})


def snap_switch_beta(node):
    __snapd_socket_post({'action': 'refresh', 'channel': 'beta'})


def snap_switch_edge(node):
    __snapd_socket_post({'action': 'refresh', 'channel': 'edge'})


def __snapd_socket_post(payload):
    try:
        with requests_unixsocket.Session() as s:
            res = s.post('http+unix://%2Frun%2Fsnapd.socket/v2/snaps/ammp-edge', json=payload)
        logger.info(f"Response from snapd API: Status {res.status_code} / {res.text}")
    except Exception:
        logger.exception('Exception while doing snapd API socket request')


def env_scan(node):
    logger.info('Starting environment scan')
    scanner = EnvScanner()
    scan_result = scanner.do_scan()
    logger.info('Completed environment scan. Submitting results to API and MQTT')
    node.api.post_env_scan(scan_result)
    if node.mqtt_client.publish(scan_result, subtopic=MQTT_STATE_SUBTOPIC):
        logger.info(f"ENV_SCAN [mqtt]: Successfully pushed")
    else:
        # For some reason the env_state wasn't pushed successfully
        logger.warning(f"ENV_SCAN [mqtt]: Push failed")


def trigger_config_generation(node):
    logger.info('Starting environment scan')
    scanner = EnvScanner()
    scan_result = scanner.do_scan()
    scan_result[GENERATE_NEW_CONFIG_FLAG] = True
    logger.info('Completed environment scan. Submitting results to MQTT Broker.')
    if node.mqtt_client.publish(scan_result, subtopic=MQTT_STATE_SUBTOPIC):
        logger.info(f"ENV_SCAN [mqtt]: Successfully pushed")
    else:
        # For some reason the env_state wasn't pushed successfully
        logger.warning(f"ENV_SCAN [mqtt]: Push failed")


def imt_sensor_address(node):
    # Note: unlike the other commands here, this one is normally triggered from the Web UI
    # Ideally this will be placed elsewhere in future, within a more systematic
    # action/command framework

    def readall(s):
        resp = b''
        while True:
            r = s.read()
            if len(r) > 0:
                resp += r
            else:
                break
        return resp

    ser = serial.Serial('/dev/ttyAMA0', baudrate=9600, timeout=5)
    result = {}

    logger.info("Changing address of IMT sensor from 1 to 2 (command 0x01460402630c")
    req = bytes.fromhex('01460402630c')
    ser.write(req)
    result['Address change response (expect 01460402630c'] = readall(ser).hex()

    logger.info("Restarting IMT sensor communications (command 0x010800010000b1cb")
    req = bytes.fromhex('010800010000b1cb')
    ser.write(req)
    result['Comms restart response (expect 010800010000b1cb'] = readall(ser).hex()

    logger.info("Testing sensor reading")
    try:
        with Reader('/dev/ttyAMA0', 2, baudrate=9600, debug=True) as r:
            result['Data read test from address 2'] = r.read(0, 1, 4)
    except Exception as e:
        result['Error'] = f"Exception: {e}"

    logger.info(f"Result: {result}")

    return result


def holykell_sensor_address_7(node):
    return _change_address_holykell(original_slave_id=1, target_slave_id=7)


def holykell_sensor_address_8(node):
    return _change_address_holykell(original_slave_id=1, target_slave_id=8)


def _change_address_holykell(original_slave_id: int, target_slave_id: int) -> dict:
    mod = minimalmodbus.Instrument(port=DEFAULT_SERIAL_DEV, slaveaddress=original_slave_id, debug=True)
    mod.serial.baudrate = DEFAULT_SERIAL_BAUD_RATE
    mod.serial.timeout = 3
    result = {}

    try:
        # check that no device is already on target slave address
        _read_holykell(mod, slave_id=target_slave_id)
    except minimalmodbus.NoResponseError:
        # no device detected on target slave, go ahead and set address
        result = _set_address_holykell(mod, result, original_slave_id=original_slave_id,
                                       target_slave_id=target_slave_id)
    except Exception as e:
        # if any other exception than NoResponseError is caught, the command must fail
        logger.warning(f'Failed to check if slave {target_slave_id} available. Exception {e}')
        result['Error'] = f'Failed to check if slave {target_slave_id} available. Exception {e}'
    else:
        logger.info(f'Slave ID {target_slave_id} already in use')
        result['Error'] = f'Other device already detected on slave {target_slave_id}'

    return result


def _set_address_holykell(mod: minimalmodbus.Instrument, result: dict,
                          original_slave_id: int, target_slave_id: int) -> dict:
    logger.info(f'Slave {target_slave_id} free, assigning the device to it')
    result[f'Check on slave {target_slave_id}'] = 'Slave available, assigning the device to it'
    try:
        # command to change slave ID
        mod.address = original_slave_id
        mod.write_register(80, target_slave_id, 0, 6)
        sleep(1)
        # command to save changes
        mod.address = target_slave_id
        mod.write_register(64, 49087, 0, 6)
        sleep(1)
        # confirmation that data can be read after change
        result[f'Success, fuel level read from slave {target_slave_id} (mm)'] = mod.read_registers(2, 1, 3)
        logger.info(f'Holykell successfully assigned to slave ID {target_slave_id}')
        return result
    except Exception as e:
        result['Error'] = f'Unable to assign slave ID to {target_slave_id}. Exception {e}'
        logger.warning(f'Unable to assign slave ID to {target_slave_id}. Exception {e}')
        return result


def _read_holykell(mod: minimalmodbus.Instrument, slave_id: int):
    mod.address = slave_id
    return mod.read_registers(registeraddress=0, number_of_registers=40, functioncode=3)


def sys_reboot(node):
    os.system(
        'busctl call org.freedesktop.login1 /org/freedesktop/login1 org.freedesktop.login1.Manager Reboot b "false"')


def sys_start_snapd(node):
    os.system(
        'busctl call org.freedesktop.systemd1 /org/freedesktop/systemd1 org.freedesktop.systemd1.Manager StartUnit ss "snapd.service" "replace"'
    )


def sys_stop_snapd(node):
    os.system(
        'busctl call org.freedesktop.systemd1 /org/freedesktop/systemd1 org.freedesktop.systemd1.Manager StopUnit ss "snapd.service" "replace"'
    )


def sys_remount_rw(node):
    os.system(
        'busctl call org.freedesktop.systemd1 /org/freedesktop/systemd1 org.freedesktop.systemd1.Manager StartUnit ss "remount-rw.service" "replace"'
    )


def sys_remount_ro(node):
    os.system(
        'busctl call org.freedesktop.systemd1 /org/freedesktop/systemd1 org.freedesktop.systemd1.Manager StopUnit ss "remount-rw.service" "replace"'
    )
