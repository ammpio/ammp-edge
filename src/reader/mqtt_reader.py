import logging
import paho.mqtt.client as mqtt
from time import sleep

logger = logging.getLogger(__name__)

CLIENT_ID = 'ammp-edge'
DEFAULT_QOS = 1

# A note on the reading logic; the approach implemented here does the following:
# 1. Upon initialization, it waits for `timeout` seconds for any payloads to come in
#    on subscribed topics. I.e. it collects data.
# 2. Upon carrying out a read(), it subscribes to the topic for that read, checks whether
#    data for a desired topic is available, and if so it returns it
#
# This means that during the very first reading cycle, no data is likely to be returned. But
# since the broker should remember this client ID and its subscriptions, any QoS 1 and QoS 2
# messages that are received in-between reading cycles should subsequently be buffered,
# and delivered upon the following reading cycle.


class Reader(object):
    def __init__(self, host: str = 'localhost', port: int = 1883, timeout: int = 3, **kwargs):

        self._host = host
        self._port = port
        # Note that the timeout is the time to wait for data, not for establishing a connection
        # A timeout for connection is not supported by the Paho MQTT library
        self._timeout = timeout

        self._client = mqtt.Client(client_id=CLIENT_ID, clean_session=False, **kwargs)

        self._client.enable_logger(logger=logger)

        # We store payloads from all topics in a dict. We need this since it's possible that
        # a payload comes in for a topic that's different to the one we're currently looking
        # to read, but we need to retain it so it can be returned as the result of another reading
        self._current_payloads = {}

    def __enter__(self):
        try:
            self._client.connect(self._host, port=self._port)
        except Exception:
            logger.error('Exception while attempting to connect to MQTT broker:')
            raise

        self._client.on_message = self.__on_message
        self._client.loop_start()

        sleep(self._timeout)

        return self

    def __exit__(self, type, value, traceback):
        try:
            self._client.disconnect()
        except Exception:
            logger.warning("Could not disconnect from MQTT broker", exc_info=True)

    def __on_message(self, client, userdata, msg):
        self._current_payloads[msg.topic] = msg.payload

    def read(self, topic, **rdg):
        res, _ = self._client.subscribe(topic, qos=DEFAULT_QOS)
        if res != mqtt.MQTT_ERR_SUCCESS:
            logger.error(f"Could not subscribe to topic '{topic}'. Result: {res}")
            return None

        return self._current_payloads.get(topic)
