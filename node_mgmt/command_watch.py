import logging
logger = logging.getLogger(__name__)

import time
import json
import threading
import requests

from importlib import import_module

# If API endpoint can't be reached wait API_RETRY_DELAY seconds before retrying
API_RETRY_DELAY = 10
# Even if this is not explicitly requested, carry out a command check every COMMAND_CHECK_DELAY seconds
COMMAND_CHECK_DELAY = 900

class CommandWatch(threading.Thread): 
    """Request command from node if flag is set"""
    def __init__(self, node): 
        threading.Thread.__init__(self)
        self.name = 'command_watch'
        # Make sure this thread exits directly when the program exits; no clean-up should be required
        self.daemon = True

        self._node = node

    def run(self):

        while True:
            logger.debug('Awaiting request for command check')

            self._node.events.get_command.wait(timeout=COMMAND_CHECK_DELAY)

            logger.info('Proceeding with check for new command')

            command = self.__command_from_api()
            if command:
                logger.info('Running command: %s' % command)
                # Runs function with command name from .commands module
                try:
                    commod = import_module('.commands', 'node_mgmt')
                    getattr(commod, command)(self._node)
                except:
                    logger.exception('Could not run command %s' % command)

            self._node.events.get_command.clear()


    def __command_from_api(self):

        logger.info('Obtaining command for node %s from API' % self._node.node_id)

        try:
            r = requests.get('https://%s/api/%s/nodes/%s/command' % (self._node.remote['host'], self._node.remote['apiver'], self._node.node_id),
                headers={'Authorization': self._node.access_key})
            rtn = json.loads(r.text)

            if r.status_code == 200:
                if 'message' in rtn:
                    logger.debug('API message: %s' % rtn['message'])

                if rtn.get('command'):
                    logger.info('Obtained command from API')
                    logger.debug('Command payload: %s' % rtn['command'])
                    return rtn['command']
                else:
                    logger.error('API call successful but response did not include a command payload')
                    return None
            else:
                logger.error('Error %d requesting command from API' % r.status_code)
                if rtn:
                    logger.debug('API response: %s' % rtn)
                return None
        except:
            logger.exception('Exception raised while requesting command from API')
            return None

