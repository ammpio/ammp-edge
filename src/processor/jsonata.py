import logging
import os
import subprocess
import json
from json import JSONDecodeError
from pyjsonata import jsonata, PyjsonataError

logger = logging.getLogger(__name__)

JSON_UNDEFINED = 'undefined'


def evaluate_jsonata(data_dict, expr):

    inp = json.dumps(data_dict)

    try:
        res_str = jsonata(expr, inp)
    except PyjsonataError as e:
        logger.error(f"Error while processing JSONata: {e}"
            f"Input dictionary: {data_dict}"
            f"Expression: {expr}"
        )
        return None

    logger.debug(f"JSONata output string: {res_str}")
    if not res_str or res_str == JSON_UNDEFINED:
        return None

    try:
        res = json.loads(res_str)
    except JSONDecodeError:
        logger.error(f"Cannot decode response as JSON: {res_str}")
        return None

    return res
