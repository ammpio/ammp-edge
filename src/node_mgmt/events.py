import logging
import threading

logger = logging.getLogger(__name__)


class NodeEvents(object):
    def __init__(self):
        self.do_shutdown = threading.Event()
        self.push_in_progress = threading.Event()

        self.check_new_config = threading.Event()
        self.getting_config = threading.Condition()

        self.get_command = threading.Event()
