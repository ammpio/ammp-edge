REDIS_HOST = '127.0.0.1'
REDIS_PORT = 6379
REDIS_DB_NUM = 0
REDIS_HEALTH_CHECK_INT = 30
REDIS_SUB_SLEEP_TIME = 0.001
SQLITE_FILENAME = 'kvstore.db'

PERSISTENT_KEYS = [
    'n/node_id',
    'n/access_key',
    'n/config',
    'n/wifi_ap_available',
    'n/wifi_ap_config',
]
