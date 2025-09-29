import logging
import os
import queue
import threading
import time
from copy import deepcopy
from datetime import UTC, datetime
from time import sleep

from constants import CONFIG_CALC_VENDOR_ID, DEVICE_ID_KEY, OUTPUT_READINGS_DEV_ID, VENDOR_ID_KEY
from kvstore import KVCache, keys
from processor import get_output, process_reading

from .helpers import add_to_device_readings, check_host_vs_mac, set_host_from_mac

logger = logging.getLogger(__name__)

DEVICE_DEFAULT_TIMEOUT = 5
DEVICE_READ_MAXTIMEOUT = 600


def get_readings(config: dict, drivers: dict):
    # Work out all the readings that need to be taken, refactored by device
    dev_rdg = {}

    for rdg in config["readings"]:
        # Ignore readings that are explicitly disabled
        # (if 'enabled' key is missing altogether, assume enabled by default)
        if not config["readings"][rdg].get("enabled", True):
            continue

        # Get device and variable name for reading; if not available then move on
        try:
            dev_id = config["readings"][rdg]["device"]
            var = config["readings"][rdg]["var"]
        except KeyError:
            continue

        # Ignore devices that are explicitly disabled in the devices configuration
        # (if 'enabled' key is missing altogether, assume enabled by default)
        if dev_id in config["devices"]:
            dev = config["devices"][dev_id]
        else:
            logger.error("Reading from device %s requested, but device not defined. Skipping" % dev_id)
            continue

        if not dev.get("enabled", True):
            continue

        # Get the driver name
        drv_id = dev["driver"]
        if drv_id not in drivers:
            logger.error(f"Reading using driver {drv_id} requested, but driver not found. Skipping device {dev_id}")
            continue

        # Save all necessary reading parameters in dev_rdg
        # dev_rdg is a dict of lists of dicts ;) :
        # 1st level: dict with the device name as the key (so we can query each device separately)
        # 2nd level: list of individual readings that need to be taken from device
        # 3rd level: for each reading, a dict determining how the reading should be taken
        if dev_id not in dev_rdg:
            dev_rdg[dev_id] = []

        # Start by setting reading name
        rdict = {"reading": rdg, "var": var}
        # If applicable, add common reading parameters from driver file (e.g. function code)
        rdict.update(drivers[drv_id].get("common", {}))

        try:
            rdict.update(drivers[drv_id]["fields"][var])
        except KeyError:
            logger.warning(f"Variable {var} not found in driver {drv_id}, or driver definition malformed.")

        if rdict.get("deprecated"):
            logger.warning(f"Use of deprecated variable {var} from driver {drv_id}")

        dev_rdg[dev_id].append(rdict)

    return dev_rdg


def get_readout(config: dict, drivers: dict):
    # 'readout' is a dict formatted for device-based readings. It also contains a timestamp, and snap_rev
    try:
        snap_rev = int(os.getenv("SNAP_REVISION", 0))
    except ValueError:  # Occurs if it's a devel snap with revision prefixed in 'x'
        snap_rev = 0

    reading_timestamp = int(datetime.now(UTC).timestamp())

    readout = {
        "t": reading_timestamp,
        "r": [],
        "m": {
            "snap_rev": snap_rev,
        },
    }

    dev_rdg = get_readings(config, drivers)
    # Set up queue in which to save readouts from the multiple threads that are reading each device
    readout_q = queue.Queue()
    jobs = []

    # Skip any devices for which min_read_interval has not yet elapsed
    with KVCache() as kvc:
        devices_to_skip = [
            dev_id
            for dev_id in dev_rdg.keys()
            if (min_read_interval := config["devices"][dev_id].get("min_read_interval"))
            and reading_timestamp - kvc.get(f"{keys.LAST_READING_TS_FOR_DEV_PFX}/{dev_id}", 0) < min_read_interval
        ]

    if devices_to_skip:
        logger.info(f"Skipping devices due to min_read_interval: {devices_to_skip}")
        for dev_id in devices_to_skip:
            del dev_rdg[dev_id]

    # Sometimes multiple "devices" will actually share the same serial port, or host IP.
    # It is best to make sure that multiple threads do not try to open concurrent
    # connections to a single port or host; in the case of a serial port at least, this
    # is bound to fail. Therefore we can create a lock for each physical port or host,
    # and ensure that the reading thread respects this lock and waits for any others
    # that are reading from the device to finish before proceeding.

    # First we need to establish the actual set of physical devices, and create a
    # lock object for each.
    locks = {}

    for dev_id, dev in config["devices"].items():
        if "address" in dev:
            # Get the device or host name if available
            d = dev["address"].get("device") or dev["address"].get("host") or dev["address"].get("mac")

            # Create a lock for this device or host name if it doesn't already exist
            if d and d not in locks:
                locks[d] = threading.Lock()

            # Set host IP based on MAC, if MAC is available
            set_host_from_mac(dev["address"])

    # Set up threads for reading each of the devices
    for dev_id in dev_rdg:
        dev = config["devices"][dev_id]
        dev.update({"id": dev_id})

        try:
            d = dev["address"].get("device") or dev["address"].get("host") or dev["address"].get("mac")
            dev_lock = locks[d]
        except KeyError:
            dev_lock = None

        dev_thread = threading.Thread(
            target=read_device, name="Readout-" + dev_id, args=(dev, dev_rdg[dev_id], readout_q, dev_lock), daemon=True
        )

        jobs.append(dev_thread)

    # Start each of the device reading jobs
    for j in jobs:
        j.start()

    # Wait until all of the reading jobs have completed
    for j in jobs:
        j.join(timeout=DEVICE_READ_MAXTIMEOUT)

    # Get the results for each device and append them to the readout structure
    for j in jobs:
        try:
            fields = readout_q.get(block=False)
            readout["r"].append(fields)
        except queue.Empty:
            logger.warning("Not all devices returned readings")

    logger.debug(f"Populated readings for all devices: {dev_rdg}")
    with KVCache() as kvc:
        kvc.set(keys.LAST_READINGS, dev_rdg)
        kvc.set(keys.LAST_READINGS_TS, readout["t"])
        for dev_id in dev_rdg.keys():
            kvc.set(f"{keys.LAST_READING_TS_FOR_DEV_PFX}/{dev_id}", reading_timestamp)

    # time that took to read all devices.
    readout["m"]["reading_duration"] = datetime.now(UTC).timestamp() - reading_timestamp

    if "output" in config:
        # Get additional processed values
        output = get_output(dev_rdg, config["output"])
        logger.debug(f"Calculated outputs: {output}")
        for output_field in output:
            if output_field.get("device") in config["devices"]:
                # The field needs to be added for a known device
                add_to_device_readings(
                    readout["r"], output_field["device"], {output_field["field"]: output_field.get("value")}
                )
            else:
                # There is no known device associated; use default device and vendor ID
                add_to_device_readings(
                    readout["r"],
                    OUTPUT_READINGS_DEV_ID,
                    {
                        VENDOR_ID_KEY: config.get(CONFIG_CALC_VENDOR_ID),
                        output_field["field"]: output_field.get("value"),
                    },
                )

    logger.debug(f"Readout: {readout}")

    return readout


def read_device(dev, readings, readout_q, dev_lock=None):
    # If the device has a concurrency lock associated with it, make sure it's available
    if dev_lock:
        dev_lock.acquire()
        # If we've just finished reading another device on this port, let it breathe
        time.sleep(0.5)

    fields = {
        DEVICE_ID_KEY: dev["id"],
    }
    if "vendor_id" in dev:
        fields[VENDOR_ID_KEY] = dev["vendor_id"]

    logger.info("READ: Start reading %s" % dev["id"])

    logger.debug("Reading device %s" % dev)

    # The reading type for each of the devices can be one of the following:
    # modbustcp - ModbusTCP
    # modbusrtu or serial - RS-485 / ModbusRTU
    # rawserial - Raw serial request

    if dev["reading_type"] == "modbustcp":
        reader_config = deepcopy(dev["address"])
        reader_config["timeout"] = dev.get("timeout", DEVICE_DEFAULT_TIMEOUT)
        from reader.modbustcp_reader import Reader

    elif dev["reading_type"] == "modbusrtu" or dev["reading_type"] == "serial":
        reader_config = deepcopy(dev["address"])
        reader_config["timeout"] = dev.get("timeout", DEVICE_DEFAULT_TIMEOUT)
        from reader.modbusrtu_reader import Reader

    elif dev["reading_type"] == "rawserial":
        reader_config = deepcopy(dev["address"])
        reader_config["timeout"] = dev.get("timeout", DEVICE_DEFAULT_TIMEOUT)
        from reader.rawserial_reader import Reader

    elif dev["reading_type"] == "rawtcp":
        reader_config = deepcopy(dev["address"])
        reader_config["timeout"] = dev.get("timeout", DEVICE_DEFAULT_TIMEOUT)
        from reader.rawtcp_reader import Reader

    elif dev["reading_type"] == "mqtt":
        reader_config = deepcopy(dev["address"])
        reader_config["timeout"] = dev.get("timeout", DEVICE_DEFAULT_TIMEOUT)
        from reader.mqtt_reader import Reader

    elif dev["reading_type"] == "sma_speedwire":
        reader_config = deepcopy(dev["address"])
        reader_config["timeout"] = dev.get("timeout", DEVICE_DEFAULT_TIMEOUT)
        from reader.sma_speedwire_reader import Reader

    elif dev["reading_type"] == "sys":
        reader_config = {}
        from reader.sys_reader import Reader

    logger.debug(f"Setting up reader of type {dev['reading_type']} with config {reader_config}")

    try:
        with Reader(**reader_config) as reader:
            if not reader:
                raise Exception(f"No reader object could be created for device {dev['id']}. Skipping")

            if "address" in dev and not check_host_vs_mac(dev["address"]):
                raise Exception(f"MAC mismatch for {dev['id']}. Not reading device.")

            for rdg in readings:
                if "read_delay" in dev and isinstance(dev["read_delay"], (float, int)):
                    sleep(dev["read_delay"])

                try:
                    val_b = reader.read(**rdg)
                    if val_b is None:
                        logger.warning("READ: [%s] Returned None for reading %s" % (dev["id"], rdg["reading"]))
                        continue

                except Exception:
                    logger.exception("READ: [%s] Could not obtain reading %s. Exception" % (dev["id"], rdg["reading"]))
                    continue

                # Get processed value
                value = process_reading(val_b, **rdg)

                # Append to key-value store
                fields[rdg["var"]] = value

                # Also save within readings structure
                rdg["value"] = value

                logger.debug("READ: [%s] %s = %s %s" % (dev["id"], rdg["var"], repr(val_b), rdg.get("unit", "")))

    except Exception:
        logger.exception("Exception while reading device %s" % dev["id"])

    logger.info(f"READ: Finished reading {dev['id']}")
    # Append result to readings (alongside those from other devices)
    readout_q.put(fields)

    # If the device has a concurrency lock associated with it, release it
    # so that other threads can proceed with reading
    if dev_lock:
        dev_lock.release()
