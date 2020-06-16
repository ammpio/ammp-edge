import logging
import paho.mqtt.client as mqtt
from time import sleep

logger = logging.getLogger(__name__)

DEFAULT_CLIENT_ID = 'ammp-edge'
DEFAULT_QOS = 1
READING_CHECK_INTERVAL = 0.01


class Reader(object):
    def __init__(
            self,
            host: str = 'localhost',
            port: int = 1883,
            timeout: int = 30,
            **kwargs
            ):

        self._host = host
        self._port = port
        # Note that the timeout is the time to wait for data, not for a connection
        # A timeout for connection is not supported by the Paho MQTT library
        self._timeout = timeout

        self._client = mqtt.Client(
            client_id=DEFAULT_CLIENT_ID,
            clean_session=False,
            **kwargs)

        self._client.enable_logger(logger=logger)

    def __enter__(self):
        try:
            self._client.connect(self._host, port=self._port)
        except Exception:
            logger.error('Exception while attempting to connect to MQTT broker:')
            raise

        return self

    def __exit__(self, type, value, traceback):
        try:
            self._client.disconnect()
        except Exception:
            logger.warning("Could not disconnect from MQTT broker", exc_info=True)

    def __on_message(self, client, userdata, msg):
        self._last_msg = msg.payload

    def read(self, topic, **rdg):
        self._last_msg = None

        res, _ = self._client.subscribe(topic, qos=DEFAULT_QOS)
        if res != mqtt.MQTT_ERR_SUCCESS:
            logger.error(f"Could not subscribe to topic '{topic}'. Result: {res}")
            return None

        self._client.on_message = self.__on_message
        self._client.loop_start()

        num_iterations = round(self._timeout / READING_CHECK_INTERVAL)
        for i in range(num_iterations):
            if self._last_msg is not None:
                break
            sleep(READING_CHECK_INTERVAL)

        self._client.unsubscribe(topic)

        return self._last_msg
