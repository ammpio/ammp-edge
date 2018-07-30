#!/usr/bin/env python3
# Copyright (c) 2018

# Set up logging
import logging
logging.basicConfig(format="%(asctime)s %(name)s [%(levelname)s] %(message)s", level="DEBUG")
logger = logging.getLogger(__name__)

# Try systemd, or fall back to stdout
# try:
#     from systemd.journal import JournalHandler
#     logger.addHandler(JournalHandler())
#     print('Logging to systemd journal')
# except Exception as ex:
#     logger.info('Systemd journal handler not available; logging to STDOUT')


import sys, os
import argparse
import arrow
import json
import struct
import sched, time
import threading, queue
import signal

from reader import ModbusClient_alt
#from reader import ModbusClient

import requests

__version__ = '0.7'

import node_mgmt
from data_mgmt import *

DEVICE_DEFAULT_TIMEOUT=30
DEVICE_READ_MAXTIMEOUT=600

def reading_cycle(node, q, sc=None):
    # Check if scheduler has been applied, and if so schedule this function to be run again
    # at the appropriate interval before taking the readings
    if sc:
        # If we want readings with round timestamps, schedule the next reading for the next such time
        # Otherwise schedule it 'interval' seconds from now. The latter (non-rounded) option
        # will lead to timestamp drift if any readings take longer than 'interval' (since the scheduler
        # won't start a new process until the current one has finished).
        # With the round-time option, any readings immediately following ones that take too long will have
        # non-round timestamps, but if possible ones following that should "catch up". That said,
        # drift can still accumulate and if it becomes greater than 'interval', a reading will be skipped.
        if node.config.get('read_roundtime'):
            sc.enterabs(roundtime(node.config['read_interval']), 1, reading_cycle, (node, q, sc))
        else:
            sc.enter(node.config['read_interval'], 1, reading_cycle, (node, q, sc))

    try:
        readout = get_readings(node)
        # Put the readout in the internal queue
        q.put(readout)
    
    except:
        logger.exception('READ: Exception getting readings')


def get_readings(node):

    # Work out all the readings that need to be taken, refactored by device
    dev_rdg = {}

    for rdg in node.config['readings']:
        # Ignore readings that are explicitly disabled
        # (if 'enabled' key is missing altogether, assume enabled by default)
        if not node.config['readings'][rdg].get('enabled', True): continue

        # Get device and variable name for reading; if not available then move on
        try:
            dev_id = node.config['readings'][rdg]['device']
            var = node.config['readings'][rdg]['var']
        except KeyError: continue

        # Ignore devices that are explicitly disabled in the devices configuration
        # (if 'enabled' key is missing altogether, assume enabled by default)
        if dev_id in node.config['devices']:
            dev = node.config['devices'][dev_id]
        else:
            logger.error('Reading from device %s requested, but device not defined. Skipping' % dev_id)
            continue

        if not dev.get('enabled', True): continue

        # Get the driver name
        drv_id = dev['driver']
        if not drv_id in node.drivers:
            logger.error('Reading using driver %s requested, but driver not found. Skipping device %s' % (drv_id, dev_id))
            continue

        # Save all necessary reading parameters in dev_rdg
        # dev_rdg is a dict of lists of dicts ;) :
        # 1st level: dict with the device name as the key (so we can query each device separately)
        # 2nd level: list of individual readings that need to be taken from device
        # 3rd level: for each reading, a dict determining how the reading should be taken
        if not dev_id in dev_rdg:
            dev_rdg[dev_id] = []

        # Start by setting reading name
        rdict = {'reading': rdg}
        # If applicable, add common reading parameters from driver file (e.g. function code)
        rdict.update(
            node.drivers[drv_id].get('common', {})
            )
 
        rdict.update(
            node.drivers[drv_id]['fields'].get(var, {})
            )

        dev_rdg[dev_id].append(rdict)

    # 'readout' is a dict formatted for insertion into InfluxDB (with 'time' and 'fields' keys)
    readout = {
        'time': arrow.utcnow().strftime('%Y-%m-%dT%H:%M:%SZ'),
        'fields': {},
        'meta': {}
    }

    try:
        readout['fields']['comms_lggr_snap_rev'] = int(os.getenv('SNAP_REVISION',0))
    except:
        logger.warn('Could not get snap revision number, or could not parse as integer', exc_info=True)

    # Set up queue in which to save readouts from the multiple threads that are reading each device
    readout_q = queue.Queue()
    jobs = []

    # Set up threads for reading each of the devices
    for dev_id in dev_rdg:
        dev = node.config['devices'][dev_id]
        dev.update({'id': dev_id})

        dev_thread = threading.Thread(
                target=read_device,
                name='Readout-' + dev_id,
                args=(dev, dev_rdg[dev_id], readout_q),
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

    readout['fields']['reading_duration'] = (arrow.utcnow() - arrow.get(readout['time'])).total_seconds()

    return readout


def read_device(dev, readings, readout_q):

    fields = {}

    logger.info('READ: Start reading %s' % dev['id'])

    logger.debug('Reading device %s' % dev)

    # The reading type for each of the devices can be one of the following:
    # modbustcp - ModbusTCP
    # serial - RS-485 / ModbusRTU
    # snmp - SNMP

    if dev['reading_type'] == 'modbustcp':
        # Set up and read from ModbusTCP client

        try:
#            c = ModbusClient(
            c = ModbusClient_alt(
                host=dev['address']['host'],
                port=dev['address'].get('port', 502),
                unit_id=dev['address']['unit_id'],
                timeout=dev.get('timeout', DEVICE_DEFAULT_TIMEOUT),
                auto_open=False,
                auto_close=False,
                debug=True
            )
        except:
            logger.exception('READ: Attempting to create ModbusTCP client raised exception')
            return

        for rdg in readings:

            # Make sure we have an open connection to server
            if not c.is_open():
                if dev.get('conn_check'):
                    # Do a quick ping check
                    r = os.system('ping -c 1 %s' % dev['address']['host'])
                    if r == 0:
                        logger.debug('Host %s appears to be up' % dev['address']['host'])
                    else:
                        logger.error('Unable to ping host %s' % dev['address']['host'])

                    # Do a quick TCP socket open check
                    import socket
                    sock = socket.socket()
                    try:
                        sock.connect((dev['address']['host'], dev['address']['port']))
                        logger.debug('Successfully opened test connection to %s:%s' % (dev['address']['host'], dev['address']['port']))
                    except:
                        logger.exception('Cannot open ModbusTCP socket on %s:%s' % (dev['address']['host'], dev['address']['port']))
                    finally:
                        sock.close()

                c.open(try_conn=dev.get('conn_retry', 10))
                if c.is_open():
                    logger.debug('READ: [%s] Opened connection' % dev['id'])
                else:
                    logger.error('READ: [%s] Unable to open connection' % dev['id'])

            logger.debug('Carrying out reading %s' % rdg)

            # If open() is ok, read register
            if c.is_open():
                try:
                    # If register is a string, assume that it's hex and convert to integer
                    # (having a "0x" prefix is acceptable but optional)
                    if type(rdg['register']) is str:
                        rdg['register'] = int(rdg['register'], 16)
                    
                    val_i = c.read_holding_registers(rdg['register'], rdg['words'])
                except:
                    logger.exception('READ: [%s] Could not obtain reading %s' % (dev['id'], rdg['reading']))

                    continue

                if val_i is None:
                    logger.warning('READ: [%s] Device returned None for reading %s' % (dev['id'], rdg['reading']))
                    continue

                try:
                    # The pyModbusTCP library helpfully converts the binary result to a list of integers, so
                    # it's best to first convert it back to binary. We assume big-endian order - unless 'order'
                    # parameter is set to 'lsr' = least significant register, in which case we reverse the order
                    # of the registers.
                    if rdg.get('order') == 'lsr':
                        val_i.reverse()

                    val_b = struct.pack('>%sH' % len(val_i), *val_i)

                    value = process_response(rdg, val_b)

                    # Append to key-value store            
                    fields[rdg['reading']] = value

                    logger.debug('READ: [%s] %s = %s %s' % (dev['id'], rdg['reading'], value, rdg.get('unit', '')))
                except:
                    logger.exception('READ: [%s] Could not process reading %s. Exception' % (dev['id'], rdg['reading']))
                    continue

        # Be nice and close the Modbus socket
        c.close()


    elif dev['reading_type'] == 'serial':

        from reader import SerialReader

        serialconf = dev['address']
        serialconf.update(dev.get('serial', {}))

        try:
            with SerialReader(**serialconf) as reader:
                for rdg in readings:
                    try:
                        value = reader.read(**rdg)

                    except:
                        logger.exception('READ: [%s] Could not obtain reading %s. Exception' % (dev['id'], rdg['reading']))
                        continue

                    # Append to key-value store            
                    fields[rdg['reading']] = value

                    logger.debug('READ: [%s] %s = %s %s' % (dev['id'], rdg['reading'], value, rdg.get('unit', '')))
        except:
            logger.exception('Exception while reading device %s' % dev['id'])


    elif dev['reading_type'] == 'sys':

        from reader import SysReader

        with SysReader() as reader:

            for rdg in readings:
                try:
                    val = reader.read(**rdg)

                except:
                    logger.exception('READ: [%s] Could not obtain reading %s. Exception' % (dev['id'], rdg['reading']))
                    continue

                # Assume no processing is necessary
                value = val

                # Append to key-value store            
                fields[rdg['reading']] = value

                logger.debug('READ: [%s] %s = %s %s' % (dev['id'], rdg['reading'], value, rdg.get('unit', '')))


    elif dev['reading_type'] == 'snmp':

        from reader import SNMPReader

        with SNMPReader(host=dev['address']['host'], port=dev['address'].get('port'), community=dev['address'].get('community'), timeout=dev.get('timeout', DEVICE_DEFAULT_TIMEOUT)) as reader:

            for rdg in readings:
                try:
                    value = reader.read(**rdg)

                except:
                    logger.exception('READ: [%s] Could not obtain reading %s. Exception' % (dev['id'], rdg['reading']))
                    continue

                # Append to key-value store            
                fields[rdg['reading']] = value

                logger.debug('READ: [%s] %s = %s %s' % (dev['id'], rdg['reading'], value, rdg.get('unit', '')))

    logger.info('READ: Finished reading %s' % dev['id'])

    # Append result to readings (alongside those from other devices)
    readout_q.put(fields)


def process_response(rdg, val_b):
    # Format identifiers used to unpack the binary result into desired format based on datatype
    fmt = {
        'int16':  'h',
        'uint16': 'H',
        'int32':  'i',
        'uint32': 'I',
        'float':  'f',
        'single': 'f',
        'double': 'd'
    }
    # If datatype is not available, fall back on format characters based on data length (in bytes)
    fmt_fallback = [None, 'B', 'H', None, 'I', None, None, None, 'd']

    # Check for defined value mappings in the driver
    # NOTE: The keys for these mappings must be HEX strings
    if 'valuemap' in rdg:
        # Get hex string representing byte reading (first method works in Pythin 3.5+)
        try:
            val_h = val_b.hex()
        except AttributeError:
            val_h = ''.join(format(b, '02x') for b in val_b)

        # If the value exists in the map, return 
        if val_h in rdg['valuemap']:
            return rdg['valuemap'][val_h]

    # Get the right format character to convert from binary to the desired data type
    if rdg.get('datatype') in fmt:
        fmt_char = fmt[rdg['datatype']]
    else:
        fmt_char = fmt_fallback[len(val_b)]

    # Convert
    value = struct.unpack('>%s' % fmt_char, val_b)[0]

    # Apply a float multiplier if desired
    if rdg.get('multiplier'):
        value = value * rdg['multiplier']

    return value


def roundtime(interval):
    tnow = time.time()
    next_roundtime = tnow + interval - (tnow % interval)
    return next_roundtime


def sigterm_handler(_signo, _stack_frame):
    logger.info('Received SIGTERM; shutting down')
    # Raises SystemExit(0):
    sys.exit(0)


class StreamToLogger(object):
    """
    Fake file-like stream object that redirects writes to a logger instance.
    """
    def __init__(self, logger, log_level=logging.INFO):
        self.logger = logger
        self.log_level = log_level
        self.linebuf = ''

    def write(self, buf):
        for line in buf.rstrip().splitlines():
            self.logger.log(self.log_level, line.rstrip())

    def flush(self):
        pass

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('-l', '--logfile', type=str, help='Log file to use as fallback if systemd logging is not available')
    parser.add_argument('-d', '--debug', dest='debug', action='store_true', default=False, help='Debug mode')

    args = parser.parse_args()
    pargs = vars(args)

    # Set up logging parameters 
    if pargs['debug']:
        logger.setLevel(logging.DEBUG)
    else:
        logger.setLevel(logging.INFO)    

    if pargs['logfile']:
        fh = logging.FileHandler(pargs['logfile'])
        logger.addHandler(fh)

    node = node_mgmt.Node()

    # Redirect stdout and stderr to error file
    sys.stdout = StreamToLogger(logger, logging.INFO)
    sys.stderr = StreamToLogger(logger, logging.ERROR)

    # Handle SIGTERM from daemon control
    signal.signal(signal.SIGTERM, sigterm_handler)

    # Set up reading queue
    q = queue.LifoQueue()

    # Create an instance of the queue processor, and start the thread's internal run() method
    pusher = DataPusher(node, q)
    pusher.start() 


    if node.config.get('read_interval'):
        # We will be carrying out periodic readings (daemon mode)
   
        # Create an instance of the volatile<->non-volatile queue processor, and start it
        nvqproc = NonVolatileQProc(node, q)
        nvqproc.start()

        try:

            # Set up scheduler and run reading cycle with schedule (reading_cycle function then schedules
            # its own further iterations)
            s = sched.scheduler(time.time, time.sleep)

            if node.config.get('read_roundtime'):
                s.enterabs(roundtime(node.config['read_interval']), 1, reading_cycle, (node, q, s))
                logger.info('Waiting to start on round time interval...')
            else:
                reading_cycle(node, q, s)

            s.run()

        except (KeyboardInterrupt, SystemExit):
            node.events.do_shutdown.set()
            q.put({})            

    else:
        # Carry out a one-off reading, with no scheduler
        reading_cycle(node, q)
        q.put({})

if __name__ == '__main__':
    main()
