import logging
import time
import arrow
import json
import threading
import requests
from copy import deepcopy
import os
from dotenv import load_dotenv
import paho.mqtt.client as mqtt
from influxdb import InfluxDBClient
from influxdb.exceptions import InfluxDBClientError
from influxdb.exceptions import InfluxDBServerError
from data_mgmt.helpers import convert_to_api_payload

logger = logging.getLogger(__name__)
dotenv_path = os.path.join(os.environ.get('SNAP_COMMON', '.'), '.env')
load_dotenv(dotenv_path)

if os.environ.get('MQTT_LEVEL'):
    try:
        mqtt_cert_path = os.path.join(os.getenv('SNAP_COMMON', './'), 'resources', 'ca-' + os.environ.get('MQTT_LEVEL') + '.crt')
    except Exception:
        logger.warning(f"Failed to set mqtt level to {os.environ['MQTT_LEVEL']}", exc_info=True)


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
            self._mqtt_session = mqtt.Client(client_id="ammp", clean_session=False, transport="tcp")
            logger.debug(f"MQTT attempting to connect with user: {self._node.node_id}, and pass: {self._node.access_key}")
            logger.debug(f"MQTT attempting to connect to: {self._dep['config']['host']}, on port: {self._dep['config']['port']}")
            logger.debug(f"MQTT attempting to connect with ca file: {mqtt_cert_path}")
            self._mqtt_session.tls_set(ca_certs=mqtt_cert_path)
            self._mqtt_session.username_pw_set(self._node.node_id, self._node.access_key)
            self._mqtt_session.connect(self._dep['config']['host'], port=self._dep['config']['port'])
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

                logger.debug(f"PUSH: [{self.dep_name}] Got readout at {readout['time']} from queue; attempting to push")
                if self.__push_readout(readout):
                    logger.info(f"PUSH: [{self.dep_name}] Successfully pushed point at {readout['time']}")
                    if self._is_default_endpoint:
                        self._node.events.push_in_progress.clear()

                else:
                    # For some reason the point wasn't pushed successfully, so we should put it back in the queue
                    logger.warning(f"PUSH: [{self.dep_name}] Did not work."
                                   f"Putting readout at {readout['time']} back to queue")
                    self._queue.put(readout)

                    if self._is_default_endpoint:
                        self._node.events.push_in_progress.clear()

                    # Slow this down to avoid generating a high rate of errors if no connection is available
                    time.sleep(self._node.config.get('push_throttle_delay', 10))

            except:
                logger.exception(f"PUSH: [{self.dep_name}] Unexpected exception while trying to push data")

                if self._is_default_endpoint:
                    self._node.events.push_in_progress.clear()

        logger.info(f"PUSH: [{self.dep_name}] Shutting down")

    def __push_readout(self, readout_to_push):
        # TODO: Use API object/session

        # This ensures that any modifications are only local to this function, and do not affect the original (in case
        # it needs to be pushed back into the queue)
        readout = deepcopy(readout_to_push)
        if self._dep.get('type') == 'api':
            # Push to API endpoint
            try:
                # Append offset between time that reading was taken and current time
                readout['reading_offset'] = int((arrow.utcnow() - arrow.get(readout['time'])).total_seconds() - readout['reading_duration'])
                # Transform the device-based readout to the older API format
                readout = convert_to_api_payload(readout, self._node.config['readings'])
                logger.debug(f"PUSH API ENDPOINT. Readout to push: {readout}")
            except:
                logger.exception('Could not construct final data payload to push')
                return False

            try:
                r = self._session.post(f"https://{self._dep['config']['host']}/api/{self._dep['config']['apiver']}/nodes/{self._node.node_id}/data",
                    json=readout, timeout=self._node.config.get('push_timeout') or self._dep['config'].get('timeout') or 120)
            except requests.exceptions.ConnectionError:
                logger.warning('Connection error while trying to push data at %s to API.' % readout['time'])
                return False
            except requests.exceptions.Timeout:
                logger.warning('Timeout error while trying to push data at %s to API.' % readout['time'])
                return False
            except:
                logger.warning('Exception while trying to push data at %s to API.' % readout['time'], exc_info=True)
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

        elif self._dep.get('type') == 'influxdb':
            try:
                # Append offset between time that reading was taken and current time
                readout['fields']['reading_offset'] = int(
                    (arrow.utcnow() - arrow.get(readout['time'])).total_seconds() - readout['fields'].get('reading_duration', 0)
                )

                # Set measurement where data should be written
                readout['measurement'] = self._dep['meta']['measurement']
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
            logger.debug(f"MQTT Attempting to push device-based readout: {readout_to_push}")
            pub = self._mqtt_session.publish(f"a/{self._node.node_id}/data", json.dumps(readout_to_push))
            logger.debug(f"MQTT result: {pub}")
            return True

        else:
            logger.warning(f"Data endpoint type '{self._dep.get('type')}' not recognized")





