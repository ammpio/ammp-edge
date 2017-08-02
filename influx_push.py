#!/usr/bin/env python3
# Copyright (c) 2017

import sys, os
import time
import json
from influxdb import InfluxDBClient

class PushConfig(object):
    dbconf = {}

    def __init__(self, pargs):
        self.params = {'debug': pargs['debug'],
                'timeout': pargs['timeout'],
                'infile': pargs['infile']
            }

        with open(pargs['dbconf']) as dbconf_file:
            self.dbconf = json.load(dbconf_file)

def push_point(d, c):
    # Read last line of input file
    with open(d.params['infile'], 'r+b') as f:
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

    if d.params['debug']:
        print(lastline)

    try:
        point = json.loads(lastline)

        # Append measure and tag information to reading, to identify asset
        point.update(d.dbconf['body'])

        # Push to Influx
        if c.write_points([point]):
            print('Successfully pushed point at %s' % (point['time']))
            return True
        else:
            raise Exception('Something didn''t go well for point at %s' % (point['time']))
    except:
        # For one reason or another the point wasn't written to Influx, so we should put it back in the file
        print('Did not work. Writing line back to file')
        with open(d.params['infile'], 'a') as f:
            f.write(lastline)

    return False


if __name__ == '__main__':
    import argparse

    parser = argparse.ArgumentParser()
    parser.add_argument('-D', '--dbconf', default='conf/dbconf.json', help='InfluxDB configuration spec file')
    parser.add_argument('-I', '--infile', default='data/queue.json', help='Input data file/queue')
    parser.add_argument('-t', '--timeout', type=int, default=300, help='Request timeout (s)')
    parser.add_argument('-d', '--debug', dest='debug', action='store_true', default=False, help='Debug mode')

    args = parser.parse_args()
    pargs = vars(args)

    d = PushConfig(pargs)

    c = InfluxDBClient(
        host = d.dbconf['conn']['host'],
        port = d.dbconf['conn']['port'],
        username = d.dbconf['conn']['username'],
        password = d.dbconf['conn']['password'],
        database = d.dbconf['conn']['dbname'],
        ssl = True,
        verify_ssl = True,
        timeout = d.params['timeout'])

    while 1:
        while os.path.getsize(d.params['infile']) > 0:
            push_point(d, c)
            time.sleep(1)

        time.sleep(10)

