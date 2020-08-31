import logging
from copy import deepcopy

logger = logging.getLogger(__name__)

"""
This module is currently used by the datapusher
"""

def make_api_payload(readout):
	readout = deepcopy(readout)
	fields = {}
	for rdg in readout['device_readings']:
		# delete device names, maybe a more elegant way ?
		dev_id = rdg.pop('dev_id', None)
		fields.update(rdg)

	readout['device_readings'] = fields
	readout['fields'] = readout.pop('device_readings')
	logger.debug(f"Readout to push: {readout}")

	return readout

