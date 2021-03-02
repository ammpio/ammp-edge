import logging
from playhouse.sqlite_ext import Model, SqliteExtDatabase, PrimaryKeyField, TextField, JSONField
import os

logger = logging.getLogger(__name__)

# Create a database files for non-volatile queue storage, and load the model
QUEUE_DB_PATH = os.path.join(os.getenv('SNAP_COMMON', './'), 'queue.db')
qdb = SqliteExtDatabase(QUEUE_DB_PATH,
                        pragmas={
                            'journal_mode': 'wal',
                            'synchronous': 2
                        })
qdb.connect()


class NVQueue(Model):
    # Use integer timestamp as default row ID
    id = PrimaryKeyField()
    item = JSONField()

    class Meta:
        database = qdb


qdb.create_tables([NVQueue], safe=True)


# If the (legacy) config.db file exists, also load it here
CONFIG_DB_PATH = os.path.join(os.getenv('SNAP_COMMON', './'), 'config.db')
if os.path.exists(CONFIG_DB_PATH):
    cdb = SqliteExtDatabase(CONFIG_DB_PATH,
                            pragmas={
                                'journal_mode': 'wal',
                                'synchronous': 2
                            })
    cdb.connect()
else:
    cdb = None


class NodeConfig(Model):
    node_id = TextField(primary_key=True)
    config = JSONField(null=True)
    access_key = TextField(null=True)

    class Meta:
        database = cdb
