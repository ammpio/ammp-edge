import logging
from redis import Redis
from redis.exceptions import ConnectionError
import json
from time import sleep

logger = logging.getLogger(__name__)

DEFAULT_DB = 0
HEALTH_CHECK_INT = 30
STR_ENCODING = 'utf-8'


class KVStore(Redis):
    def __init__(self, db: int = DEFAULT_DB) -> None:
        self.db = db
        self.r = Redis(
            host='127.0.0.1',
            port=6379,
            db=db,
            health_check_interval=HEALTH_CHECK_INT)

        try:
            if self.r.ping():
                logger.info("Connection to Redis server successful")
            else:
                logger.error("Cannot ping Redis server")
        except ConnectionError:
            logger.exception("Exception while trying to connect to Redis server")

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_value, traceback):
        self.r.close()

    def __del__(self):
        self.r.close()

    def get(self, key: str):
        value = self.r.get(key)
        if value is None:
            return None
        else:
            return json.loads(value.decode(STR_ENCODING))

    def set(self, key: str, value, force: bool = False) -> bool:
        # If the current value is already the same as what is being set,
        # (and "force" is not set) then take no action
        if force or self.get(key) != value:
            res = self.r.set(key, json.dumps(value).encode(STR_ENCODING))
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
        channel_name = f"__keyspace@{self.db}__:{key}"

        p = self.r.pubsub(ignore_subscribe_messages=True)
        p.subscribe(channel_name)

        while True:
            message = p.get_message()
            if message:
                return self.get(key)
            sleep(0.01)
