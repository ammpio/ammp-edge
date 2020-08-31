import logging

import os
import arrow
import time
import threading
import queue
from time import sleep
from copy import deepcopy

from processor import process_reading, get_output
from .helpers import set_host_from_mac, check_host_vs_mac

logger = logging.getLogger(__name__)

DEVICE_DEFAULT_TIMEOUT = 5
DEVICE_READ_MAXTIMEOUT = 600


def get_readings(node):

    # Work out all the readings that need to be taken, refactored by device
    dev_rdg = {}

    for rdg in node.config['readings']:
        # Ignore readings that are explicitly disabled
        # (if 'enabled' key is missing altogether, assume enabled by default)
        if not node.config['readings'][rdg].get('enabled', True):
            continue

        # Get device and variable name for reading; if not available then move on
        try:
            dev_id = node.config['readings'][rdg]['device']
            var = node.config['readings'][rdg]['var']
        except KeyError:
            continue

        # Ignore devices that are explicitly disabled in the devices configuration
        # (if 'enabled' key is missing altogether, assume enabled by default)
        if dev_id in node.config['devices']:
            dev = node.config['devices'][dev_id]
        else:
            logger.error('Reading from device %s requested, but device not defined. Skipping' % dev_id)
            continue

        if not dev.get('enabled', True):
            continue

        # Get the driver name
        drv_id = dev['driver']
        if drv_id not in node.drivers:
            logger.error(f"Reading using driver {drv_id} requested, but driver not found. Skipping device {dev_id}")
            continue

        # Save all necessary reading parameters in dev_rdg
        # dev_rdg is a dict of lists of dicts ;) :
        # 1st level: dict with the device name as the key (so we can query each device separately)
        # 2nd level: list of individual readings that need to be taken from device
        # 3rd level: for each reading, a dict determining how the reading should be taken
        if dev_id not in dev_rdg:
            dev_rdg[dev_id] = []

        # Start by setting reading name
        rdict = {'reading': rdg, 'var': var}
        # If applicable, add common reading parameters from driver file (e.g. function code)
        rdict.update(
            node.drivers[drv_id].get('common', {})
            )

        try:
            rdict.update(
                node.drivers[drv_id]['fields'][var]
                )
        except KeyError:
            logger.warn(f"Variable {var} not found in driver {drv_id}, or driver definition malformed.")

        dev_rdg[dev_id].append(rdict)

    return dev_rdg

def get_readout(node):
    # 'readout' is a dict formatted for insertion into InfluxDB (with 'time' and 'fields' keys)
    readout = {
        'time': arrow.utcnow().strftime('%Y-%m-%dT%H:%M:%SZ'),
        'fields': {},
        'meta': {}
    }
    dev_rdg = get_readings(node)

    try:
        readout['fields']['comms_lggr_snap_rev'] = int(os.getenv('SNAP_REVISION', 0))
    except Exception:
        logger.warn('Could not get snap revision number, or could not parse as integer', exc_info=True)

    # Set up queue in which to save readouts from the multiple threads that are reading each device
    readout_q = queue.Queue()
    jobs = []

    # Sometimes multiple "devices" will actually share the same serial port, or host IP.
    # It is best to make sure that multiple threads do not try to open concurrent
    # connections to a single port or host; in the case of a serial port at least, this
    # is bound to fail. Therefore we can create a lock for each physical port or host,
    # and ensure that the reading thread respects this lock and waits for any others
    # that are reading from the device to finish before proceeding.

    # First we need to establish the actual set of physical devices, and create a
    # lock object for each.
    locks = {}

    for dev_id, dev in node.config['devices'].items():
        if 'address' in dev:
            # Get the device or host name if available
            d = dev['address'].get('device') or dev['address'].get('host') or dev['address'].get('mac')

            # Create a lock for this device or host name if it doesn't already exist
            if d and d not in locks:
                locks[d] = threading.Lock()

            # Set host IP based on MAC, if MAC is available
            set_host_from_mac(dev['address'])

    # Set up threads for reading each of the devices
    for dev_id in dev_rdg:
        dev = node.config['devices'][dev_id]
        dev.update({'id': dev_id})

        try:
            d = dev['address'].get('device') or dev['address'].get('host') or dev['address'].get('mac')
            dev_lock = locks[d]
        except KeyError:
            dev_lock = None

        dev_thread = threading.Thread(
                target=read_device,
                name='Readout-' + dev_id,
                args=(dev, dev_rdg[dev_id], readout_q, dev_lock),
                daemon=True
                )

        jobs.append(dev_thread)

    # Start each of the device reading jobs
    for j in jobs:
        j.start()

    # Wait until all of the reading jobs have completed
    for j in jobs:
        j.join(timeout=DEVICE_READ_MAXTIMEOUT)

    # Get the results for each device and append them to the readout structure
    for j in jobs:
        try:
            fields = readout_q.get(block=False)
            readout['fields'].update(fields)
        except queue.Empty:
            logger.warning('Not all devices returned readings')

    readout['fields']['reading_duration'] = \
        (arrow.utcnow() - arrow.get(readout['time'])).total_seconds()

    logger.debug(f"Device readings: {dev_rdg}")
    logger.debug(f"Readout: {readout}")

    if 'output' in node.config:
        # Get additional processed values (new model to be used for all readings in future)
        output_fields = get_output(dev_rdg, node.config['output'])
        logger.debug(f"Output fields: {output_fields}")
        readout['fields'].update(output_fields)

    return readout


def read_device(dev, readings, readout_q, dev_lock=None):

    # If the device has a concurrency lock associated with it, make sure it's available
    if dev_lock:
        dev_lock.acquire()
        # If we've just finished reading another device on this port, let it breathe
        time.sleep(0.5)

    fields = {}

    logger.info('READ: Start reading %s' % dev['id'])

    logger.debug('Reading device %s' % dev)

    # The reading type for each of the devices can be one of the following:
    # modbustcp - ModbusTCP
    # modbusrtu or serial - RS-485 / ModbusRTU
    # rawserial - Raw serial request
    # snmp - SNMP

    if dev['reading_type'] == 'modbustcp':
        reader_config = deepcopy(dev['address'])
        reader_config['timeout'] = dev.get('timeout', DEVICE_DEFAULT_TIMEOUT)
        from reader.modbustcp_reader import Reader

    elif dev['reading_type'] == 'modbusrtu' or dev['reading_type'] == 'serial':
        reader_config = deepcopy(dev['address'])
        reader_config['timeout'] = dev.get('timeout', DEVICE_DEFAULT_TIMEOUT)
        from reader.modbusrtu_reader import Reader

    elif dev['reading_type'] == 'rawserial':
        reader_config = deepcopy(dev['address'])
        reader_config['timeout'] = dev.get('timeout', DEVICE_DEFAULT_TIMEOUT)
        from reader.rawserial_reader import Reader

    elif dev['reading_type'] == 'rawtcp':
        reader_config = deepcopy(dev['address'])
        reader_config['timeout'] = dev.get('timeout', DEVICE_DEFAULT_TIMEOUT)
        from reader.rawtcp_reader import Reader

    elif dev['reading_type'] == 'snmp':
        reader_config = deepcopy(dev['address'])
        reader_config['timeout'] = dev.get('timeout', DEVICE_DEFAULT_TIMEOUT)
        from reader.snmp_reader import Reader

    elif dev['reading_type'] == 'mqtt':
        reader_config = deepcopy(dev['address'])
        reader_config['timeout'] = dev.get('timeout', DEVICE_DEFAULT_TIMEOUT)
        from reader.mqtt_reader import Reader

    elif dev['reading_type'] == 'sys':
        reader_config = {}
        from reader.sys_reader import Reader

    try:
        with Reader(**reader_config) as reader:
            if not reader:
                raise Exception(f"No reader object could be created for device {dev['id']}. Skipping")

            if 'address' in dev and not check_host_vs_mac(dev['address']):
                raise Exception(f"MAC mismatch for {dev['id']}. Not reading device.")

            for rdg in readings:
                if 'read_delay' in dev and isinstance(dev['read_delay'], (float, int)):
                    sleep(dev['read_delay'])

                try:
                    val_b = reader.read(**rdg)
                    if val_b is None:
                        logger.warning('READ: [%s] Returned None for reading %s' % (dev['id'], rdg['reading']))
                        continue

                except Exception:
                    logger.exception('READ: [%s] Could not obtain reading %s. Exception' % (dev['id'], rdg['reading']))
                    continue

                # Get processed value
                value = process_reading(val_b, **rdg)

                # Append to key-value store
                fields[rdg['reading']] = value

                # Also save within readings structure
                rdg['value'] = value

                logger.debug('READ: [%s] %s = %s %s' % (dev['id'], rdg['reading'], repr(val_b), rdg.get('unit', '')))

    except Exception:
        logger.exception('Exception while reading device %s' % dev['id'])

    logger.info('READ: Finished reading %s' % dev['id'])

    # Append result to readings (alongside those from other devices)
    readout_q.put(fields)

    # If the device has a concurrency lock associated with it, release it
    # so that other threads can proceed with reading
    if dev_lock:
        dev_lock.release()
