import logging
logger = logging.getLogger(__name__)

import threading

class NodeEvents(object):
    def __init__(self):

        self.do_shutdown = threading.Event()
        self.push_in_progress = threading.Event()

        self.check_new_config = threading.Event()
        self.getting_config = threading.Condition()