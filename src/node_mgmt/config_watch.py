import logging
import time
import json
import threading
import requests

logger = logging.getLogger(__name__)

# If API endpoint doesn't return config, wait API_RETRY_DELAY seconds before retrying
API_RETRY_DELAY = 10
# Even if this is not explicitly requested, carry out a configuration check every CONFIG_REFRESH_DELAY seconds
CONFIG_REFRESH_DELAY = 900


class ConfigWatch(threading.Thread): 
    """Request new configuration for node if flag is set"""
    def __init__(self, node): 
        threading.Thread.__init__(self)
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
                        self._node.save_config()
                        self._node.update_drv_from_config()

                        self._node.events.getting_config.notify_all()

                self._node.events.check_new_config.clear()

            except Exception:
                logger.exception(
                    f"Exception while checking/obtaining/applying config; sleeping {API_RETRY_DELAY} seconds"
                    )
                time.sleep(API_RETRY_DELAY)

    def __new_config_available(self):
        # TODO: Move this into the edge_api module
        logger.info('Checking for configuration for node %s from API' % self._node.node_id)

        try:
            r = requests.get('https://%s/api/%s/nodes/%s' % (self._node.remote_api['host'], self._node.remote_api['apiver'], self._node.node_id),
                headers={'Authorization': self._node.access_key})

            if r.status_code == 200:
                rtn = json.loads(r.text)

                if 'message' in rtn:
                    logger.debug('API message: %s' % rtn['message'])

                if not 'active_config' in rtn and not 'candidate_config' in rtn:
                    logger.error('No configuration info returned from API')
                    return None

                if self._node.config is None:
                    logger.debug('Local configuration is not available, but remote config is.')
                    return True

                if rtn.get('candidate_config'):
                    if self._node.config.get('config_id') != rtn['candidate_config']:
                        logger.debug('New candidate configuration ID %s is available' % rtn['candidate_config'])
                        return True
                else:
                    if self._node.config.get('config_id') == rtn['active_config']:
                        logger.debug('Latest remote configuration is in use locally')
                        return False
                    else:
                        logger.warning('Local configuration %s does not match remote active configuration %s, but no candidate is set. Please set remote candidate to force refresh.' % (self._node.config.get('config_id'), rtn.get('active_config')))
            else:
                logger.error('Error %d requesting node info from API' % r.status_code)
                if r.text:
                    logger.info('API response: %s' % r.text)
                return None

        except Exception:
            logger.exception('Exception raised while requesting node info from API')
            return None
