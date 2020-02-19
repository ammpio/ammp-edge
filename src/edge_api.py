import logging
import requests
from time import sleep
from kvstore import KVStore

logger = logging.getLogger(__name__)

DEFAULT_REQUEST_TIMEOUT = 60
MAX_REQUEST_RETRIES = 3
REQUEST_HOLDOFF = 5


class EdgeAPI(object):
    def __init__(self) -> None:

        self._kvs = KVStore()

        # These wait for the values to be set in Redis (if not already set)
        # This should be done during node loading/initialization
        remote_api = self._kvs.get_or_wait('node:remote_api')
        self.api_host = remote_api['host']
        self.api_ver = remote_api['apiver']
        self.node_id = self._kvs.get_or_wait('node:node_id')
        self.access_key = self._kvs.get_or_wait('node:access_key')

        self._base_url = f"https://{self.api_host}/api/{self.api_ver}/nodes/{self.node_id}/"

        self._session = requests.Session()
        self._session.headers.update({'Authorization': self.access_key})
        self.__request_timeout = remote_api.get('timeout', DEFAULT_REQUEST_TIMEOUT)

    def get_command(self) -> str:
        status_code, rtn = self.__get_request('command')

        if status_code == 200:
            if rtn.get('command'):
                logger.info(f"Obtained command {rtn['command']} from API")
                return rtn['command']
        elif status_code == 204:
            logger.info("API returned 204 (No Content)")
            return None
        else:
            logger.error(f"HTTP Error {status_code} returned from command API request")
            return None

    def get_upload_url(self) -> str:
        status_code, rtn = self.__get_request('upload_url')

        if status_code == 200:
            if rtn.get('upload_url'):
                logger.info(f"Obtained upload URL {rtn['upload_url']} from API")
                return rtn['upload_url']
        else:
            logger.error(f"HTTP Error {status_code} returned from command API request")
            return None

    def post_env_scan(self, scan_result: dict) -> bool:
        status_code, rtn = self.__post_request('env_scan', payload=scan_result)
        if status_code == 200:
            logger.info("Successfully submitted environment scan")
            return True
        else:
            logger.error(f"HTTP Error {status_code} while trying to to submit environment scan to API")
            return False

    def __get_request(self, endpoint: str, params: dict = None) -> dict:
        r = self.__make_http_request(self._base_url + endpoint, 'GET', None, params)
        return self.__parse_response(r)

    def __post_request(self, endpoint: str, payload: dict = None, params: dict = None) -> dict:
        r = self.__make_http_request(self._base_url + endpoint, 'POST', payload, params)
        return self.__parse_response(r)

    def __make_http_request(
                    self,
                    url: str,
                    method: str = 'GET',
                    payload: dict = None,
                    params: dict = None,
                    retry_count: int = 0
                    ) -> requests.Response:

        try:
            if method.upper() == 'GET':
                return self._session.get(url, params=params, timeout=self.__request_timeout)
            elif method.upper() == 'POST':
                return self._session.post(url, json=payload, data=params, timeout=self.__request_timeout)
            else:
                logger.error(f"Unknown request method {method}")
                return None
        except requests.exceptions.ConnectionError as e:
            logger.error(f"Connection error {e} while doing {method} request to {url}")
        except requests.exceptions.Timeout as e:
            logger.error(f"Timeout error {e} while doing {method} request to {url}")
        except Exception as e:
            logger.exception(f"Exception {e} while doing {method} request to {url}")

        # If we've got this far there has been an exception and we need to decide whether to retry
        if retry_count < MAX_REQUEST_RETRIES:
            retry_count += 1
            logger.info(f"Will retry (#{retry_count}/{MAX_REQUEST_RETRIES}). First sleeping {REQUEST_HOLDOFF} s.")
            sleep(REQUEST_HOLDOFF)
            return self.__make_http_request(url, method, payload, params, retry_count)

        return None

    @staticmethod
    def __parse_response(r: requests.Response) -> (int, dict):
        try:
            return r.status_code, r.json()
        except ValueError:
            logger.error(f"Response from API: {r.text}. Cannot be parsed as JSON")
            return r.status_code, None
