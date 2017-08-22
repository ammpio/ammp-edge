#!/usr/bin/env python3
# Copyright (c) 2017

import sys, os
from datetime import datetime
import json
import struct
import sched, time
import threading, queue
from pyModbusTCP_alt import ModbusClient_alt
from influxdb import InfluxDBClient


class LoggerConfig(object):
    params = {}
    devices = {}
    readings = {}
    drivers = {}

    def __init__(self, pargs):
        self.params = {'debug': pargs['debug'],
                'interval': pargs['interval'],
                'qfile': pargs['qfile'],
                'rtimeout': pargs['rtimeout'],
                'dbtimeout': pargs['dbtimeout'],
                'roundtime': pargs['roundtime']
            }

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
                if self.params['debug']:
                    print('Loaded driver %s' % (drv))


class InfluxPusher(threading.Thread): 
    def __init__(self, d, queue): 
        threading.Thread.__init__(self)
        self._d = d
        self._queue = queue

    def run(self):

        client = InfluxDBClient(
            host = self._d.dbconf['conn']['host'],
            port = self._d.dbconf['conn']['port'],
            username = self._d.dbconf['conn']['username'],
            password = self._d.dbconf['conn']['password'],
            database = self._d.dbconf['conn']['dbname'],
            ssl = True,
            verify_ssl = True,
            timeout = self._d.params['dbtimeout'])

        while True: 
            # Don't go too fast (in case we're just recovering from a data outage and there's a lot in the queue)
            time.sleep(1)

            # If the internal queue is empty but the queue file isn't then pull from it
            if self._queue.empty() and os.path.isfile(d.params['qfile']) and os.path.getsize(d.params['qfile']) > 1:
                readout = get_readout_from_file(d)
                # push_readout includes a function to write back to file if the push is not successful
                push_readout(d, client, readout)

                continue

            # queue.get() blocks the current thread until 
            # an item is retrieved. 
            readout = self._queue.get() 

            # If there is a readout, push it to the database; if False or None, we break
            if readout:
                push_readout(self._d, client, readout)
            else:
                break



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

    # If the internal queue is already busy, push straight to the queue file
    if q.qsize() < 10:
        q.put(readout)
    else:
        save_readout_to_file(d, readout)


def get_readings(d):

    # Work out all the readings that need to be taken, refactored by device
    dev_rdg = {}

    for rdg in d.readings:
        # Ignore readings that are explicitly disabled
        # (if 'enabled' key is missing altogether, assume enabled by default)
        if 'enabled' in d.readings[rdg] and not d.readings[rdg]['enabled']: continue

        # Get device and variable name for reading; if not available then move on
        try:
            dev = d.readings[rdg]['device']
            var = d.readings[rdg]['var']
        except KeyError: continue

        # Ignore devices that are explicitly disabled in the devices.json file
        # (if 'enabled' key is missing altogether, assume enabled by default)
        if 'enabled' in d.devices[dev] and not d.devices[dev]['enabled']: continue

        # Get the driver name
        drv = d.devices[dev]['driver']

        # Save all necessary reading parameters in dev_rdg
        # dev_rdg is a dict of lists of dicts ;) :
        # 1st level: dict with the device name as the key (so we can query each device separately)
        # 2nd level: list of individual readings that need to be taken from device
        # 3rd level: for each reading, a dict determining how the reading should be taken
        if not dev in dev_rdg:
            dev_rdg[dev] = []

        rdict = {'reading': rdg}
        rdict.update(d.drivers[drv][var])

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
                args=(d, dev, dev_rdg[dev], readout_q)
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

    c = ModbusClient_alt(
            host=d.devices[dev]['address']['host'],
            port=d.devices[dev]['address']['port'],
            unit_id=d.devices[dev]['address']['unit_id'],
            timeout=d.params['rtimeout'],
            debug=False, #d.params['debug'],
            auto_open=False,
            auto_close=False
        )

    fields = {}

    if d.params['debug']:
        print('READ: Start reading %s at %s' % (dev, str(datetime.utcnow())))

    for rdg in readings:

        # Make sure we have an open connection to server
        if not c.is_open():
            if not c.open():
                print('READ ERROR: Unable to connect to %s' % dev)

        # If open() is ok, read register
        if c.is_open():
            val_i = c.read_holding_registers(rdg['register'], rdg['words'])
            ##### Need to insert error checking to make sure we get something sensible back.
            ##### E.g. None is a possibility if there is an issue with the data returned by the server
            ##### For the moment we use a clumsy try-except in order to not get thrown off by bad readings

            try:
                # The pyModbusTCP library helpfully converts the binary result to a list of integers, so
                # it's best to first convert it back to binary (assuming big-endian)
                val_b = struct.pack('>%sH' % len(val_i), *val_i)

                value = process_response(rdg, val_b)

                # Append to key-value store            
                fields[rdg['reading']] = value

                if d.params['debug']:
                    print('READ: [%s] %s = %s %s' % (dev, rdg['reading'], value, rdg['unit'] or ''))
            except:
                print('READ ERROR: Could not get reading %s' % rdg['reading'])
                continue

    # Be nice and close the Modbus socket
    c.close()

    if d.params['debug']:
        print('READ: Finished reading %s at %s' % (dev, str(datetime.utcnow())))

    # Append result to readings (alongside those from other devices)
    readout_q.put(fields)


def process_response(rdg, val_b):
    # Format identifiers used to unpack the binary result into desired format based on datatype
    fmt = {
        'int16': 'i',
        'int32': 'i',
        'uint32': 'I'
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


def push_readout(d, client, readout):
    # Read last line of input file
    # if d.params['debug']:
    #     print(readout)

    try:
        # Append measure and tag information to reading, to identify asset
        readout.update(d.dbconf['body'])

        # Append offset between time that reading was taken and current time
        readout['fields']['reading_offset'] = int((datetime.utcnow() - datetime.strptime(readout['time'], "%Y-%m-%dT%H:%M:%SZ")).total_seconds() - readout['fields']['reading_duration'])

        # Push to Influx
        if client.write_points([readout]):
            print('PUSH: Successfully pushed point at %s' % (readout['time']))
            return True
        else:
            raise Exception('PUSH: Something didn''t go well for point at %s' % readout['time'])
    except Exception as ex:
        template = "An exception of type {0} occurred. Arguments:\n{1!r}"
        message = template.format(type(ex).__name__, ex.args)
        print(message)
        # For some reason the point wasn't written to Influx, so we should put it back in the file
        print('PUSH: Did not work. Writing readout at %s to queue file instead' % readout['time'])
        save_readout_to_file(d, readout)
 
        return False


def save_readout_to_file(d, readout):
    with open(d.params['qfile'], 'a+') as qfile:
        json.dump(readout, qfile)
        qfile.write('\n')


def get_readout_from_file(d):
    with open(d.params['qfile'], 'r+b') as f:
        f.seek(-2, os.SEEK_END)     # Jump to the second last byte.
        while f.read(1) != b'\n':   # Until EOL is found or we're at the start of the file
            if f.tell() < 2:
                f.seek(0, os.SEEK_SET) # We're basically at the start of the file - just jump back one byte to the actual start
                break
            else:
                f.seek(-2, os.SEEK_CUR) # Jump back the read byte plus one more
        
        # Remember where the start of the last line is, and read it
        lastline_start = f.tell()
        lastline = str(f.readline(), 'utf-8')
        # Truncate the file at the start of the last line
        f.truncate(lastline_start)

    readout = json.loads(lastline)

    return readout



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
    parser.add_argument('-B', '--dbconf', default='conf/dbconf.json', help='InfluxDB configuration spec file')
    parser.add_argument('-q', '--qfile', default='data/queue.json', help='Queue file (for non-volatile storage during comms outage')
    parser.add_argument('-I', '--interval', type=int, help='Interval for repeated readings (s)')
    parser.add_argument('-r', '--roundtime', action='store_true', default=False, help='Start on round time interval (only with --interval)')    
    parser.add_argument('-t', '--rtimeout', type=int, default=5, help='Modbus reading timeout (s)')
    parser.add_argument('-b', '--dbtimeout', type=int, default=120, help='Influx request timeout (s)')
    parser.add_argument('-d', '--debug', dest='debug', action='store_true', default=False, help='Debug mode')

    args = parser.parse_args()
    pargs = vars(args)

    d = LoggerConfig(pargs)

    q = queue.LifoQueue()

    # Create an instance of the queue processor
    pusher = InfluxPusher(d, q)
    # Start calls the internal run() method to kick off the thread
    pusher.start() 


    if d.params['interval']:
        # We will be carrying out periodic readings (daemon mode)
   
        # Set up scheduler and run reading cycle with schedule (reading_cycle function then schedules
        # its own further iterations)
        s = sched.scheduler(time.time, time.sleep)

        if d.params['roundtime']:
            s.enterabs(roundtime(d), 1, reading_cycle, (d, q, s))
            if d.params['debug']:
                print('Waiting to start on round time interval...')
        else:
            reading_cycle(d, q, s)

        s.run()

    else:
        # Carry out a one-off reading, with no scheduler
        reading_cycle(d, q)
        q.put(False)

