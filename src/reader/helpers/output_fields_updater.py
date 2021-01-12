import logging
from constants import DEVICE_ID_KEY, VENDOR_ID_KEY, OUTPUT_READINGS_DEV_ID

logger = logging.getLogger(__name__)

def output_fields_updater(output_fields, vendor_id):
	try:
		output_fields.update(
		{
			DEVICE_ID_KEY: OUTPUT_READINGS_DEV_ID,
			VENDOR_ID_KEY: vendor_id
		}
		)
		logger.debug(f"Updated output_fields: {output_fields}")
		return output_fields

	except Exception:
		logger.error(f"Output fields failed to update with vendor_id: {vendor_id}. Returning original output_fields")
		return	output_fields