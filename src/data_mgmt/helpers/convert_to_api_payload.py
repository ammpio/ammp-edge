import logging
from copy import deepcopy

logger = logging.getLogger(__name__)

"""
This module is currently used by the datapusher
"""

def convert_to_api_payload(readout):
	logger.debug(f"CONVERT TO API PAYLOAD. READOUT: {readout}")
	readout = deepcopy(readout)
	# fields is a dict will all the readings of all devices -- without the device id. This is done for backwards compatibility
	fields = {}
	for rdg in readout['device_readings']:
		# delete device names, maybe a more elegant way ?
		dev_id = rdg.pop('dev_id', None)
		fields.update(rdg)
	readout['device_readings'] = fields
	logger.debug(f"CONVERT TO API PAYLOAD. Transformed readout1: {readout}")
	readout['fields'] = readout.pop('device_readings')
	logger.debug(f"CONVERT TO API PAYLOAD. Transformed readout: {readout}")

	return readout
