import logging
logger = logging.getLogger(__name__)

import time
from datetime import datetime
import threading
from .events import push_in_progress, do_shutdown

from config_mgmt import config
from db_model import NVQueue

class NonVolatileQ(object):

    def __init__(self):
        # No real setup for peewee model?
        pass

    def get(self):
        # Operate queue in LIFO fashion (obtain last inserted item)
        try:
            lastrow = NVQueue.select().order_by(NVQueue.id.desc()).get()
            try:
                item = lastrow.item
                lastrow.delete_instance()
                # json.loads(lastrow.item) ???
                return item

            except Exception as ex:
                logger.exception('NVQP: Exception')

        except NVQueue.DoesNotExist:
            # There's nothing in the queue
            return None

    def put(self, item):
        # # Use timestamp as row ID
        # try:
        #     ts = 
        # except Exception as ex:
        #     logger.exception('NVQP: Exception')
        #     ts = None

        # We expect the item being returned to be a dict
#        item_str = json.dumps(item)
        NVQueue.create(item=item)

    def qsize(self):
        qsize = NVQueue.select().count()

        return qsize

    def close(self):
        NVQueue._meta.database.close()


class NonVolatileQProc(threading.Thread): 
    def __init__(self, queue): 
        threading.Thread.__init__(self)
        self.name = 'nvq_proc'
        # We want to get the chance to do clean-up on this thread if the program exits
        self.daemon = False

        self._queue = queue

    def run(self):

        self._nvq = NonVolatileQ()

        while not do_shutdown.is_set():

            qsize = self._queue.qsize()
            nvqsize = self._nvq.qsize()
            logger.info('NVQP: Queue size: internal: %d, non-volatile: %d, pending: %d' % (qsize, nvqsize, push_in_progress.is_set()))

            if nvqsize > 0 and (qsize + push_in_progress.is_set()) < 5:
                # If the internal queue is almost empty but the queue file isn't then pull from it
                readout = self._nvq.get()
                logger.debug('NVQP: Got readout at %s from queue file; moving to internal queue' % (readout['time']))
                self._queue.put(readout)

                # Make sure we're not going way too fast
                time.sleep(1)

            elif qsize > config.get('volatile_q_size', 5):
                # If the internal queue is starting to grow large, then move items to the queue file
                readout = self._queue.get() 
                logger.debug('NVQP: Got readout at %s from internal queue; moving to file' % (readout['time']))
                self._nvq.put(readout)

            else:
                # If the queue is "just right" then take is easy for a little while
                time.sleep(10)

        logger.info('NVQP: Stashing internal queue')
        # If we're exiting, then put all of the internal queue into the non-volatile queue
        while not self._queue.empty():
            readout = self._queue.get() 
            if not readout == {}:
                logger.debug('NVQP: Got readout at %s from internal queue; moving to file' % (readout['time']))
                self._nvq.put(readout)

        logger.info('NVQP: Shutting down')
        self._nvq.close()
