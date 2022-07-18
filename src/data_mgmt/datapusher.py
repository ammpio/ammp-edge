import logging
from data_mgmt.helpers.mqtt_pub import MQTTPublisher

logger = logging.getLogger(__name__)

MQTT_CLIENT_ID_SUFFIX = 'data'


class DataPusher:
    def __init__(self, node):
        self._node = node

        self._session = MQTTPublisher(
            node_id=self._node.node_id,
            client_id_suffix=MQTT_CLIENT_ID_SUFFIX,
        )

    def push_readout(self, readout) -> bool:
        # Append offset between time that reading was taken and current time
        logger.debug(f"PUSH [mqtt] Readout: {readout}")
        return self._session.publish_data(readout)
