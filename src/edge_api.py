import logging
from typing import Optional, Tuple
import requests
from time import sleep
from kvstore import KVStore, keys

logger = logging.getLogger(__name__)

DEFAULT_REQUEST_TIMEOUT = 60
MAX_REQUEST_RETRIES = 5
REQUEST_HOLDOFF = 15
REMOTE_API_ROOT = 'https://edge.stage.ammp.io/api/v0/'


class EdgeAPI(object):
    def __init__(self, root: str = REMOTE_API_ROOT) -> None:

        self._kvs = KVStore()

        self.remote_api_root = root
        self.node_id = self._kvs.get(keys.NODE_ID)
        self.access_key = self._kvs.get(keys.ACCESS_KEY)

        self._base_url = f"{self.remote_api_root}nodes/{self.node_id}"

        self._session = requests.Session()
        self._session.headers.update({'Authorization': self.access_key})
        self.__request_timeout = DEFAULT_REQUEST_TIMEOUT

    def get_node(self) -> Optional[dict]:
        status_code, rtn = self.__get_request('')

        if status_code == 200:
            logger.info("Obtained node metadata from API")
            logger.debug(f"Payload: {rtn}")
            return rtn
        else:
            logger.error(f"Error {status_code} returned from metadata API request")
            logger.info(f"API response: {rtn}")
            return None

    def get_config(self) -> Optional[dict]:
        status_code, rtn = self.__get_request('/config')

        if status_code == 200:
            if rtn.get('config'):
                logger.info("Obtained config from API")
                logger.debug(f"Payload: {rtn}")
                return rtn['config']
            else:
                logger.error('API call successful but response did not include a config payload')
                return None
        else:
            logger.error(f"Error {status_code} returned from config API request")
            logger.info(f"API response: {rtn}")
            return None

    def get_command(self) -> Optional[str]:
        status_code, rtn = self.__get_request('/command')

        if status_code == 200:
            if rtn.get('command'):
                logger.info(f"Obtained command {rtn['command']} from API")
                return rtn['command']
        elif status_code == 204:
            logger.info("API returned 204 (No Content)")
            return None
        else:
            logger.error(f"HTTP Error {status_code} returned from command API request")
            logger.info(f"API response: {rtn}")
            return None

    def post_env_scan(self, scan_result: dict) -> bool:
        status_code, _ = self.__post_request('/env_scan', payload=scan_result)
        if status_code in [200, 204]:
            logger.info("Successfully submitted environment scan")
            return True
        else:
            logger.error(f"HTTP Error {status_code} while trying to to submit environment scan to API")
            return False

    def __get_request(self, endpoint: str, params: Optional[dict] = None) -> dict:
        r = self.__make_http_request(self._base_url + endpoint, 'GET', None, params)
        return self.__parse_response(r)

    def __post_request(self, endpoint: str, payload: Optional[dict] = None, params: Optional[dict] = None) -> dict:
        r = self.__make_http_request(self._base_url + endpoint, 'POST', payload, params)
        return self.__parse_response(r)

    def __make_http_request(
                    self,
                    url: str,
                    method: str = 'GET',
                    payload: Optional[dict] = None,
                    params: Optional[dict] = None,
                    retry_count: int = 0
                    ) -> Optional[requests.Response]:

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
    def __parse_response(r: requests.Response) -> Tuple[Optional[int], Optional[dict]]:
        if r is None:
            return None, None

        try:
            return r.status_code, r.json()
        except ValueError:
            if r.status_code != 204:
                logger.error(f"Response from API: {r.text}. Cannot be parsed as JSON")
            return r.status_code, None
