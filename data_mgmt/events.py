import logging
logger = logging.getLogger(__name__)

import threading

do_shutdown = threading.Event()
push_in_progress = threading.Event()