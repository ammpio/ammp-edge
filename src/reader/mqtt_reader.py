import logging
import paho.mqtt.client as mqtt
from time import sleep

logger = logging.getLogger(__name__)

DEFAULT_CLIENT_ID = 'ammp-edge'
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

    def read(self, topic, **rdg):
        received_msg = None

        def on_message(client, userdata, msg):
            global received_msg
            received_msg = msg.payload

        res, _ = self._client.subscribe(topic)
        if res != mqtt.MQTT_ERR_SUCCESS:
            logger.error(f"Could not subscribe to topic '{topic}'. Result: {res}")
            return None

        self._client.on_message = on_message
        self._client.loop_start()

        num_iterations = self._timeout / READING_CHECK_INTERVAL
        for i in range(num_iterations):
            if received_msg is not None:
                break
            sleep(READING_CHECK_INTERVAL)

        return received_msg
