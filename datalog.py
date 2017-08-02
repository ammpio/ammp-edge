#!/usr/bin/env python3
# Copyright (c) 2017

import sys, os
from datetime import datetime
import json
import struct
from pyModbusTCP_alt import ModbusClient

class LoggerConfig(object):
    params = {}
    devices = {}
    readings = {}
    drivers = {}

    def __init__(self, pargs):
        self.params = {'debug': pargs['debug'],
                'interval': pargs['interval'],
                'timeout': pargs['timeout'],
                'outfile': pargs['outfile'],
                'roundtime': pargs['roundtime']
            }

        with open(pargs['devices']) as devices_file:
            self.devices = json.load(devices_file)

        with open(pargs['readings']) as readings_file:
            self.readings = json.load(readings_file)

        self.drivers = {}

        driver_files = [pos_json for pos_json in os.listdir(pargs['drvpath']) if pos_json.endswith('.json')]
        for drv in driver_files:
            with open(os.path.join(pargs['drvpath'], drv)) as driver_file:
                self.drivers[os.path.splitext(drv)[0]] = json.load(driver_file)
                if self.params['debug']:
                    print('Loaded driver %s' % (drv))


def reading_cycle(d, sc=None):
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
            sc.enterabs(roundtime(d), 1, reading_cycle, (d, sc))
        else:
            sc.enter(d.params['interval'], 1, reading_cycle, (d, sc))

    readout = get_readings(d)
    store_readings(d, readout)


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

    for dev in dev_rdg:
        if d.params['debug']:
            print('Start reading %s at %s' % (dev, str(datetime.utcnow())))

        readout['fields'].update(read_device(d, dev, dev_rdg[dev]))

        if d.params['debug']:
            print('Finished reading %s at %s' % (dev, str(datetime.utcnow())))

    return readout


def read_device(d, dev, readings):

    c = ModbusClient(
            host=d.devices[dev]['address']['host'],
            port=d.devices[dev]['address']['port'],
            unit_id=d.devices[dev]['address']['unit_id'],
            timeout=d.params['timeout'],
            debug=False, #d.params['debug'],
            auto_open=False,
            auto_close=False
        )

    fields = {}

    for rdg in readings:

        # Make sure we have an open connection to server
        if not c.is_open():
            if not c.open():
                print('Unable to connect to %s' % (dev))

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
                    print('READ: %s = %s %s' % (rdg['reading'], value, rdg['unit'] or ''))
            except:
                print('ERROR: Could not get reading %s' % (rdg['reading']))
                continue

    # Be nice and close the Modbus socket
    c.close()

    return fields


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


def store_readings(d, readout):
    with open(d.params['outfile'], 'a+') as outfile:
        json.dump(readout, outfile)
        outfile.write('\n')

def roundtime(d):
    tnow = time.time()
    next_roundtime = tnow + d.params['interval'] - (tnow % d.params['interval'])
    return next_roundtime


if __name__ == '__main__':
    import argparse

    parser = argparse.ArgumentParser()
    parser.add_argument('-D', '--devices', default='conf/devices.json', help='Device list file')
    parser.add_argument('-R', '--readings', default='conf/readings.json', help='Readings definition file')
    parser.add_argument('-P', '--drvpath', default='conf/drivers', help='Path containing drivers (device register maps)')
    parser.add_argument('-O', '--outfile', default='data/queue.json', help='Output data file/queue')
    parser.add_argument('-I', '--interval', type=int, help='Interval for repeated readings (s)')
    parser.add_argument('-r', '--roundtime', action='store_true', default=False, help='Start on round time interval (only with --interval)')    
    parser.add_argument('-t', '--timeout', type=int, default=5, help='Request timeout (s)')
    parser.add_argument('-d', '--debug', dest='debug', action='store_true', default=False, help='Debug mode')

    args = parser.parse_args()
    pargs = vars(args)

    d = LoggerConfig(pargs)

    if d.params['interval']:
        # We will be carrying out periodic readings (daemon mode)
        import sched, time
   
        # Set up scheduler and run reading cycle with schedule (reading_cycle function then schedules
        # its own further iterations)
        s = sched.scheduler(time.time, time.sleep)

        if d.params['roundtime']:
            s.enterabs(roundtime(d), 1, reading_cycle, (d, s))
            if d.params['debug']:
                print('Waiting to start on round time interval...')
        else:
            reading_cycle(d, s)

        s.run()

    else:
        # Carry out a one-off reading, with no scheduler
        reading_cycle(d)

