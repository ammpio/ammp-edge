#!/usr/bin/env python3

import logging
import sys
import os
import sched
import time
import signal

from dotenv import load_dotenv

import node_mgmt
from data_mgmt import DataPusher
from node_mgmt.node import Node
from reader import get_readout

# Set up logging
logging.basicConfig(format='%(threadName)s:%(name)s:%(lineno)d [%(levelname)s] %(message)s', level='INFO')
logger = logging.getLogger(__name__)

# Load additional environment variables from env file (set by snap configuration)
dotenv_path = os.path.join(os.environ.get('SNAP_COMMON', '.'), '.env')
load_dotenv(dotenv_path)

if os.environ.get('LOGGING_LEVEL'):
    try:
        logging.getLogger().setLevel(os.environ['LOGGING_LEVEL'])
    except Exception:
        logger.warn(f"Failed to set log level to {os.environ['LOGGING_LEVEL']}", exc_info=True)

__version__ = '0.9'


VOLATILE_QUEUE_MAXSIZE = 10000


def reading_cycle(node: Node, pusher: DataPusher, sc=None):
    # Check if scheduler has been applied, and if so schedule this function to be run again
    # at the appropriate interval before taking the readings
    if sc:
        # If we want readings with round timestamps, schedule the next reading for the next such time
        # Otherwise schedule it 'interval' seconds from now. The latter (non-rounded) option
        # will lead to timestamp drift if any readings take longer than 'interval' (since the scheduler
        # won't start a new process until the current one has finished).
        # With the round-time option, any readings immediately following ones that take too long will have
        # non-round timestamps, but if possible ones following that should "catch up". That said,
        # drift can still accumulate and if it becomes greater than 'interval', a reading will be skipped.
        if node.config.get('read_roundtime'):
            sc.enterabs(roundtime(node.config['read_interval']), 1, reading_cycle, (node, pusher, sc))
        else:
            sc.enter(node.config['read_interval'], 1, reading_cycle, (node, pusher, sc))

    try:
        readout = get_readout(node)
        pusher.push_readout(readout)

    except Exception:
        logger.exception('READ: Exception getting readings')


def roundtime(interval):
    tnow = time.time()
    next_roundtime = tnow + interval - (tnow % interval)
    return next_roundtime


def sigterm_handler(_signo, _stack_frame):
    logger.info('Received SIGTERM; shutting down')
    # Raises SystemExit(0):
    sys.exit(0)


class StreamToLogger(object):
    """
    Fake file-like stream object that redirects writes to a logger instance.
    """
    def __init__(self, logger, log_level=logging.INFO):
        self.logger = logger
        self.log_level = log_level
        self.linebuf = ''

    def write(self, buf):
        for line in buf.rstrip().splitlines():
            self.logger.log(self.log_level, line.rstrip())

    def flush(self):
        pass


def main():

    node = node_mgmt.Node()

    # Redirect stdout and stderr to error file
    sys.stdout = StreamToLogger(logger, logging.INFO)
    sys.stderr = StreamToLogger(logger, logging.ERROR)

    # Handle SIGTERM from daemon control
    signal.signal(signal.SIGTERM, sigterm_handler)

    # Create queue processor instances and start the threads' internal run() method
    pusher = DataPusher(node)

    if node.config.get('read_interval'):
        # We will be carrying out periodic readings (daemon mode)

        # Set up scheduler and run reading cycle with schedule (reading_cycle function then schedules
        # its own further iterations)
        s = sched.scheduler(time.time, time.sleep)

        if node.config.get('read_roundtime'):
            s.enterabs(roundtime(node.config['read_interval']), 1, reading_cycle, (node, qs, s))
            logger.info('Waiting to start on round time interval...')
        else:
            reading_cycle(node, pusher, s)

        s.run()

    else:
        # Carry out a one-off reading, with no scheduler
        reading_cycle(node, pusher)


if __name__ == '__main__':
    main()
