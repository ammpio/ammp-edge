import logging
from peewee import Model, SqliteDatabase, TextField, BlobField
import os
import json

from kvstore.constants import SQLITE_STORE_REL_PATH, SQLITE_CACHE_ABS_PATH

logger = logging.getLogger(__name__)

# A placeholder database object needs to be created in order for the model to be initialized
sqlite_db = SqliteDatabase(None, autoconnect=False)


class KV:
    def __init__(self, sqlite_db_path: str) -> None:
        self._sqlite_db = sqlite_db
        self._sqlite_db.init(
            sqlite_db_path,
            pragmas={
                'journal_mode': 'wal',
                'synchronous': 'full'
            },
        )
        self._sqlite_db.connect(reuse_if_open=True)
        self._sqlite = self.PersistentKV

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_value, traceback):
        self._sqlite_db.close()

    def __del__(self):
        self._sqlite_db.close()

    @staticmethod
    def __dump(value) -> bytes:
        return json.dumps(value).encode('utf-8')

    @staticmethod
    def __load(bvalue: bytes):
        # try:
        return json.loads(bvalue)
        # except something:
        #     return None

    def get(self, key: str, default=None):
        try:
            bvalue = self._sqlite.get_by_id(key).value
        except self._sqlite.DoesNotExist:
            return default

        return self.__load(bvalue)

    def set(self, key: str, value) -> bool:
        bvalue = self.__dump(value)
        return self._sqlite.insert(key=key, value=bvalue) \
                .on_conflict('replace').execute()

    class KVStore(Model):
        key = TextField(primary_key=True)
        value = BlobField(null=True)

        class Meta:
            database = sqlite_db


class KVStore(KV):
    def __init__(self) -> None:
        SQLITE_STORE_DB_PATH = os.path.join(os.getenv('SNAP_COMMON', './'), SQLITE_STORE_REL_PATH)
        self._sqlite_db = sqlite_db
        self._sqlite_db.init(
            SQLITE_STORE_DB_PATH,
            pragmas={
                'journal_mode': 'wal',
                'synchronous': 'full'
            },
        )
        self._sqlite_db.connect(reuse_if_open=True)
        self._sqlite = self.KVStore



class KVCache(KV):
    def __init__(self) -> None:
        self._sqlite_db = sqlite_db
        self._sqlite_db.init(
            SQLITE_CACHE_ABS_PATH,
            pragmas={
                'journal_mode': 'wal',
                'synchronous': 'full'
            },
        )
        self._sqlite_db.connect(reuse_if_open=True)
        self._sqlite_db.create_tables([self.KVStore], safe=True)
        self._sqlite = self.KVStore