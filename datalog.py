#!/usr/bin/env python3
# Copyright (c) 2017

import sys, os
from datetime import datetime
import json
import sqlite3
import struct
import sched, time
import threading, queue

from pyModbusTCP_alt import ModbusClient_alt
import minimalmodbus, serial

from influxdb import InfluxDBClient
import requests

import logging
import logging.handlers

do_shutdown = threading.Event()

class DatalogConfig(object):
    params = {}
    devices = {}
    readings = {}
    drivers = {}

    def __init__(self, pargs, logfile):
        self.params = pargs
        self.logfile = logfile

        with open(pargs['devices']) as devices_file:
            self.devices = json.load(devices_file)

        with open(pargs['readings']) as readings_file:
            self.readings = json.load(readings_file)

        with open(pargs['dbconf']) as dbconf_file:
            self.dbconf = json.load(dbconf_file)

        self.drivers = {}

        driver_files = [pos_json for pos_json in os.listdir(pargs['drvpath']) if pos_json.endswith('.json')]
        for drv in driver_files:
            with open(os.path.join(pargs['drvpath'], drv)) as driver_file:
                self.drivers[os.path.splitext(drv)[0]] = json.load(driver_file)
                logfile.info('Loaded driver %s' % (drv))


class DataPusher(threading.Thread): 
    def __init__(self, d, queue): 
        threading.Thread.__init__(self)
        self.name = 'data_pusher'
        # Make sure this thread exits directly when the program exits; no clean-up should be required
        self.daemon = True

        self._d = d
        self._queue = queue

    def run(self):

        while not do_shutdown.is_set():

            # queue.get() blocks the current thread until an item is retrieved
            self._d.logfile.debug('PUSH: Waiting to get readings from queue')
            readout = self._queue.get() 

            # If we get the "stop" signal we exit
            if readout == {}:
                self._d.logfile.debug('PUSH: Shutting down (got {} from queue)')
                return

            # Try pushing the readout to the remote endpoint
            try:
                self._d.logfile.debug('PUSH: Got readout at %s from queue; attempting to push' % (readout['time']))
                if push_readout(self._d, readout):
                    self._d.logfile.info('PUSH: Successfully pushed point at %s' % (readout['time']))
                else:
                    # For some reason the point wasn't written to Influx, so we should put it back in the file
                    d.logfile.warn('PUSH: Did not work. Putting readout at %s back to queue' % readout['time'])
                    self._queue.put(readout)

                    # Slow this down to avoid generating a high rate of errors if no connection is available
                    time.sleep(10)

            except Exception as ex:
                template = "An exception of type {0} occurred. Arguments:\n{1!r}"
                message = template.format(type(ex).__name__, ex.args)
                self._d.logfile.error('PUSH: %s' % message)

        self._d.logfile.info('PUSH: Shutting down')


class NonVolatileQProc(threading.Thread): 
    def __init__(self, d, queue): 
        threading.Thread.__init__(self)
        self.name = 'nvq_proc'
        # We want to get the chance to do clean-up on this thread if the program exits
        self.daemon = False

        self._d = d
        self._queue = queue

    def run(self):

        self._nvq = NonVolatileQ(self._d.params['qfile'])

        while not do_shutdown.is_set():

            qsize = self._queue.qsize()
            nvqsize = self._nvq.qsize()
            self._d.logfile.debug('NVQP: Queue size: internal: %d, non-volatile: %d' % (qsize, nvqsize))

            if qsize < 5 and nvqsize > 0:
                # If the internal queue is almost empty but the queue file isn't then pull from it
                readout = self._nvq.get()
                self._d.logfile.debug('NVQP: Got readout at %s from queue file; moving to internal queue' % (readout['time']))
                self._queue.put(readout)

                # Make sure we're not going way too fast
                time.sleep(1)

            elif qsize > 5:
                # If the internal queue is starting to grow large, then move items to the queue file
                readout = self._queue.get() 
                self._d.logfile.debug('NVQP: Got readout at %s from internal queue; moving to file' % (readout['time']))
                self._nvq.put(readout)

            else:
                # If the queue is "just right" then take is easy for a little while
                time.sleep(self._d.params.get('interval', 60)/2)

        self._d.logfile.info('NVQP: Stashing internal queue')
        # If we're exiting, then put all of the internal queue into the non-volatile queue
        while not self._queue.empty():
            readout = self._queue.get() 
            if not readout == {}:
                self._d.logfile.debug('NVQP: Got readout at %s from internal queue; moving to file' % (readout['time']))
                self._nvq.put(readout)

        self._d.logfile.info('NVQP: Shutting down')
        self._nvq.close()


class NonVolatileQ(object):
    def __init__(self, qfile):
        # Set up SQLite database connection for non-volatile queue storage
        # (file will be created if it doesn't already exist)
        self._qdb = sqlite3.connect(qfile)
        self._qdbc = self._qdb.cursor()

        # Start with a vacuum clean
        self._qdb.execute("VACUUM")

        # Pretty self-explanatory, but if the queue table doesn't exist it'll be created
        self._qdbc.execute("CREATE TABLE IF NOT EXISTS queue(id INTEGER PRIMARY KEY, item TEXT);")
        self._qdb.commit()

    def get(self):
        # Operate queue in LIFO fashion (obtain last inserted item)
        self._qdbc.execute("SELECT * FROM queue ORDER BY id DESC LIMIT 1;")

        lastrow = self._qdbc.fetchone()
        if lastrow:
            try:
                (lastid, item_str) = lastrow
                item = json.loads(item_str)

                self._qdbc.execute("DELETE FROM queue WHERE id = (?);", (lastid,))
                self._qdb.commit()

                return item
            except Exception as ex:
                template = "An exception of type {0} occurred. Arguments:\n{1!r}"
                message = template.format(type(ex).__name__, ex.args)
                self._d.logfile.error(message)
        else:
            return None

    def put(self, item):
        # We expect the item being returned to be a dict
        item_str = json.dumps(item)
        self._qdbc.execute("INSERT INTO queue(item) VALUES(?)", (item_str,))
        self._qdb.commit()

    def qsize(self):
        self._qdbc.execute("SELECT Count(*) FROM queue")
        qsize = self._qdbc.fetchone()[0]

        return qsize

    def close(self):
        self._qdb.close()


def setup_logfile(log_filename, debug_flag):

    if debug_flag:
        log_level = logging.DEBUG
    else:
        log_level = logging.INFO

    # Configure logging to log to a file, making a new file at midnight and keeping the last 7 day's data
    # Give the logger a unique name (good practice)
    logger = logging.getLogger(__name__)
    # Set the log level to LOG_LEVEL
    logger.setLevel(log_level)
    # Make a handler that writes to a file, making a new file at midnight and keeping 7 backups
    handler = logging.handlers.TimedRotatingFileHandler(log_filename, when="midnight", backupCount=7)
    # Format each log message like this
    formatter = logging.Formatter('%(asctime)s %(levelname)-8s %(message)s')
    # Attach the formatter to the handler
    handler.setFormatter(formatter)
    # Attach the handler to the logger
    logger.addHandler(handler)

    return logger


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


def reading_cycle(d, q, sc=None):
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
        if d.params['roundtime']:
            sc.enterabs(roundtime(d), 1, reading_cycle, (d, q, sc))
        else:
            sc.enter(d.params['interval'], 1, reading_cycle, (d, q, sc))

    readout = get_readings(d)

    # Put the readout in the internal queue
    q.put(readout)


def get_readings(d):

    # Work out all the readings that need to be taken, refactored by device
    dev_rdg = {}

    for rdg in d.readings:
        # Ignore readings that are explicitly disabled
        # (if 'enabled' key is missing altogether, assume enabled by default)
        if not d.readings[rdg].get('enabled', True): continue

        # Get device and variable name for reading; if not available then move on
        try:
            dev = d.readings[rdg]['device']
            var = d.readings[rdg]['var']
        except KeyError: continue

        # Ignore devices that are explicitly disabled in the devices.json file
        # (if 'enabled' key is missing altogether, assume enabled by default)
        if not dev in d.devices or not d.devices[dev].get('enabled', True): continue

        # Get the driver name
        drv = d.devices[dev]['driver']

        # Save all necessary reading parameters in dev_rdg
        # dev_rdg is a dict of lists of dicts ;) :
        # 1st level: dict with the device name as the key (so we can query each device separately)
        # 2nd level: list of individual readings that need to be taken from device
        # 3rd level: for each reading, a dict determining how the reading should be taken
        if not dev in dev_rdg:
            dev_rdg[dev] = []

        # Start by setting reading name
        rdict = {'reading': rdg}
        # If applicable, add common reading parameters from driver file (e.g. function code)
        rdict.update(d.drivers[drv].get('common', {}))
        rdict.update(d.drivers[drv]['fields'][var])

        dev_rdg[dev].append(rdict)

    # 'readout' is a dict formatted for insertion into InfluxDB (with 'time' and 'fields' keys)
    readout = {
        'time': datetime.utcnow().strftime("%Y-%m-%dT%H:%M:%SZ"),
        'fields': {}
    }

    # Set up queue in which to save readouts from the multiple threads that are reading each device
    readout_q = queue.Queue()
    jobs = []

    # Set up threads for reading each of the devices
    for dev in dev_rdg:
        dev_thread = threading.Thread(
                target=read_device,
                name='Readout-' + dev,
                args=(d, dev, dev_rdg[dev], readout_q),
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


def read_device(d, dev, readings, readout_q):

    fields = {}

    d.logfile.info('READ: Start reading %s' % dev)

    # The reading type for each of the devices can be one of the following:
    # 1 - ModbusTCP
    # 2 - RS-485 / ModbusRTU

    if d.devices[dev]['reading_type'] == 1:
        # Set up and read from ModbusTCP client

        c = ModbusClient_alt(
                host=d.devices[dev]['address']['host'],
                port=d.devices[dev]['address']['port'],
                unit_id=d.devices[dev]['address']['unit_id'],
                timeout=d.params['rtimeout'],
                debug=False, #d.params['debug'],
                auto_open=False,
                auto_close=False
            )

        for rdg in readings:

            # Make sure we have an open connection to server
            if not c.is_open():
                c.open()
                if c.is_open():
                    d.logfile.debug('READ: [%s] Opened connection' % dev)
                else:
                    d.logfile.error('READ: [%s] Unable to open connection' % dev)


            # If open() is ok, read register
            if c.is_open():
                try:
                    val_i = c.read_holding_registers(rdg['register'], rdg['words'])
                except Exception as ex:
                    d.logfile.error('READ: [%s] Could not obtain reading %s' % (dev, rdg['reading']))
                    template = "An exception of type {0} occurred. Arguments:\n{1!r}"
                    message = template.format(type(ex).__name__, ex.args)
                    d.logfile.error(message)

                if val_i is None:
                    d.logfile.warn('READ: [%s] Device returned None for reading %s' % (dev, rdg['reading']))
                    continue

                try:
                    # The pyModbusTCP library helpfully converts the binary result to a list of integers, so
                    # it's best to first convert it back to binary (assuming big-endian)
                    val_b = struct.pack('>%sH' % len(val_i), *val_i)

                    value = process_response(rdg, val_b)

                    # Append to key-value store            
                    fields[rdg['reading']] = value

                    d.logfile.debug('READ: [%s] %s = %s %s' % (dev, rdg['reading'], value, rdg.get('unit', '')))
                except Exception as ex:
                    d.logfile.error('READ: [%s] Could not process reading %s' % (dev, rdg['reading']))
                    template = "An exception of type {0} occurred. Arguments:\n{1!r}"
                    message = template.format(type(ex).__name__, ex.args)
                    d.logfile.error(message)
                    continue


        # Be nice and close the Modbus socket
        c.close()


    elif d.devices[dev]['reading_type'] == 2:

        # Set up RS-485 client
        c = minimalmodbus.Instrument(
            port=d.devices[dev]['address']['device'],
            slaveaddress=d.devices[dev]['address']['slaveaddr']
        )

        c.serial.debug = d.params['debug']
        c.serial.timeout = d.params['rtimeout']

        # Set up serial connection parameters according to device driver
        if 'serial' in d.drivers[d.devices[dev]['driver']]:
            srlconf = d.drivers[d.devices[dev]['driver']]['serial']
            
            c.serial.baudrate = srlconf.get('baudrate', 9600)
            c.serial.bytesize = srlconf.get('bytesize', 8)
            paritysel = {'none': serial.PARITY_NONE, 'odd': serial.PARITY_ODD, 'even': serial.PARITY_EVEN}
            c.serial.parity = paritysel[srlconf.get('parity', 'none')]
            c.serial.stopbits = srlconf.get('stopbits', 1)

        for rdg in readings:

            # Make sure we have an open connection to device
            if not c.serial.is_open:
                c.serial.open()
                if c.serial.is_open:
                    d.logfile.debug('READ: Opened connection to %s' % dev)
                else:
                    d.logfile.error('READ: Unable to connect to %s' % dev)

            # If connection is ok, read register
            if c.serial.is_open:
                try:
                    val_i = c.read_registers(rdg['register'], rdg['words'], rdg['fncode'])
                except Exception as ex:
                    d.logfile.error('READ: [%s] Could not obtain reading %s' % (dev, rdg['reading']))
                    template = "An exception of type {0} occurred. Arguments:\n{1!r}"
                    message = template.format(type(ex).__name__, ex.args)
                    d.logfile.error(message)

                if val_i is None:
                    d.logfile.warn('READ: [%s] Device returned None for reading %s' % (dev, rdg['reading']))
                    continue

                try:
                    # The minimalmodbus library helpfully converts the binary result to a list of integers, so
                    # it's best to first convert it back to binary (assuming big-endian)
                    val_b = struct.pack('>%sH' % len(val_i), *val_i)

                    value = process_response(rdg, val_b)

                    # Append to key-value store            
                    fields[rdg['reading']] = value

                    d.logfile.debug('READ: [%s] %s = %s %s' % (dev, rdg['reading'], value, rdg.get('unit', '')))
                except Exception as ex:
                    d.logfile.error('READ: [%s] Could not process reading %s' % (dev, rdg['reading']))
                    template = "An exception of type {0} occurred. Arguments:\n{1!r}"
                    message = template.format(type(ex).__name__, ex.args)
                    d.logfile.error(message)
                    continue

        # Be nice and close the serial port between readings
        c.serial.close()


    d.logfile.info('READ: Finished reading %s' % dev)

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
    if 'datatype' in rdg and rdg['datatype'] in fmt:
        fmt_char = fmt[rdg['datatype']]
    else:
        fmt_char = fmt_fallback(len(val_b))

    # Convert
    value = struct.unpack('>%s' % fmt_char, val_b)[0]

    # Apply a float multiplier if desired
    if 'multiplier' in rdg and rdg['multiplier']:
        value = value * rdg['multiplier']

    return value


def push_readout(d, readout):

    try:
        # Append measure and tag information to reading, to identify asset
        readout.update(d.dbconf['body'])

        # Append offset between time that reading was taken and current time
        readout['fields']['reading_offset'] = int((datetime.utcnow() - datetime.strptime(readout['time'], "%Y-%m-%dT%H:%M:%SZ")).total_seconds() - readout['fields'].get('reading_duration', 0))

        # Push to endpoint (own ingester or Influx, depending on type sent in dbconf)
        if d.dbconf['conn']['type'] == 'ingest':

            r = requests.post('https://%s/' % d.dbconf['conn']['host'],
                json=readout,
                headers={'X-API-Key': d.dbconf['conn']['key']},
                timeout=d.params['dbtimeout'])
            result = r.status_code == 200

        elif d.dbconf['conn']['type'] == 'influx':

            influx_client = InfluxDBClient(
                host = d.dbconf['conn']['host'],
                port = d.dbconf['conn']['port'],
                username = d.dbconf['conn']['username'],
                password = d.dbconf['conn']['password'],
                database = d.dbconf['conn']['dbname'],
                ssl = True,
                verify_ssl = True,
                timeout = d.params['dbtimeout'])

            result = influx_client.write_points([readout])

        if result:
            return True
        else:
            raise Exception('PUSH: Something didn''t go well for point at %s' % readout['time'])
    except Exception as ex:
        template = "An exception of type {0} occurred. Arguments:\n{1!r}"
        message = template.format(type(ex).__name__, ex.args)
        d.logfile.warn(message)
 
        return False


def roundtime(d):
    tnow = time.time()
    next_roundtime = tnow + d.params['interval'] - (tnow % d.params['interval'])
    return next_roundtime


if __name__ == '__main__':
    import argparse

    parser = argparse.ArgumentParser()
    parser.add_argument('-R', '--readings', default='conf/readings.json', help='Readings definition file')
    parser.add_argument('-D', '--devices', default='conf/devices.json', help='Device list file')
    parser.add_argument('-P', '--drvpath', default='conf/drivers', help='Path containing drivers (device register maps)')
    parser.add_argument('-B', '--dbconf', default='conf/dbconf.json', help='Output endpoint configuration spec file')
    parser.add_argument('-q', '--qfile', default='/tmp/datalog_queue.db', help='Queue file (for non-volatile storage during comms outage)')
    parser.add_argument('-l', '--logfile', default='/tmp/datalog.log', help='Log file')
    parser.add_argument('-I', '--interval', type=int, help='Interval for repeated readings (s)')
    parser.add_argument('-r', '--roundtime', action='store_true', default=False, help='Start on round time interval (only with --interval)')    
    parser.add_argument('-t', '--rtimeout', type=int, default=5, help='Modbus reading timeout (s)')
    parser.add_argument('-b', '--dbtimeout', type=int, default=120, help='Output endpoint timeout (s)')
    parser.add_argument('-d', '--debug', dest='debug', action='store_true', default=False, help='Debug mode')

    args = parser.parse_args()
    pargs = vars(args)

    # Set up logging and redirect stdout and stderr ro error file
    logfile = setup_logfile(pargs['logfile'], pargs['debug'])
    sys.stdout = StreamToLogger(logfile, logging.INFO)
    sys.stderr = StreamToLogger(logfile, logging.ERROR)

    # Set up configuration dict/structure
    d = DatalogConfig(pargs, logfile)

    # Set up reading queue
    q = queue.LifoQueue()

    # Create an instance of the queue processor, and start the thread's internal run() method
    pusher = DataPusher(d, q)
    pusher.start() 


    if d.params['interval']:
        # We will be carrying out periodic readings (daemon mode)
   
        # Create an instance of the volatile<->non-volatile queue processor, and start it
        nvqproc = NonVolatileQProc(d, q)
        nvqproc.start()

        try:

            # Set up scheduler and run reading cycle with schedule (reading_cycle function then schedules
            # its own further iterations)
            s = sched.scheduler(time.time, time.sleep)

            if d.params['roundtime']:
                s.enterabs(roundtime(d), 1, reading_cycle, (d, q, s))
                d.logfile.info('Waiting to start on round time interval...')
            else:
                reading_cycle(d, q, s)

            s.run()

        except (KeyboardInterrupt, SystemExit):
            do_shutdown.set()
            q.put({})            

    else:
        # Carry out a one-off reading, with no scheduler
        reading_cycle(d, q)
        q.put({})

    # Wait for queue to empty out (most likely by saving to non-volatile storage by NVQP thread)
#    q.join()
