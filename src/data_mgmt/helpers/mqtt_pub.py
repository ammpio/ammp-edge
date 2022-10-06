import logging
import json
from os import getenv
import paho.mqtt.client as mqtt
from random import randrange
from typing import Dict, List, Optional

logger = logging.getLogger(__name__)

MQTT_HOST = getenv('MQTT_BRIDGE_HOST', 'localhost')
MQTT_PORT = 1883

MQTT_CLEAN_SESSION = False
MQTT_QOS = 1
MQTT_RETAIN = False
MQTT_CONN_SUCCESS = 0
MQTT_PUB_TIMEOUT = 5

MQTT_DATA_TOPIC = 'u/data'


class MQTTPublisher():
    def __init__(self, node_id: str, client_id_suffix: Optional[str] = None) -> None:
        if client_id_suffix is None:
            client_id = f'{node_id}-{"%06x" % randrange(16**6)}'
        else:
            client_id = f'{node_id}-{client_id_suffix}-{"%06x" % randrange(16**6)}'
        client = mqtt.Client(client_id=client_id, clean_session=MQTT_CLEAN_SESSION)
        client.enable_logger(logger)

        client.on_connect = self.__on_connect
        client.on_disconnect = self.__on_disconnect
        client.connect(host=MQTT_HOST, port=MQTT_PORT)
        client.loop_start()

        self._client = client
        self._connected = False

    def publish(self, payload: Dict, topic: str) -> bool:
        try:
            self._client.publish(
                topic,
                self.__get_mqtt_payload(payload),
                qos=MQTT_QOS, retain=MQTT_RETAIN
            ).wait_for_publish(timeout=MQTT_PUB_TIMEOUT)
        except RuntimeError:
            # Client is likely not connected (see wait_for_publish() docs)
            logger.exception('Publish unsuccessful; attempting to reconnect')
            self._client.reconnect()
            return False
        logger.debug(f"MQTT message published successfully")
        return True

    def publish_data(self, payload: Dict) -> bool:
        return self.publish(payload, MQTT_DATA_TOPIC)

    @staticmethod
    def __get_mqtt_payload(payload: dict) -> str:
        mqtt_payload = json.dumps(payload, separators=(',', ':'))
        return mqtt_payload

    def __on_connect(self, client: mqtt.Client, userdata, flags, rc: List) -> None:
        # Callback for when the client receives a CONNACK response from the server.
        if rc == MQTT_CONN_SUCCESS:
            logger.info("Successfully connected to MQTT broker")
            self._connected = True
        else:
            logger.error(f"Connection attempt to broker yielded result code {rc}")

    def __on_disconnect(self, client: mqtt.Client, userdata, rc: List) -> None:
        if rc == MQTT_CONN_SUCCESS:
            logger.info(f"Successfully disconnected to MQTT broker")
        else:
            logger.error(f"Disconnection from broker with result code {rc}")
        self._connected = False
