#!/usr/bin/env python3

import logging
import os
import sched
import time

from dotenv import load_dotenv

import node_mgmt
from data_mgmt import DataPusher
from node_mgmt.node import Node
from node_mgmt.config_watch import ConfigWatch
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

__version__ = '1.0'


def reading_cycle(node: Node, pusher: DataPusher, sc=None):
    config = node.config

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
        if config.get('read_roundtime'):
            sc.enterabs(roundtime(config['read_interval']), 1, reading_cycle, (node, pusher, sc))
        else:
            sc.enter(config['read_interval'], 1, reading_cycle, (node, pusher, sc))

    try:
        node.update_drv_from_config()
        readout = get_readout(config, node.drivers)
        pusher.push_readout(readout)

    except Exception:
        logger.exception('READ: Exception getting readings')


def roundtime(interval):
    tnow = time.time()
    next_roundtime = tnow + interval - (tnow % interval)
    return next_roundtime


def main():
    node = node_mgmt.Node()

    config_watch = ConfigWatch(node)
    config_watch.start()

    # If we still have not got a config, wait for one to be provided
    if node.config is None:
        logger.info('No stored configuration available; waiting until available')
        while node.config is None:
            time.sleep(15)

    # Create data pusher
    pusher = DataPusher(node)

    if node.config.get('read_interval'):
        # We will be carrying out periodic readings (daemon mode)

        # Set up scheduler and run reading cycle with schedule (reading_cycle function then schedules
        # its own further iterations)
        s = sched.scheduler(time.time, time.sleep)

        if node.config.get('read_roundtime'):
            s.enterabs(roundtime(node.config['read_interval']), 1, reading_cycle, (node, pusher, s))
            logger.info('Waiting to start on round time interval...')
        else:
            reading_cycle(node, pusher, s)

        s.run()

    else:
        # Carry out a one-off reading, with no scheduler
        reading_cycle(node, pusher)


if __name__ == '__main__':
    main()
