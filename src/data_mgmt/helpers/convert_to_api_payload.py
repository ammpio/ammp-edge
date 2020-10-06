import logging
from copy import deepcopy

logger = logging.getLogger(__name__)

"""
This module is currently used by the datapusher
"""

def convert_to_api_payload(readout):
	logger.debug(f"CONVERT TO API PAYLOAD. READOUT: {readout}")
	readout = deepcopy(readout)
	fields = {}
	for rdg in readout['device_readings']:
		# delete device names, maybe a more elegant way ?
		dev_id = rdg.pop('dev_id', None)
		fields.update(rdg)
	logger.debug(f"CONVERT TO API PAYLOAD. Fields: {fields}")
	readout['device_readings'] = fields
	readout['fields'] = readout.pop('device_readings')
	logger.debug(f"CONVERT TO API PAYLOAD. Transformed readout: {readout}")

	return readout

