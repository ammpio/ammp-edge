import logging
import time
import arrow
import json
import threading
import requests
from copy import deepcopy
import os

from influxdb import InfluxDBClient
from influxdb.exceptions import InfluxDBClientError
from influxdb.exceptions import InfluxDBServerError

from data_mgmt.helpers import convert_to_api_payload

from data_mgmt.helpers.mqtt_pub import MQTTPublisher

logger = logging.getLogger(__name__)


class DataPusher(threading.Thread):
    def __init__(self, node, queue, dep):
        threading.Thread.__init__(self)
        self.name = 'data_pusher'
        self.dep_name = dep.get('name') or 'unnamed'
        # Make sure this thread exits directly when the program exits; no clean-up should be required
        self.daemon = True
        self._node = node
        self._queue = queue
        self._dep = dep
        self._is_default_endpoint = dep.get('isdefault', False)
        if dep.get('type') == 'api':
            self._session = requests.Session()
            self._session.headers.update({'Authorization': self._node.access_key})
        elif dep.get('type') == 'influxdb':
            self._session = InfluxDBClient(**dep['client_config'])
        elif dep.get('type') == 'mqtt':
            self._session = MQTTPublisher(
                node_id=self._node.node_id,
                access_key=self._node.access_key,
                config=dep['config']
            )
        else:
            logger.warning(f"Data endpoint type '{dep.get('type')}' not recognized")

    def run(self):

        while not self._node.events.do_shutdown.is_set():

            # queue.get() blocks the current thread until an item is retrieved
            logger.debug(f"PUSH: [{self.dep_name}] Waiting to get readout from queue")
            readout = self._queue.get()
            # If we get the "stop" signal (i.e. empty dict) we exit
            if readout == {}:
                logger.debug(f"PUSH: [{self.dep_name}] Shutting down (got empty dict from queue)")
                return

            # Try pushing the readout to the remote endpoint
            try:
                if self._is_default_endpoint:
                    self._node.events.push_in_progress.set()

                logger.debug(f"PUSH: [{self.dep_name}] Got readout at {arrow.Arrow.fromtimestamp(readout['t'])}"
                             f"from queue; attempting to push")
                if self.__push_readout(readout):
                    logger.info(f"PUSH: [{self.dep_name}] Successfully pushed point at {arrow.Arrow.fromtimestamp(readout['t'])}")
                    if self._is_default_endpoint:
                        self._node.events.push_in_progress.clear()

                else:
                    # For some reason the point wasn't pushed successfully, so we should put it back in the queue
                    logger.warning(f"PUSH: [{self.dep_name}] Did not work. "
                                   f"Putting readout at {arrow.Arrow.fromtimestamp(readout['t'])} back to queue")
                    self._queue.put(readout)

                    if self._is_default_endpoint:
                        self._node.events.push_in_progress.clear()

                    # Slow this down to avoid generating a high rate of errors if no connection is available
                    time.sleep(self._node.config.get('push_throttle_delay', 10))

            except Exception:
                logger.exception(f"PUSH: [{self.dep_name}] Unexpected exception while trying to push data")

                if self._is_default_endpoint:
                    self._node.events.push_in_progress.clear()

        logger.info(f"PUSH: [{self.dep_name}] Shutting down")

    def __push_readout(self, readout_to_push) -> None:
        # TODO: Use API object/session

        # This ensures that any modifications are only local to this function, and do not affect the original (in case
        # it needs to be pushed back into the queue)

        readout = deepcopy(readout_to_push)
        if self._dep.get('type') == 'api':
            # Push to API endpoint
            try:
                # Transform the device-based readout to the older API format
                readout = convert_to_api_payload(readout, self._node.config['readings'])
                # Append offset between time that reading was taken and current time
                readout['fields']['reading_offset'] = int((arrow.utcnow() - arrow.get(readout['time'])).total_seconds()
                                                          - readout['fields']['reading_duration'])
                logger.debug(f"PUSH [API]. API-Based Readout: {readout}")
            except:
                logger.exception('Could not construct final data payload to push')
                return False

            try:
                API_URL = self._dep['config']['host']
                r = self._session.post(f"https://{API_URL}/api/{self._dep['config']['apiver']}/nodes/{self._node.node_id}/data",
                                       json=readout,
                                       timeout=self._node.config.get('push_timeout') or self._dep['config'].get('timeout') or 120)
            except requests.exceptions.ConnectionError:
                logger.warning('Connection error while trying to push data at %s to API.' % readout['t'])
                return False
            except requests.exceptions.Timeout:
                logger.warning('Timeout error while trying to push data at %s to API.' % readout['t'])
                return False
            except:
                logger.warning('Exception while trying to push data at %s to API.' % readout['t'], exc_info=True)
                return False

            if r.status_code != 200:
                logger.warning('Error code %d while trying to push data point at %s.' % (r.status_code, readout['t']))
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

        elif self._dep.get('type') == 'influxdb':
            try:
                # Append offset between time that reading was taken and current time
                readout['fields']['reading_offset'] = int(
                    (arrow.utcnow() - arrow.get(readout['t'])).total_seconds() - readout['fields'].get('reading_duration', 0)
                )

                # Set measurement where data should be written
                readout['measurement'] = self._dep['m']['measurement']
            except:
                logger.exception('Could not construct final data payload to push')
                return False

            r = None
            try:
                r = self._session.write_points([readout])
            except InfluxDBClientError as e:
                logger.error(f"InfluxDB client error: {e}")
            except InfluxDBServerError as e:
                logger.error(f"InfluxDB server error for {self._dep.get('client_config')}: {e}")
            except ConnectionRefusedError as e:
                logger.error(f"InfluxDB server at {self._dep.get('client_config')} not available: {e}")
            except:
                logger.exception(f"Could not write to InfluxDB at {self._dep.get('client_config')}")

            return r

        elif self._dep.get('type') == 'mqtt':
            # Append offset between time that reading was taken and current time
            readout['m']['reading_offset'] = int((arrow.utcnow() - arrow.get(readout['t'])).total_seconds() -
                                                    readout['m']['reading_duration'])
            logger.debug(f"PUSH [MQTT] Device-based readout: {readout}")
            return self._session.publish(readout)
        else:
            logger.warning(f"Data endpoint type '{self._dep.get('type')}' not recognized")
