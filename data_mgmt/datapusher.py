import logging
logger = logging.getLogger(__name__)

from config_mgmt import node_id, config, access_key

import time
from datetime import datetime
import threading
from .events import push_in_progress, do_shutdown

if config['remote']['type'] == 'api':
    import requests
elif config['remote']['type'] == 'influx':
    from influxdb import InfluxDBClient


class DataPusher(threading.Thread): 
    def __init__(self, queue): 
        threading.Thread.__init__(self)
        self.name = 'data_pusher'
        # Make sure this thread exits directly when the program exits; no clean-up should be required
        self.daemon = True

        self._queue = queue

    def run(self):

        while not do_shutdown.is_set():

            # queue.get() blocks the current thread until an item is retrieved
            logger.debug('PUSH: Waiting to get readings from queue')
            readout = self._queue.get() 

            # If we get the "stop" signal (i.e. empty dict) we exit
            if readout == {}:
                logger.debug('PUSH: Shutting down (got {} from queue)')
                return

            # Try pushing the readout to the remote endpoint
            try:
                push_in_progress.set()

                logger.debug('PUSH: Got readout at %s from queue; attempting to push' % (readout['time']))
                if push_readout(readout):
                    logger.info('PUSH: Successfully pushed point at %s' % (readout['time']))
                    push_in_progress.clear()

                else:
                    # For some reason the point wasn't written to Influx, so we should put it back in the file
                    logger.warn('PUSH: Did not work. Putting readout at %s back to queue' % readout['time'])
                    self._queue.put(readout)

                    push_in_progress.clear()

                    # Slow this down to avoid generating a high rate of errors if no connection is available
                    time.sleep(10)

            except Exception as ex:
                logger.exception('PUSH: Exception')

                push_in_progress.clear()

        logger.info('PUSH: Shutting down')



def push_readout(readout):

    try:
        readout['node_id'] = node_id

        # Append offset between time that reading was taken and current time
        readout['fields']['reading_offset'] = int((datetime.utcnow() - datetime.strptime(readout['time'], "%Y-%m-%dT%H:%M:%SZ")).total_seconds() - readout['fields'].get('reading_duration', 0))

        if config['remote']['type'] == 'api':
            # Push to API endpoint
            r = requests.post('https://%s/api/%s/nodes/%s/data' % (config['remote']['host'], config['remote']['apiver'], node_id),
                json=readout,
                headers={'Authorization': access_key},
                timeout=config['push_timeout'])
            result = r.status_code == 200
        elif config['remote']['type'] == 'influx':
            # Push to Influx database directly
            influx_client = InfluxDBClient(
                host = config['remote']['influx']['host'],
                port = config['remote']['influx']['port'],
                username = config['remote']['influx']['username'],
                password = config['remote']['influx']['password'],
                database = config['remote']['influx']['dbname'],
                ssl = True,
                verify_ssl = True,
                timeout = config['push_timeout'])

            result = influx_client.write_points([readout])

        if result:
            return True
        else:
            raise Exception('PUSH: Could not push point at %s. Error code %d' % (readout['time'], r.status_code))
    except Exception as ex:
        logger.exception('PUSH: Exception')
 
        return False