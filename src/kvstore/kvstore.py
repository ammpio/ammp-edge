import logging
from redis import Redis
from redis.exceptions import ConnectionError
from peewee import Model, SqliteDatabase, TextField, BlobField
import os
import cbor2 as cbor
from time import sleep

from kvstore.constants import (REDIS_HOST, REDIS_PORT, REDIS_DB_NUM,
                               REDIS_HEALTH_CHECK_INT, REDIS_SUB_SLEEP_TIME,
                               SQLITE_FILENAME, PERSISTENT_KEYS)

logger = logging.getLogger(__name__)

SQLITE_DB_PATH = os.path.join(os.getenv('SNAP_COMMON', './'), SQLITE_FILENAME)
# A placeholder database object needs to be created in order for the model to be initialized
sqlite_db = SqliteDatabase(None, autoconnect=False)


class KVStore(object):
    def __init__(self, sqlite_db_path: str = SQLITE_DB_PATH,
                 redis_host: str = REDIS_HOST,
                 redis_port: int = REDIS_PORT,
                 redis_db_num: int = REDIS_DB_NUM,
                 persistent_keys: list = PERSISTENT_KEYS) -> None:
        self._sqlite_db = sqlite_db
        self._sqlite_db.init(
            sqlite_db_path,
            pragmas={
                'journal_mode': 'wal',
                'synchronous': 2
            },
        )
        self._sqlite_db.connect(reuse_if_open=True)
        self._sqlite_db.create_tables([self.PersistentKV], safe=True)
        self._sqlite = self.PersistentKV

        self._redis = Redis(
            host=redis_host,
            port=redis_port,
            db=redis_db_num,
            health_check_interval=REDIS_HEALTH_CHECK_INT,
        )
        self._redis_db_num = redis_db_num
        try:
            if self._redis.ping():
                logger.info("Connection to Redis server successful")
            else:
                logger.error("Cannot ping Redis server")
        except ConnectionError as e:
            logger.exception(
                f"{e} while trying to connect to Redis server")
            raise

        self._persistent_keys = persistent_keys

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_value, traceback):
        self._redis.close()
        self._sqlite_db.close()

    def __del__(self):
        self._redis.close()
        self._sqlite_db.close()

    @staticmethod
    def __dump(value) -> bytes:
        return cbor.dumps(value)

    @staticmethod
    def __load(bvalue: bytes):
        try:
            return cbor.loads(bvalue)
        except cbor.CBORDecodeEOF:
            return None

    def get(self, key: str, default=None):
        bvalue = self._redis.get(key)
        if bvalue is None:
            if key in self._persistent_keys:
                try:
                    bvalue = self._sqlite.get_by_id(key).value
                except self._sqlite.DoesNotExist:
                    return default
                # Copy persistent value to Redis for future queries
                self._redis.set(key, bvalue)
            else:
                return default

        return self.__load(bvalue)

    def set(self, key: str, value, force: bool = False) -> bool:
        bvalue = self.__dump(value)
        if key in self._persistent_keys:
            self._sqlite.insert(key=key, value=bvalue) \
                .on_conflict('replace').execute()

        # If the current value in Redis is already the same as what is being set,
        # and "force" is not set, then take no action. This is useful in determining
        # whether a Redis keyspace notification is triggered
        if force or self._redis.get(key) != bvalue:
            res = self._redis.set(key, bvalue)
        else:
            res = True

        return res

    def waitfor(self, key: str):
        """
        Waits for an update to 'key', and returns value when an update occurs.
        This is also triggered when the new value is the same as the previous one, but a SET
        function has been called on that key in Redis.

        Note that keyspace notifications must be enabled in the Redis config for this to work.
        """
        channel_name = f"__keyspace@{self._redis_db_num}__:{key}"

        p = self._redis.pubsub(ignore_subscribe_messages=True)
        p.subscribe(channel_name)

        while True:
            message = p.get_message()
            if message:
                return self.get(key)
            sleep(REDIS_SUB_SLEEP_TIME)

    def get_or_wait(self, key: str):
        """
        Gets value, or waits for value to be set if not set
        """

        value = self.get(key)
        while value is None:
            value = self.waitfor(key)

        return value

    class PersistentKV(Model):
        key = TextField(primary_key=True)
        value = BlobField(null=True)

        class Meta:
            database = sqlite_db
