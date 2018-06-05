import logging
logger = logging.getLogger(__name__)

import time
import arrow
import json
import threading
import requests

from influxdb import InfluxDBClient


class DataPusher(threading.Thread): 
    def __init__(self, node, queue): 
        threading.Thread.__init__(self)
        self.name = 'data_pusher'
        # Make sure this thread exits directly when the program exits; no clean-up should be required
        self.daemon = True

        self._node = node
        self._queue = queue

    def run(self):

        while not self._node.events.do_shutdown.is_set():

            # queue.get() blocks the current thread until an item is retrieved
            logger.debug('PUSH: Waiting to get readings from queue')
            readout = self._queue.get() 

            # If we get the "stop" signal (i.e. empty dict) we exit
            if readout == {}:
                logger.debug('PUSH: Shutting down (got {} from queue)')
                return

            # Try pushing the readout to the remote endpoint
            try:
                self._node.events.push_in_progress.set()

                logger.debug('PUSH: Got readout at %s from queue; attempting to push' % (readout['time']))
                if self.__push_readout(readout):
                    logger.info('PUSH: Successfully pushed point at %s' % (readout['time']))
                    self._node.events.push_in_progress.clear()

                else:
                    # For some reason the point wasn't written to Influx, so we should put it back in the file
                    logger.warn('PUSH: Did not work. Putting readout at %s back to queue' % readout['time'])
                    self._queue.put(readout)

                    self._node.events.push_in_progress.clear()

                    # Slow this down to avoid generating a high rate of errors if no connection is available
                    time.sleep(self._node.config.get('push_throttle_delay', 10))

            except:
                logger.exception('PUSH: Exception')

                self._node.events.push_in_progress.clear()

        logger.info('PUSH: Shutting down')


    def __push_readout(self, readout):

        try:
            readout['meta'].update({'config_id': self._node.config['config_id']})

            # Append offset between time that reading was taken and current time
            readout['fields']['reading_offset'] = int((arrow.utcnow() - arrow.get(readout['time'])).total_seconds() - readout['fields'].get('reading_duration', 0))

            if self._node.remote.get('type') == 'api' or self._node.remote.get('type') is None:
                # Push to API endpoint
                r = requests.post('https://%s/api/%s/nodes/%s/data' % (self._node.remote['host'], self._node.remote['apiver'], self._node.node_id),
                    json=readout,
                    headers={'Authorization': self._node.access_key},
                    timeout=self._node.config.get('push_timeout', 120))
                result = r.status_code == 200

                try:
                    rtn = json.loads(r.text)
                except:
                    logger.warning('PUSH: API response "%s" could not be parsed as JSON' % r.text, exc_info=True)
                    rtn = {}

            elif self._node.remote.get('type') == 'influx':
                # Push to Influx database directly
                influx_client = InfluxDBClient(
                    host = self._node.remote['influx']['host'],
                    port = self._node.remote['influx']['port'],
                    username = self._node.remote['influx']['username'],
                    password = self._node.remote['influx']['password'],
                    database = self._node.remote['influx']['dbname'],
                    ssl = True,
                    verify_ssl = True,
                    timeout = self._node.config['push_timeout'])

                result = influx_client.write_points([readout])

            if rtn.get('newconfig'):
                logger.info('API response indicates new configuration is available. Requesting pull')
                self._node.events.check_new_config.set()

            if rtn.get('newcommand'):
                logger.info('API response indicates command is available. Triggering check')
                self._node.events.get_command.set()

            if result:
                return True
            else:
                raise Exception('PUSH: Could not push point at %s. Error code %d' % (readout['time'], r.status_code))
        except Exception:
            logger.exception('PUSH: Exception')
     
            return False