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
                logger.exception('Unexpected exception while trying to push data')

                self._node.events.push_in_progress.clear()

        logger.info('PUSH: Shutting down')


    def __push_readout(self, readout):

        try:
            # Tag data with current config ID (TODO: consider whether this should be done when data is generated)
            readout['meta'].update({'config_id': self._node.config['config_id']})

            # Append offset between time that reading was taken and current time
            readout['fields']['reading_offset'] = int((arrow.utcnow() - arrow.get(readout['time'])).total_seconds() - readout['fields'].get('reading_duration', 0))
        except:
            logger.exception('Could not construct final data payload to push')

        # Push to API endpoint
        try:
            r = requests.post('https://%s/api/%s/nodes/%s/data' % (self._node.remote['host'], self._node.remote['apiver'], self._node.node_id),
                json=readout,
                headers={'Authorization': self._node.access_key},
                timeout=self._node.config.get('push_timeout', 120))
        except requests.exceptions.ConnectionError:
            logger.warning('Connection error while trying to push data at %s to API.' % readout['time'])
            return False
        except requests.exceptions.ConnectionError:
            logger.warning('Timeout error while trying to push data at %s to API.' % readout['time'])
            return False
        except:
            logger.warning('Error while trying to push data at %s to API.' % readout['time'], exc_info=True)
            return False

        if r.status_code != 200:
            logger.warning('Error code %d while trying to push data point at %s.' % (r.status_code, readout['time']))
            return False

        try:
            rtn = json.loads(r.text)
        except:
            logger.warning('API response "%s" could not be parsed as JSON' % r.text, exc_info=True)
            rtn = {}

        if rtn.get('newconfig'):
            logger.info('API response indicates new configuration is available. Requesting pull')
            self._node.events.check_new_config.set()

        if rtn.get('newcommand'):
            logger.info('API response indicates command is available. Triggering check')
            self._node.events.get_command.set()

        return True