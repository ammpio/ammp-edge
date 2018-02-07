
import logging
logger = logging.getLogger(__name__)

from peewee import *
from playhouse.sqlite_ext import *
import os

# Create separate database files for configuration storage and for non-volatile queue storage
# Naively, it seems that doing it this way will reduce the fact that the config file
# might get corrupted due to issues with writing to the queue file (in case of e.g. power outage)


cdb = SqliteExtDatabase(os.path.join(os.getenv('SNAP_COMMON', './'), 'config.db'))
cdb.connect()

qdb = SqliteExtDatabase(os.path.join(os.getenv('SNAP_COMMON', './'), 'queue.db'))
qdb.connect()

class NodeConfig(Model):
    node_id = TextField(primary_key=True)
    config = JSONField(null=True)
    access_key = DateTimeField(null=True)
    class Meta:
        database = cdb

class NVQueue(Model):
    # Use integer timestamp as default row ID
    id = PrimaryKeyField()
    item = JSONField()
    class Meta:
        database = qdb

# Create tables if they don't already exist
cdb.create_tables([NodeConfig], safe=True)
qdb.create_tables([NVQueue], safe=True)