import logging
import struct

logger = logging.getLogger(__name__)


def process_reading(val_b: bytes, **rdg):
    """
    Process reading obtained from device. Inputs:
    val_b: Reading value - in most cases a bytes object;
      if it is not bytes, the input is taken as is and not converted
    rdg: Optional dict with further parameters for processing
      Currently supported parameters:
      - parse_as: 'bytes' (default if omitted), 'str', or 'hex';
        'str' is used when numerical value is represented as a string (e.g. b'12.3' -> 12.3)
        'hex' is used when a bytes values is represented as a hex string (e.g. b'0a12' -> 2578)
      - datatype: used when processing bytes or hex values; one of
        'int16', 'uint16', 'int32', 'uint32', 'uint64', 'float', 'single', 'double'
      - typecast: used when obtaining value from string; one of
        'int', 'float', 'str', 'bool'
      - valuemap:
        - for 'bytes' and 'hex' values: a dict where the keys are hex values of
        the form '0x123abc' - the leading '0x' is required and the representation
        should be lowercase. If bytes value mathes any of the keys in the dict, the
        corresponding (mapped) value from the dict is returned
        - for 'string' values: as above, but the map keys are literal string values
      - multiplier: applies a multiplier to reading
      - offset: applies an offset to reading (after application of multiplier),
        i.e. output = multiplier * reading + offset
    """

    if isinstance(val_b, bytes):
        value = parse_val_b(val_b, **rdg)
    else:
        value = val_b

    # Don't do further processing if we don't have a value
    if value is None:
        return None

    # Apply multiplier and offset, unless we're dealing with a string or boolean value
    if rdg.get("typecast") not in ["str", "bool"]:
        value = apply_mult_offset(value, **rdg)

    value = typecast(value, **rdg)

    return value


def parse_val_b(val_b: bytes, **rdg):
    if rdg.get("parse_as") == "str":
        try:
            val_s = val_b.decode("utf-8")
        except UnicodeDecodeError:
            logger.error(f"Could not decode {repr(val_b)} into a string")
            return
        value = value_from_string(val_s, **rdg)

    elif rdg.get("parse_as") == "hex":
        try:
            val_h = val_b.decode("utf-8")
            val_b = bytes.fromhex(val_h)
        except UnicodeDecodeError:
            logger.error(f"Could not decode {repr(val_b)} into a string")
            return
        except ValueError:
            logger.error(f"Could not parse {val_h} as a hex value")
            return
        value = value_from_bytes(val_b, **rdg)

    else:
        value = value_from_bytes(val_b, **rdg)

    return value


def value_from_bytes(val_b: bytes, **rdg):
    # Format identifiers used to unpack the binary result into desired format based on datatype
    fmt = {
        "int16": "h",
        "uint16": "H",
        "int32": "i",
        "uint32": "I",
        "int64": "q",
        "uint64": "Q",
        "float": "f",
        "single": "f",
        "double": "d",
    }
    # If datatype is not available, fall back on format characters based on data length (in bytes)
    fmt_fallback = [None, "B", "H", None, "I", None, None, None, "d"]

    # Check for defined value mappings in the driver
    # NOTE: The keys for these mappings must be HEX strings
    if "valuemap" in rdg:
        # NOTE: Currently only mapping against hex representations works
        # Get hex string representing byte reading
        val_h = "0x" + val_b.hex()

        # If the value exists in the map, return
        if val_h in rdg["valuemap"]:
            return rdg["valuemap"][val_h]

    # Get the right format character to convert from binary to the desired data type
    if rdg.get("datatype") in fmt:
        fmt_char = fmt[rdg["datatype"]]
    else:
        fmt_char = fmt_fallback[len(val_b)]

    # Convert
    value = struct.unpack(">%s" % fmt_char, val_b)[0]

    return value


def value_from_string(val_s: str, **rdg):
    # Check for defined value mappings in the driver
    if "valuemap" in rdg:
        # If the string value exists as a key in the map, return
        if val_s in rdg["valuemap"]:
            return rdg["valuemap"][val_s]

    # Don't do further processing here. We rely on the typecast() function for this
    return val_s


def apply_mult_offset(value, **rdg):
    # If the raw value is a string or bool, we need to apply a typecast before any
    # of the below (and this typecast does need to be explicitly defined in the driver)
    if isinstance(value, (str, bool)):
        value = typecast(value, **rdg)

    try:
        # Apply a float multiplier if set
        if "multiplier" in rdg:
            value = value * rdg["multiplier"]

        # Apply an offset if set
        if "offset" in rdg:
            value = value + rdg["offset"]

        return value

    except Exception:
        logger.exception(f"Exception while applying multiplier and offset to {value}. Parameters: {rdg}")
        return None


def typecast(value, **rdg):
    if value is None:
        return None

    if "typecast" in rdg:
        if rdg["typecast"] in ["int", "float", "str", "bool"]:
            typecast_fn = {"int": int, "float": float, "str": str, "bool": bool}[rdg["typecast"]]
        else:
            logger.warn(
                f"Not applying invalid typecast value {rdg['typecast']}. Must be one of 'int', 'float', 'str', 'bool'."
            )
            return value
    else:
        return value

    try:
        value = typecast_fn(value)
    except ValueError:
        logger.error(f"Could not parse {value} as value of type {rdg['typecast']}")
        return

    return value
