import logging
from typing import Dict, List
from processor.jsonata import evaluate_jsonata
from processor.process_reading import typecast
from copy import copy

logger = logging.getLogger(__name__)


def get_output(dev_rdg: Dict, output_config: List) -> List:
    """
    Get output fields based on readings.

    Inputs:
    dev_rdg: list of readings from each device. Example structure:
      {
        'dev_1': [{'var': 'pv_P', 'value': 10}, {'var': 'pv_E', 'value': 1000}],
        'dev_2': [{'var': 'batt_V', 'value': 49.5}, {'var': 'batt_soc', 'value': 95}]
      }
      in reality many other parameters will be available (i.e. everything required to
      take the readings).
    output_config: list of outputs. Each item in the list should be a dict containing
      'source' and 'field' keys with string values. The 'source' value should be a JSONata
      expression that's applied to dev_rdg. The output is saved under the 'value' key in
      the same list.
    """

    output = copy(output_config)

    for oc in output_config:
        evaluated_value = evaluate_jsonata(dev_rdg, oc['source'])
        if evaluated_value is None:
            continue
        output['value'] = typecast(evaluated_value, **oc)

    return output
