import logging
from copy import deepcopy

logger = logging.getLogger(__name__)

"""
This module is currently used by the datapusher
"""


def convert_to_api_payload(readout, readings_from_config):
	readout = deepcopy(readout)
	# fields is a dict will all the readings of all devices -- without the device id. This is done for backwards compatibility
	fields = {}
	for rdg in readout['device_readings']:
		# delete device names, maybe a more elegant way ?
		rdg.pop('dev_id', None)
		fields.update(rdg)
	# get the old reading names from the config for backwards compatibility
	for rdg in readings_from_config:
		for key in fields:
			if readings_from_config[rdg]['var'] == key:
				fields[rdg] = fields.pop(key)
				break
	readout['fields'] = fields
	# move snap_rev, reading_duration , and reading_offset under fields
	for key in ['snap_rev', 'reading_duration', 'reading_offset']:
		readout['fields'][key] = readout[key]
		readout.pop(key)
	readout.pop('device_readings')
	return readout
