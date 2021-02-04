import logging
import time
import threading

from importlib import import_module
import os

logger = logging.getLogger(__name__)

# Even if this is not explicitly requested, carry out a command check every COMMAND_CHECK_DELAY seconds
COMMAND_CHECK_DELAY = 900.0
SHELL_COMMAND_PREFIX = '#'


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
            try:

                logger.debug('Awaiting request for command check')

                self._node.events.get_command.wait(timeout=COMMAND_CHECK_DELAY)

                logger.info('Proceeding with check for new command')

                command = self._node.api.get_command()
                self._node.events.get_command.clear()

                if command:
                    logger.info(f"Running command: {command}")

                    if command.startswith(SHELL_COMMAND_PREFIX):
                        # Run shell command
                        try:
                            os.system(command[1:])
                        except Exception:
                            logger.exception(f"Could not run shell command {command}")
                    else:
                        # Runs function with command name from .commands module
                        try:
                            commod = import_module('.commands', 'node_mgmt')
                            getattr(commod, command)(self._node)
                        except Exception:
                            logger.exception(f"Could not run command {command}")

            except Exception:
                logger.exception("Exception raised in command watch")
                time.sleep(COMMAND_CHECK_DELAY)
