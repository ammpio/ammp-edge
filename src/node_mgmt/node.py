import logging
import json
import os

from kvstore import keys, KVStore
from edge_api import EdgeAPI
from node_mgmt.events import NodeEvents
from data_mgmt.helpers.mqtt_pub import MQTTPublisher

logger = logging.getLogger(__name__)

MQTT_CLIENT_ID_SUFFIX = 'meta'


class Node(object):

    def __init__(self) -> None:

        self._kvs = KVStore()

        self.node_id = self._kvs.get(keys.NODE_ID)
        self.access_key = self._kvs.get(keys.ACCESS_KEY)

        logger.info(f"Node ID: {self.node_id}")

        self.api = EdgeAPI(self.node_id, self.access_key)
        logger.info("Instantiated API")

        self.mqtt_client = MQTTPublisher(
            node_id=self.node_id,
            client_id_suffix=MQTT_CLIENT_ID_SUFFIX,
        )
        logger.info("Instantiated MQTT")

        self.events = NodeEvents()

        if self.config is not None:
            # Configuration is available in DB; use this
            logger.info('Using stored configuration from database')
        else:
            # Check for a provisioning configuration
            try:
                with open(os.path.join(os.getenv('SNAP', './'), 'provisioning', 'config.json'), 'r') as config_json:
                    config = json.load(config_json)
                    logger.info("Using configuration from provisioning file")
                    self.config = config
            except FileNotFoundError:
                logger.info("No provisioning config.json file found")
            except Exception:
                logger.exception("Exception while trying to process provisioning config.json")

        # Even if we loaded a stored config, check for a new one
        self.events.check_new_config.set()

        # Load drivers from files, and also add any from the config
        self.drivers = self.__get_drivers()
        self.update_drv_from_config()

    @property
    def config(self) -> dict:
        return self._kvs.get(keys.CONFIG)

    @config.setter
    def config(self, value) -> None:
        self._kvs.set(keys.CONFIG, value)

    @property
    def drivers(self) -> dict:
        return self._drivers

    @drivers.setter
    def drivers(self, value) -> None:
        self._drivers = value

    def __get_drivers(self) -> dict:

        drivers = {}

        drvpath = os.path.join(os.getenv('SNAP', './'), 'drivers')

        driver_files = [pos_json for pos_json in os.listdir(drvpath) if pos_json.endswith('.json')]
        for drv in driver_files:
            try:
                with open(os.path.join(drvpath, drv)) as driver_file:
                    drivers[os.path.splitext(drv)[0]] = json.load(driver_file)
                    logger.info('Loaded driver %s' % drv)
            except Exception:
                logger.error('Could not load driver %s' % drv, exc_info=True)

        return drivers

    def update_drv_from_config(self) -> None:
        """
        Check whether there are custom drivers in the config definition, and if so add them to the driver definition.
        """

        if self.config and 'drivers' in self.config:
            try:
                self.drivers.update(self.config['drivers'])
            except AttributeError:
                self.drivers = self.config['drivers']
