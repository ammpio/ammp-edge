#!/usr/bin/env python3
# Copyright (c) 2017

# Set up logging
import logging
logging.basicConfig(format="%(asctime)s %(name)s %(levelname)-8s %(message)s", level="DEBUG")
logger = logging.getLogger(__name__)

# Try systemd, or fall back to stdout
try:
    from systemd.journal import JournalHandler
    logger.addHandler(JournalHandler())
    print('Logging to systemd journal')
except Exception as ex:
    logger.info('Systemd journal handler not available; logging to STDOUT', exc_info=True)


import sys, os
import argparse
from datetime import datetime
import json
import struct
import sched, time
import threading, queue
import signal

from reader import ModbusClient_alt
import minimalmodbus, serial

import requests

__version__ = '0.2.0'

from config_mgmt import node_id, config, drivers
from data_mgmt import *


def reading_cycle(q, sc=None):
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
        if config.get('read_roundtime'):
            sc.enterabs(roundtime(config['read_interval']), 1, reading_cycle, (q, sc))
        else:
            sc.enter(config['read_interval'], 1, reading_cycle, (q, sc))

    try:
        readout = get_readings()
        # Put the readout in the internal queue
        q.put(readout)
    
    except Exception as ex:
        logger.exception('READ: Exception getting readings')


def get_readings():

    # Work out all the readings that need to be taken, refactored by device
    dev_rdg = {}

    for rdg in config['readings']:
        # Ignore readings that are explicitly disabled
        # (if 'enabled' key is missing altogether, assume enabled by default)
        if not config['readings'][rdg].get('enabled', True): continue

        # Get device and variable name for reading; if not available then move on
        try:
            dev_id = config['readings'][rdg]['device']
            var = config['readings'][rdg]['var']
        except KeyError: continue

        # Ignore devices that are explicitly disabled in the devices configuration
        # (if 'enabled' key is missing altogether, assume enabled by default)
        if dev_id in config['devices']:
            dev = config['devices'][dev_id]
        else:
            logger.debug('Reading from device %s requested, but device not defined. Skipping' % dev_id)
            continue

        if not dev.get('enabled', True): continue

        # Get the driver name
        drv_id = dev['driver']

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
        rdict.update(drivers[drv_id].get('common', {}))
        rdict.update(drivers[drv_id]['fields'][var])

        dev_rdg[dev_id].append(rdict)

    # 'readout' is a dict formatted for insertion into InfluxDB (with 'time' and 'fields' keys)
    readout = {
        'time': datetime.utcnow().strftime("%Y-%m-%dT%H:%M:%SZ"),
        'fields': {}
    }

    # Set up queue in which to save readouts from the multiple threads that are reading each device
    readout_q = queue.Queue()
    jobs = []

    # Set up threads for reading each of the devices
    for dev_id in dev_rdg:
        dev = config['devices'][dev_id]
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
        j.join()

    # Get the results for each device and append them to the readout structure
    for j in jobs:
        fields = readout_q.get()
        readout['fields'].update(fields)

    readout['fields']['reading_duration'] = (datetime.utcnow() - datetime.strptime(readout['time'], "%Y-%m-%dT%H:%M:%SZ")).total_seconds()

    return readout


def read_device(dev, readings, readout_q):

    fields = {}

    logger.info('READ: Start reading %s' % dev['id'])

    # The reading type for each of the devices can be one of the following:
    # modbustcp - ModbusTCP
    # serial - RS-485 / ModbusRTU
    # snmp - SNMP

    if dev['reading_type'] == 'modbustcp':
        # Set up and read from ModbusTCP client

        try:
            c = ModbusClient_alt(
                host=dev['address']['host'],
                port=dev['address'].get('port', 502),
                unit_id=dev['address']['unit_id'],
                timeout=dev.get('timeout'),
                auto_open=False,
                auto_close=False
            )
        except:
            logger.exception('READ: Attempting to create ModbusTCP client raised exception')
            return fields

        for rdg in readings:

            # Make sure we have an open connection to server
            if not c.is_open():
                c.open()
                if c.is_open():
                    logger.debug('READ: [%s] Opened connection' % dev['id'])
                else:
                    logger.error('READ: [%s] Unable to open connection' % dev['id'])


            # If open() is ok, read register
            if c.is_open():
                try:
                    val_i = c.read_holding_registers(rdg['register'], rdg['words'])
                except Exception as ex:
                    logger.exception('READ: [%s] Could not obtain reading %s' % (dev['id'], rdg['reading']))

                    continue

                if val_i is None:
                    logger.warn('READ: [%s] Device returned None for reading %s' % (dev, rdg['reading']))
                    continue

                try:
                    # The pyModbusTCP library helpfully converts the binary result to a list of integers, so
                    # it's best to first convert it back to binary (assuming big-endian)
                    val_b = struct.pack('>%sH' % len(val_i), *val_i)

                    value = process_response(rdg, val_b)

                    # Append to key-value store            
                    fields[rdg['reading']] = value

                    logger.debug('READ: [%s] %s = %s %s' % (dev, rdg['reading'], value, rdg.get('unit', '')))
                except Exception as ex:
                    logger.exception('READ: [%s] Could not process reading %s. Exception' % (dev, rdg['reading']))
                    continue


        # Be nice and close the Modbus socket
        c.close()


    elif dev['reading_type'] == 'serial':

        # Set up RS-485 client
        c = minimalmodbus.Instrument(
            port=dev['address']['device'],
            slaveaddress=dev['address']['slaveaddr']
        )

#        c.serial.debug = pargs['debug']
        c.serial.timeout = dev.get('timeout')

        # Set up serial connection parameters according to device driver
        if 'serial' in dev:
            srlconf = dev['serial']
            
            c.serial.baudrate = dev['serial'].get('baudrate', 9600)
            c.serial.bytesize = dev['serial'].get('bytesize', 8)
            paritysel = {'none': serial.PARITY_NONE, 'odd': serial.PARITY_ODD, 'even': serial.PARITY_EVEN}
            c.serial.parity = paritysel[dev['serial'].get('parity', 'none')]
            c.serial.stopbits = dev['serial'].get('stopbits', 1)

        for rdg in readings:

            # Make sure we have an open connection to device
            if not c.serial.is_open:
                c.serial.open()
                if c.serial.is_open:
                    logger.debug('READ: Opened connection to %s' % dev['id'])
                else:
                    logger.error('READ: Unable to connect to %s' % dev['id'])

            # If connection is ok, read register
            if c.serial.is_open:
                try:
                    val_i = c.read_registers(rdg['register'], rdg['words'], rdg['fncode'])
                except Exception as ex:
                    logger.exception('READ: [%s] Could not process reading %s. Exception' % (dev['id'], rdg['reading']))
                    continue

                if val_i is None:
                    logger.warn('READ: [%s] Device returned None for reading %s' % (dev['id'], rdg['reading']))
                    continue

                try:
                    # The minimalmodbus library helpfully converts the binary result to a list of integers, so
                    # it's best to first convert it back to binary (assuming big-endian)
                    val_b = struct.pack('>%sH' % len(val_i), *val_i)

                    value = process_response(rdg, val_b)

                    # Append to key-value store            
                    fields[rdg['reading']] = value

                    logger.debug('READ: [%s] %s = %s %s' % (dev['id'], rdg['reading'], value, rdg.get('unit', '')))
                except Exception as ex:
                    logger.exception('READ: [%s] Could not process reading %s. Exception' % (dev['id'], rdg['reading']))
                    continue

        # Be nice and close the serial port between readings
        c.serial.close()

    elif dev['reading_type'] == 'snmp':

        from reader import SNMPReader

        with SNMPReader(host=dev['address']['host'], port=dev['address'].get('port'), community=dev['address'].get('community'), timeout=dev.get('timeout')) as reader:

            for rdg in readings:
                try:
                    val = reader.read(rdg['oid'])

                except Exception as ex:
                    logger.exception('READ: [%s] Could not obtain reading %s. Exception' % (dev['id'], rdg['reading']))
                    continue

                try:
                    value = reader.process(rdg, val)

                    # Append to key-value store            
                    fields[rdg['reading']] = value

                    logger.debug('READ: [%s] %s = %s %s' % (dev['id'], rdg['reading'], value, rdg.get('unit', '')))

                except Exception as ex:
                    logger.exception('READ: [%s] Could not process reading %s. Exception' % (dev['id'], rdg['reading']))
                    continue

    logger.info('READ: Finished reading %s' % dev['id'])

    # Append result to readings (alongside those from other devices)
    readout_q.put(fields)


def process_response(rdg, val_b):
    # Format identifiers used to unpack the binary result into desired format based on datatype
    fmt = {
        'int16':  'i',
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
        fmt_char = fmt_fallback(len(val_b))

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


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('-q', '--qfile', default='nvq.db', help='Queue file (for non-volatile storage during comms outage)')
    parser.add_argument('-l', '--logfile', type=str, help='Log file to use as fallback if systemd logging is not available')
    parser.add_argument('-d', '--debug', dest='debug', action='store_true', default=False, help='Debug mode')

    args = parser.parse_args()
    pargs = vars(args)

    # Set up logging parameters 
    if pargs['debug']:
        logger.setLevel(logging.DEBUG)

    if pargs['logfile']:
        fh = logging.FileHandler(pargs['logfile'])
        logger.addHandler(fh)

    # # Redirect stdout and stderr ro error file
    # sys.stdout = set_logger.StreamToLogger(logger, logging.INFO)
    # sys.stderr = set_logger.StreamToLogger(logger, logging.ERROR)

    # Handle SIGTERM from daemon control
    signal.signal(signal.SIGTERM, sigterm_handler)

    # Set up reading queue
    q = queue.LifoQueue()

    # Create an instance of the queue processor, and start the thread's internal run() method
    pusher = DataPusher(q)
    pusher.start() 


    if config.get('read_interval'):
        # We will be carrying out periodic readings (daemon mode)
   
        # Create an instance of the volatile<->non-volatile queue processor, and start it
        nvqproc = NonVolatileQProc(q)
        nvqproc.start()

        try:

            # Set up scheduler and run reading cycle with schedule (reading_cycle function then schedules
            # its own further iterations)
            s = sched.scheduler(time.time, time.sleep)

            if config.get('read_roundtime'):
                s.enterabs(roundtime(config['read_interval']), 1, reading_cycle, (q, s))
                logger.info('Waiting to start on round time interval...')
            else:
                reading_cycle(q, s)

            s.run()

        except (KeyboardInterrupt, SystemExit):
            do_shutdown.set()
            q.put({})            

    else:
        # Carry out a one-off reading, with no scheduler
        reading_cycle(q)
        q.put({})

if __name__ == '__main__':
    main()
