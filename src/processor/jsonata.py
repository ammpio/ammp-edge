import logging
from jsonata import Jsonata, JException

logger = logging.getLogger(__name__)

JSON_UNDEFINED = "undefined"


def evaluate_jsonata(data, expr):

    try:
        expr = Jsonata(expr)
        res = expr.evaluate(data)
    except JException as e:
        logger.error(
            f"Error while processing JSONata: {e}\n"
            f"Input data: {data}\n"
            f"Expression: {expr}"
        )
        return None

    logger.debug(f"JSONata output: {res}")

    return res
