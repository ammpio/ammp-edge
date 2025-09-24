# Python Typing Implementation Plan

This document outlines a comprehensive plan for implementing stronger typing in the AMMP Edge Python codebase, replacing untyped dictionaries with structured data objects like Pydantic models, NamedTuples, TypedDicts, and Dataclasses.

## Analysis Summary

The codebase currently uses extensive dictionary-based data exchange between functions, with 256 dictionary occurrences across 30 Python files. Key areas include configuration management, device readings, protocol parsing, and data processing.

## Key Data Structures That Need Typing

### 1. Configuration Data Structures
- **Main config dict** (`get_readings.py:22`) with `"readings"`, `"devices"`, `"output"` keys
- **Device configuration** with address, type, timeout settings
- **Reading definitions** with register, datatype, multiplier fields
- **Driver definitions** from JSON files

### 2. Runtime Data Structures
- **Readout data** with timestamp, readings array, metadata
- **Device readings** with device ID and key-value pairs
- **Request/response schemas** for protocol parsing
- **Address objects** with host/mac/device/port fields

## Recommended Typing Approaches

### For Configuration Objects → Pydantic Models
**Best for:** Complex validation, JSON serialization, driver configs

```python
from pydantic import BaseModel, Field
from typing import Optional, Dict, Any

class DeviceAddress(BaseModel):
    host: Optional[str] = None
    mac: Optional[str] = None
    device: Optional[str] = None  # serial device path
    port: Optional[int] = None

class DeviceConfig(BaseModel):
    id: str
    reading_type: str
    enabled: bool = True
    timeout: int = 5
    min_read_interval: Optional[int] = None
    vendor_id: Optional[str] = None
    address: DeviceAddress
```

### For Simple Data Exchange → TypedDict
**Best for:** Function parameters, return types, lightweight structures

```python
from typing_extensions import TypedDict, Required, NotRequired

class ReadingDict(TypedDict):
    reading: Required[str]
    var: Required[str]
    register: NotRequired[int]
    words: NotRequired[int]
    datatype: NotRequired[str]
    multiplier: NotRequired[float]
    unit: NotRequired[str]

class ReadoutData(TypedDict):
    t: Required[float]  # timestamp
    r: Required[list]   # readings array
    m: Required[dict]   # metadata
```

### For Value Objects → Dataclasses
**Best for:** Immutable data, simple structures with methods

```python
from dataclasses import dataclass, field
from typing import Dict, Any

@dataclass(frozen=True)
class DeviceReading:
    device_id: str
    timestamp: float
    values: Dict[str, Any]
    metadata: Dict[str, Any] = field(default_factory=dict)
```

### For Protocol Structures → NamedTuples
**Best for:** Small, immutable structures like parsing results

```python
from typing import NamedTuple, Dict

class ParseResult(NamedTuple):
    serial_number: int
    values: Dict[str, bytes]

class RequestInfo(NamedTuple):
    data: bytes
    expected_length: int
```

## Incremental Implementation Plan

### Phase 1: Foundation (Low Risk)
**Target:** `src/types.py` + Helper modules
- Create shared type definitions
- Start with `reader/helpers/add_to_device_readings.py` (already has some typing!)
- Add TypedDict for common structures like `ReadingDict`, `ReadoutData`

**Files to modify:**
- Create `src/types.py`
- `src/reader/helpers/add_to_device_readings.py`

### Phase 2: Data Processing Core (Medium Risk)
**Target:** `processor/` and `data_mgmt/`
- `processor/process_reading.py` - Add types for processing parameters
- `data_mgmt/datapusher.py` - Type the readout parameter
- `data_mgmt/helpers/mqtt_pub.py` - Type payload dictionaries

**Files to modify:**
- `src/processor/process_reading.py`
- `src/data_mgmt/datapusher.py`
- `src/data_mgmt/helpers/mqtt_pub.py`

### Phase 3: Reader Infrastructure (Medium Risk)
**Target:** `reader/` helpers and parsers
- `reader/helpers/request_response_parser.py` - Schema TypedDicts
- `reader/helpers/sma_speedwire_parser.py` - Return type improvements
- `reader/helpers/network_host_finder.py` - Address object typing

**Files to modify:**
- `src/reader/helpers/request_response_parser.py`
- `src/reader/helpers/sma_speedwire_parser.py`
- `src/reader/helpers/network_host_finder.py`

### Phase 4: Main Reading Logic (Higher Risk)
**Target:** Core reader functions
- `reader/get_readings.py` - Main config and drivers parameters
- Individual reader modules (`modbustcp_reader.py`, etc.)

**Files to modify:**
- `src/reader/get_readings.py`
- `src/reader/modbustcp_reader.py`
- `src/reader/modbusrtu_reader.py`
- `src/reader/sma_speedwire_reader.py`
- Other reader modules

### Phase 5: Configuration & Node Management (Higher Risk)
**Target:** `node_mgmt/` and config handling
- `node_mgmt/config_watch.py` - Config structure validation
- `node_mgmt/node.py` - Node state management
- Add Pydantic models for full config validation

**Files to modify:**
- `src/node_mgmt/config_watch.py`
- `src/node_mgmt/node.py`
- `src/node_mgmt/commands.py`

### Phase 6: Web UI & External Interfaces (Lower Priority)
**Target:** `web_ui/` and API interfaces
- Form data structures
- API response/request typing

**Files to modify:**
- `src/web_ui/__init__.py`
- `src/edge_api.py`

## Example Implementation for Phase 1

**Create `src/types.py`:**
```python
from typing_extensions import TypedDict, Required, NotRequired
from typing import Dict, List, Any, Optional
from dataclasses import dataclass

# Core data exchange types
class ReadingDict(TypedDict):
    reading: Required[str]
    var: Required[str]
    register: NotRequired[int]
    words: NotRequired[int]
    datatype: NotRequired[str]
    multiplier: NotRequired[float]
    unit: NotRequired[str]

class DeviceReadingEntry(TypedDict):
    _d: Required[str]  # device key
    # Dynamic fields added at runtime

class ReadoutData(TypedDict):
    t: Required[float]  # timestamp
    r: Required[List[DeviceReadingEntry]]  # readings
    m: Required[Dict[str, Any]]  # metadata

# Address structures
class DeviceAddress(TypedDict):
    host: NotRequired[str]
    mac: NotRequired[str]
    device: NotRequired[str]
    port: NotRequired[int]

# Protocol parsing types
class RequestSchema(TypedDict):
    sequence: Required[List[Dict[str, Any]]]

class ResponseSchema(TypedDict):
    check_crc16: NotRequired[bool]
    pos: NotRequired[Dict[str, Any]]
    length: NotRequired[Dict[str, Any]]
```

## Identified Dictionary Usage Patterns

### Configuration Keys
- `config["readings"][rdg]` - Reading definitions
- `config["devices"][dev_id]` - Device configurations
- `config["output"]` - Output processing rules
- `dev["address"]` - Device connection details
- `dev["reading_type"]` - Type of reader to use

### Device Address Structure
```python
address = {
    "host": "192.168.1.100",  # IP address
    "mac": "aa:bb:cc:dd:ee:ff",  # MAC address
    "device": "/dev/ttyUSB0",  # Serial device
    "port": 502  # TCP port
}
```

### Reading Dictionary Structure
```python
rdict = {
    "reading": rdg,  # Reading name
    "var": var,      # Variable name
    "register": 40001,
    "words": 2,
    "datatype": "uint32",
    "multiplier": 0.1,
    "unit": "V"
}
```

### Device Configuration Structure
```python
device_config = {
    "id": "device_001",
    "reading_type": "modbustcp",  # Reader type
    "enabled": True,
    "timeout": 5,
    "min_read_interval": 60,
    "vendor_id": "manufacturer",
    "address": {...}  # Address dict as above
}
```

## Benefits of This Approach

1. **Incremental & Safe:** Start with low-risk helpers, build confidence
2. **Mixed Strategy:** Uses the right tool for each use case
3. **IDE Support:** Better autocompletion and error detection
4. **Documentation:** Types serve as living documentation
5. **Validation:** Pydantic models provide runtime validation for configs
6. **Backward Compatible:** TypedDict and dataclasses don't break existing code

## Dependencies to Add

When implementing this plan, add these dependencies to `pyproject.toml`:

```toml
dependencies = [
    # ... existing dependencies ...
    "pydantic>=2.0.0",
    "typing-extensions>=4.0.0",
]
```

## Testing Strategy

- Use `ty` type checker to validate type annotations
- Add unit tests for Pydantic models with invalid data
- Ensure backward compatibility during gradual migration
- Use `typing.TYPE_CHECKING` for forward references if needed

## Notes

- This plan was created based on analysis of the codebase as of the current state
- The risk levels are estimates - actual risk may vary based on test coverage
- Consider creating type stubs for complex external dependencies if needed
- Some dictionary patterns may need to remain untyped if they're truly dynamic