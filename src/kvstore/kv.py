import logging
from os import getenv, path
import json
import sqlite3
from kvstore.constants import SQLITE_STORE_REL_PATH, SQLITE_CACHE_ABS_PATH

logger = logging.getLogger(__name__)


TABLENAME = 'kvstore'
KEY_FIELD = 'key'
VALUE_FIELD = 'value'


class KV:
    def __init__(self, sqlite_db_path: str) -> None:
        self._conn = sqlite3.connect(sqlite_db_path, check_same_thread=False)
        self._conn.set_trace_callback(logger.debug)
        self._cur = self._conn.cursor()
        self.__initialize_db()

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_value, traceback) -> None:
        self._conn.close()

    def __del__(self) -> None:
        self._conn.close()

    def __initialize_db(self) -> None:
        self._cur.executescript(
            f'''
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = FULL;
            CREATE TABLE IF NOT EXISTS '{TABLENAME}' (
                key TEXT PRIMARY KEY NOT NULL,
                value BLOB NOT NULL
            );
            '''
        )
        self._conn.commit()

    @ staticmethod
    def __dump(value) -> bytes:
        return json.dumps(value).encode('utf-8')

    @ staticmethod
    def __load(bvalue: bytes):
        return json.loads(bvalue)

    def __select(self, key: str) -> bytes:
        self._cur.execute("SELECT value FROM 'kvstore' WHERE key = :key", {'key': key})
        return self._cur.fetchone()[0]

    def __upsert(self, key: str, value: bytes) -> None:
        self._cur.execute(
            f'''INSERT INTO '{TABLENAME}' ({KEY_FIELD}, {VALUE_FIELD}) values (:key, :value)
            ON CONFLICT({KEY_FIELD}) DO UPDATE SET {VALUE_FIELD}=:value''', {'key': key, 'value': value}
        )
        self._conn.commit()

    def get(self, key: str, default=None):
        try:
            bvalue = self.__select(key)
        except TypeError:
            return default

        return self.__load(bvalue)

    def set(self, key: str, value) -> None:
        bvalue = self.__dump(value)
        self.__upsert(key, bvalue)


class KVStore(KV):
    def __init__(self) -> None:
        SQLITE_STORE_DB_PATH = path.join(getenv('SNAP_COMMON', './'), SQLITE_STORE_REL_PATH)
        KV.__init__(self, SQLITE_STORE_DB_PATH)


class KVCache(KV):
    def __init__(self) -> None:
        KV.__init__(self, SQLITE_CACHE_ABS_PATH)
