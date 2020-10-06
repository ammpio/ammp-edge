import logging
from copy import deepcopy

logger = logging.getLogger(__name__)

"""
This module is currently used by the datapusher
"""

def convert_to_api_payload(readout):
	readout = deepcopy(readout)
	# fields is a dict will all the readings of all devices -- without the device id. This is done for backwards compatibility
	fields = {}
	for rdg in readout['device_readings']:
		# delete device names, maybe a more elegant way ?
		rdg.pop('dev_id', None)
		fields.update(rdg)
	readout['fields'] = fields
	# move snap_rev, reading_duration , and reading_offset under fields
	readout['fields'].update({"snap_rev": readout['snap_rev']})
	readout['fields'].update({"reading_duration": readout['reading_duration']})
	readout['fields'].update({"reading_offset": readout['reading_offset']})
	for key in ['snap_rev', 'reading_duration', 'reading_offset']:
		readout.pop(key)
	logger.debug(f"CONVERT TO API PAYLOAD. READOUT: {readout}")
	return readout

