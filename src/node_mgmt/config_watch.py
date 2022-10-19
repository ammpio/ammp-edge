import hashlib
import json
import logging
import time
from threading import Thread

logger = logging.getLogger(__name__)

# If API endpoint doesn't return config, wait API_RETRY_DELAY seconds before retrying
API_RETRY_DELAY = 10
# Even if this is not explicitly requested, carry out a configuration check every CONFIG_REFRESH_DELAY seconds
CONFIG_REFRESH_DELAY = 3600


def get_digest(obj: dict, length: int = 7) -> str:
    s = json.dumps(obj, sort_keys=True).encode('utf-8')
    h = hashlib.sha1(s).hexdigest()
    return h[:length]


class ConfigWatch(Thread):
    """Request new configuration for node if flag is set"""

    def __init__(self, node):
        Thread.__init__(self)
        self.name = 'config_watch'
        # Make sure this thread exits directly when the program exits; no clean-up should be required
        self.daemon = True

        self._node = node

    def run(self):

        while True:
            logger.debug('Awaiting request for configuration check')

            self._node.events.check_new_config.wait(timeout=CONFIG_REFRESH_DELAY)

            logger.info('Proceeding with check for new configuration')

            try:

                if self.__new_config_available():

                    with self._node.events.getting_config:
                        config = None

                        while not config:
                            logger.info(f"Obtaining configuration for node {self._node.node_id} from API")
                            config = self._node.api.get_config()
                            # Keep trying to get the configuration if not successful
                            if not config:
                                logger.error(f"No config obtained from API; retrying in {API_RETRY_DELAY} seconds")
                                time.sleep(API_RETRY_DELAY)

                        # Update config definition, save it to DB, and load any custom drivers from it
                        self._node.config = config

                        self._node.events.getting_config.notify_all()

                self._node.events.check_new_config.clear()

            except Exception:
                logger.exception(
                    f"Exception while checking/obtaining/applying config; sleeping {API_RETRY_DELAY} seconds"
                )
                time.sleep(API_RETRY_DELAY)

    def __new_config_available(self):
        logger.info(f"Checking for configuration for node {self._node.node_id} from API")

        try:
            node_meta = self._node.api.get_node()
            if node_meta:
                if 'message' in node_meta:
                    logger.info(f"API message: {node_meta['message']}")

                if 'config_id' in node_meta:
                    available_config = self._node.config
                    logger.debug(f"Current available local config: {available_config}")
                    if not available_config:
                        logger.debug("Local configuration is not available, but remote config is.")
                        return True

                    if get_digest(available_config) == node_meta['config_id']:
                        logger.info('Latest remote configuration is in use locally')
                        return False
                    else:
                        logger.info(f"New configuration with ID {node_meta['config_id']} is available from API")
                        return True
                else:
                    logger.warn("No configuration info returned from API")
                    return None
            else:
                return None

        except Exception:
            logger.exception('Exception raised while requesting node info from API')
            return None
