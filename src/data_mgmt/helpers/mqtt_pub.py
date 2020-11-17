import logging
from os import getenv, path
import json
import paho.mqtt.client as mqtt
from typing import Dict, List

logger = logging.getLogger(__name__)

MQTT_CLEAN_SESSION = False
MQTT_QOS = 1
MQTT_RETAIN = False
MQTT_CONN_SUCCESS = 0
MQTT_PUB_SUCCESS = 0

# Attempt to send (including waiting for PUBACK) at most 2 message at a time
MAX_INFLIGHT_MESSAGES = 2
# Only use the internal MQTT queue minimally
# (note that 0 = unlimited queue size, so 1 is the minimum)
MAX_QUEUED_MESSAGES = 2


class MQTTPublisher():
    def __init__(self, node_id: str, access_key: str, config: Dict) -> None:
        client = mqtt.Client(client_id=node_id, clean_session=MQTT_CLEAN_SESSION)
        client.enable_logger(logger)
        client.tls_set(
            ca_certs=path.join(getenv('SNAP', '.'), 'resources', 'certs', config['cert']))
        client.username_pw_set(node_id, access_key)
        client.max_inflight_messages_set(MAX_INFLIGHT_MESSAGES)
        client.max_queued_messages_set(MAX_QUEUED_MESSAGES)

        client.on_connect = self.__on_connect
        client.on_disconnect = self.__on_disconnect
        client.connect(host=config['host'], port=config['port'])
        client.loop_start()

        self._client = client
        self._host = config['host']
        self._node_id = node_id
        self._topic = self.__get_topic()
        self._connected = False

    def publish(self, payload: Dict) -> None:
        if not self._connected:
            logger.warning("MQTT client not yet connected; not publishing")
            return False

        rc = self._client.publish(
            self._topic,
            self.__get_mqtt_payload(payload),
            qos=MQTT_QOS, retain=MQTT_RETAIN
        )
        logger.debug(f"PUSH [mqtt] Broker response: {rc}")

        if rc[0] == MQTT_PUB_SUCCESS:
            logger.debug("PUSH [mqtt] Successful broker response")
            return True
        else:
            logger.debug("PUSH [mqtt] Error - Message not published")
            return False

    def __get_topic(self) -> str:
        mqtt_topic = f"a/{self._node_id}/data"
        return mqtt_topic

    @staticmethod
    def __get_mqtt_payload(payload: dict) -> str:
        mqtt_payload = json.dumps(payload, separators=(',', ':'))
        return mqtt_payload

    def __on_connect(self, client: mqtt.Client, userdata, flags, rc: List) -> None:
        # Callback for when the client receives a CONNACK response from the server.
        if rc == MQTT_CONN_SUCCESS:
            logger.info(f"Successfully connected to MQTT host {self._host}")
            self._connected = True
        else:
            logger.error(f"Connection attempt to {self._host} yielded result code {rc}")

    def __on_disconnect(self, client: mqtt.Client, userdata, rc: List) -> None:
        if rc == MQTT_CONN_SUCCESS:
            logger.info(f"Successfully disconnected to MQTT host {self._host}")
        else:
            logger.error(f"Disconnection from {self._host} with result code {rc}")
        self._connected = False
