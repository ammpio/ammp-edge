import logging
import os
import subprocess
import json

logger = logging.getLogger(__name__)


def evaluate_jsonata(data_dict, expr):
    if os.getenv('SNAP'):
        jfq = os.path.join(os.getenv('SNAP'), 'bin', 'jfq')
    else:
        jfq = 'jfq'
    cmd = [jfq, '-j', expr]

    inp = json.dumps(data_dict).encode('utf-8')
    try:
        res = subprocess.run(cmd, stdout=subprocess.PIPE, input=inp)
    except FileNotFoundError:
        logger.error(f"Executable {cmd[0]} not found. Ensure that jfq is installed")
        return None

    res_str = res.stdout.decode('utf-8').rstrip()

    if not res_str:
        return None
    else:
        try:
            return json.loads(res_str)
        except Exception:
            logger.error(f"JSONata parser did not return valid JSON: {res_str}")
            return None
