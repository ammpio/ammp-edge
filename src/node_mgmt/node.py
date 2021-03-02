import logging
import yaml
import json
import requests
import sys
import os
import time

from db_model import NodeConfig as LegacyNodeConfig
from kvstore import KVStore
import kvstore.keys as keys
from edge_api import EdgeAPI
from node_mgmt.events import NodeEvents
from node_mgmt.config_watch import ConfigWatch
from node_mgmt.command_watch import CommandWatch

logger = logging.getLogger(__name__)

# If activation is not successful, wait ACTIVATE_RETRY_DELAY seconds before retrying
ACTIVATE_RETRY_DELAY = 60


class Node(object):

    def __init__(self) -> None:

        self._kvs = KVStore()

        try:
            # Load base config from YAML file
            with open(os.path.join(os.getenv('SNAP', './'), 'remote.yaml'), 'r') as remote_yaml:
                remote = yaml.safe_load(remote_yaml)
                self.remote_api = remote['api']
                self.data_endpoints = remote.get('data-endpoints', [])
        except Exception:
            sys.exit(
                'Base configuration file remote.yaml cannot be loaded. Quitting')

        # If additional provisioning remote.yaml is available, load it also
        try:
            with open(os.path.join(os.getenv('SNAP', './'), 'provisioning', 'remote.yaml'), 'r') as p_remote_yaml:
                remote = yaml.safe_load(p_remote_yaml)
                if isinstance(remote.get('data-endpoints'), list):
                    self.data_endpoints.extend(remote['data-endpoints'])
                    logger.info(
                        f"Added {len(remote['data-endpoints'])} data endpoints from provisioning remote.yaml")
                else:
                    logger.info(
                        "No valid data-endpoints definition found in provisioning remote.yaml")
        except FileNotFoundError:
            logger.info("No provisioning remote.yaml found")
        except Exception:
            logger.exception(
                'Exception while trying to process provisioning remote.yaml')

        # Check if logger has been initialized
        if not self.node_id:
            logger.info('Checking for legacy config.db')
            legacy_config = self.__get_legacy_config()
            if legacy_config is not None:
                self.node_id = legacy_config.node_id
                self.access_key = legacy_config.access_key
                self.config = legacy_config.config
            else:
                logger.info(
                    'No node configuration found in internal database. Attempting node initialization')
                self.__initialize()

        logger.info('Node ID: %s', self.node_id)

        self.api = EdgeAPI(self)
        logger.info("Instantiated API")

        self.events = NodeEvents()
        config_watch = ConfigWatch(self)
        config_watch.start()

        command_watch = CommandWatch(self)
        command_watch.start()

        if self.config is None:
            # Check for a provisioning configuration
            provisioning_config = self.__get_provisioning_config()
            if provisioning_config is not None:
                self.config = provisioning_config

        # Even if we loaded a stored config, check for a new one
        self.events.check_new_config.set()

        # If we still have not got a config, wait for one to be provided
        if self.config is None:
            logger.info('No stored configuration available')
            with self.events.getting_config:
                self.events.getting_config.wait_for(
                    lambda: self.config is not None)

        # Load drivers from files, and also add any from the config
        self.drivers = self.__get_drivers()
        self.update_drv_from_config()

    @property
    def node_id(self) -> str:
        return self._kvs.get(keys.NODE_ID)

    @node_id.setter
    def node_id(self, value: str) -> bool:
        self._kvs.set(keys.NODE_ID, value)

    @property
    def config(self) -> dict:
        return self._kvs.get(keys.NODE_CONFIG)

    @config.setter
    def config(self, value: dict) -> bool:
        return self._kvs.set(keys.NODE_CONFIG, value)

    @property
    def access_key(self) -> str:
        return self._kvs.get(keys.ACCESS_KEY)

    @access_key.setter
    def access_key(self, value: str) -> bool:
        return self._kvs.set(keys.ACCESS_KEY, value)

    @property
    def remote_api(self) -> dict:
        return self._kvs.get(keys.REMOTE_API)

    @remote_api.setter
    def remote_api(self, value: dict) -> bool:
        return self._kvs.set(keys.REMOTE_API, value)

    @property
    def data_endpoints(self) -> dict:
        return self._kvs.get(keys.DATA_ENDPOINTS)

    @data_endpoints.setter
    def data_endpoints(self, value: list) -> bool:
        return self._kvs.set(keys.DATA_ENDPOINTS, value)

    @property
    def drivers(self) -> dict:
        return self._drivers

    @ drivers.setter
    def drivers(self, value) -> None:
        self._drivers = value

    def __initialize(self) -> None:
        node_id = self.__generate_node_id()
        logger.info('Generated node ID %s' % node_id)

        access_key = None
        while not access_key:
            access_key = self.__do_node_activation(node_id)
            if not access_key:
                logger.error(
                    'Unable to obtain access key. Retrying in %d seconds...' % ACTIVATE_RETRY_DELAY)
                time.sleep(ACTIVATE_RETRY_DELAY)

        # If there are existing saved configs (potentially for a different node_id, wipe them)
        self.__wipe_config()
        self.node_id = node_id
        self.access_key = access_key
        logger.debug('Saved new config for node ID %s' % node_id)

    def __generate_node_id(self) -> str:
        # Get ID (ideally hardware MAC address) that is used to identify logger when pushing data
        def get_hw_addr(ifname: str) -> str:
            import socket
            from fcntl import ioctl
            import struct

            with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as s:
                info = ioctl(s.fileno(), 0x8927, struct.pack(
                    '256s', bytes(ifname, 'utf-8')[:15]))

            return info[18:24].hex()

        node_id = None

        # First try to get the address of the primary Ethernet adapter
        ifn_wanted = ['eth0', 'en0', 'eth1',
                      'en1', 'em0', 'em1', 'wlan0', 'wlan1']
        for ifn in ifn_wanted:
            try:
                node_id = get_hw_addr(ifn)
            except Exception as e:
                logger.warn(
                    f"Could not get MAC address of interface {ifn}. Exception {e}")

            if node_id:
                break

        if not node_id:
            logger.warn(
                'Cannot find primary network interface MAC; trying UUID MAC')

            try:
                from uuid import getnode

                uuid_node = getnode()
                node_id = "{0:0{1}x}".format(uuid_node, 12)

            except Exception:
                logger.exception(
                    'Cannot get MAC via UUID method; generating random node ID starting with ff')

                # If that also doesn't work, generate a random 12-character hex string
                import random
                node_id = 'ff' + '%010x' % random.randrange(16**10)

        if node_id == 'd43639139e08':
            # This is a Moxa with a hardcoded MAC address. Need to generate something semi-random...
            logger.warning(
                'Generating semi-random ID for Moxa with hardcoded MAC')
            import random
            node_id = 'd43639' + ('%06x' % random.randrange(16**6))

        return node_id

    def __do_node_activation(self, node_id: str) -> str:

        # Initiate activation
        logger.info('Requesting activation for node %s' % node_id)

        try:
            r1 = requests.get(
                'https://%s/api/%s/nodes/%s/activate' % (
                    self.remote_api['host'], self.remote_api['apiver'], node_id)
            )
            rtn = json.loads(r1.text)

            if r1.status_code == 200:
                access_key = rtn['access_key']
                logger.info('Obtained API key')
                if rtn:
                    logger.debug('API response: %s' % rtn)
            else:
                logger.error(
                    'Error %d requesting activation from API' % r1.status_code)
                if rtn:
                    logger.debug('API response: %s' % rtn)
                return None
        except Exception:
            logger.exception(
                'Exception raised while requesting activation from API')
            return None

        # Confirm activation
        logger.info('Confirming activation for node %s' % node_id)

        try:
            r2 = requests.post(
                'https://%s/api/%s/nodes/%s/activate' % (
                    self.remote_api['host'], self.remote_api['apiver'], node_id),
                headers={'Authorization': access_key}
            )
            rtn = json.loads(r2.text)

            if r2.status_code == 200:
                logger.info('Confirmed activation')
                if rtn:
                    logger.debug('API response: %s' % rtn)
            else:
                logger.error(
                    'Error %d confirming activation with API' % r2.status_code)
                if rtn:
                    logger.debug('API response: %s' % rtn)
                return None
        except Exception:
            logger.exception(
                'Exception raised while confirming activation with API')
            return None

        return access_key

    def __get_drivers(self) -> dict:

        drivers = {}

        drvpath = os.path.join(os.getenv('SNAP', './'), 'drivers')

        driver_files = [pos_json for pos_json in os.listdir(
            drvpath) if pos_json.endswith('.json')]
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

        if 'drivers' in self.config:
            try:
                self.drivers.update(self.config['drivers'])
            except AttributeError:
                self.drivers = self.config['drivers']

    def __get_legacy_config(self):
        if LegacyNodeConfig._meta.database is None:
            return None

        try:
            return LegacyNodeConfig.get()
        except LegacyNodeConfig.DoesNotExist:
            return None

    def __get_provisioning_config(self):
        try:
            with open(os.path.join(os.getenv('SNAP', './'), 'provisioning', 'config.json'), 'r') as config_json:
                config = json.load(config_json)
                logger.info("Using configuration from provisioning file")
                return config
        except FileNotFoundError:
            logger.info("No provisioning config.json file found")
        except Exception:
            logger.exception(
                "Exception while trying to process provisioning config.json")

    def __wipe_config(self):
        self.node_id = None
        self.access_key = None
        self.config = None
