# Reading and Submitting Status Messages

## Overview

There is a need to read and submit status messages via dataloggers. This involves enhancements to the existing datalogger code.

## Context

These readouts are generally done over **ModbusTCP**.
One important difference with general timeseries data is that often only a part of a two-byte (16-bit) register needs to be parsed into the status level.
This may be just a **single bit**, or a **2–4 bit span**.


## Implementation

The following services need to be updated:

1. **ammp-edge**
   Update the Rust code that reads data from devices on site, to also read and submit status info.
   This includes amending drivers (see below).

## Data Definitions and Transfer

Schema changes are needed to enable this readout and transmission.

### Drivers

[DONE] Add a `status_info` and `status_info_common` top-level key with the following structure:

```json
"status_info_common": {
  "fncode": 4,
  "bit_order": "lsb", // default, can be "msb" for reverse order
  "length_bits": 1,
  "status_level_value_map": [[0, 0], [1, 3]]
},
"status_info": {
  "oil_pressure": {
    "content": "Low Oil Pressure",
    "register": 2049,
    "start_bit": 4 // within the register
  }
}
```

#### Bit Definitions

* Counts start from `0`
* By default, bit `0` is the **least significant bit** (rightmost), and bit `15` is the **most significant bit**.
* Setting `order` to `"msb"` reverses the count (left to right).

If a bit is `0`, it maps to `status_level = 0` (OK).
If a bit is `1`, it maps to `status_level = 3` (Error).
More complex mappings can be added as needed.


### Config

Add a top-level `status_readings` array with the following structure, representing pairs of device key and status info key that should be read out:

```json
"status_readings": [
  {
    "d": "dse_1",
    "r": "oil_pressure"
  },
  ...
]
```

### Data Payloads

The following data will be added to the readings array:

```json
"r": [
  {
    "_d": "dse_1",
    "_vid": "dse-1",
    // Timeseries readings...
    "_s": [
      {
        "c": "Low Oil Pressure", // Content message
        "l": 3 // Level (0–4)
      },
      ...
    ]
  }
]
```

### Optional Extensions

Future work:
* It is **desirable** for status info messages to be emitted **only when there is a change** in the status level compared to the last emitted payload. This requires storing the last state (in volatile memory / KV cache) and checking for changes before emitting.
