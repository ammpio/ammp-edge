from typing import Dict, List


def add_to_device_readings(readings: List[Dict], device_key: str, kv: Dict) -> None:
    try:
        # Find readout for this device and append result
        next(r for r in readings if r['_d'] == device_key).update(kv)
    except StopIteration:
        # If readout does not exist, add it, with the appropriate device key
        readings.append({
            '_d': device_key,
            **kv
        })
