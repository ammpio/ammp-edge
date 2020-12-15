import logging
import arrow
from constants import DEVICE_ID_KEY, OUTPUT_READINGS_DEV_ID

logger = logging.getLogger(__name__)

# This module is currently used by the datapusher, for payloads sent to the API endpoint
# or an Influx instance

META_TO_FIELDS = ['snap_rev', 'reading_duration', 'reading_offset']
META_TO_META = ['config_id']
PROTECTED_KEY_PREFIX = '_'


def convert_to_api_payload(readout, config_readings):
    # Create API payload and convert time from unix timestamp to date
    api_payload = {
        'time': arrow.get(readout['t']).strftime('%Y-%m-%dT%H:%M:%SZ'),
        'fields': {},
        'meta': {},
    }

    # Get the reading definitions from the config and map device-based readings
    for rdg, r in config_readings.items():
        value = get_value_from_dev_readings(
            readout['r'], r['device'], r['var']
        )
        if value is None:
            logger.debug(f"No value for reading {rdg}: {r}")
            continue
        api_payload['fields'][rdg] = value

    # Copy any calculated output fields
    api_payload['fields'].update(
        get_all_readings_for_device(readout['r'], OUTPUT_READINGS_DEV_ID)
    )

    # Copy some metadata to 'meta' object
    for key in META_TO_META:
        api_payload['meta'][key] = readout['m'][key]

    # Copy some metadata to 'fields' object
    for key in META_TO_FIELDS:
        api_payload['fields'][key] = readout['m'][key]

    return api_payload


def get_all_readings_for_device(dev_readings, device):
    try:
        dev_rdg = next(r for r in dev_readings if r[DEVICE_ID_KEY] == device)
    except StopIteration:
        return {}
            
    return {k: v for k, v in dev_rdg.items() if not is_protected_key(k)}


def get_value_from_dev_readings(dev_readings, device, var):
    dev_rdg = get_all_readings_for_device(dev_readings, device)
    return dev_rdg.get(var)


def is_protected_key(key: str) -> bool:
    return key.startswith(PROTECTED_KEY_PREFIX)