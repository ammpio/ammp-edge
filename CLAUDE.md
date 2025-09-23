# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Common Development Commands

### Docker Development (Recommended)
```bash
make docker-run          # Build and run all services in Docker
make docker-build        # Build Docker images
make docker-clean        # Stop containers and remove images
```

### Python Development
```bash
make python-dev-setup     # Set up Python environment with uv
make python-format       # Format code with ruff
make python-lint         # Run linting with ruff
make python-lint-fix     # Run linting with auto-fix
make python-typecheck    # Run type checking with ty
make python-static-test   # Run all static analysis (ruff + ty)
```

### Rust Development
```bash
cd rust && cargo run      # Show available Rust subcommands
cargo run init            # Initialize edge application
cargo run kvs-get <key>   # Get key from key-value store
cargo run kvs-set <key> <value>  # Set key-value pair
make test                 # Run Rust tests
```

### Code Quality
- Python formatting: `make python-format` (uses ruff)
- Python linting: `make python-lint` (uses ruff)
- Python type checking: `make python-typecheck` (uses ty)
- Python static analysis: `make python-static-test` (ruff + ty)
- Line length: 120 characters (configured in pyproject.toml)
- Target Python version: 3.12
- Package management: uv (replaces pip + venv)

## Architecture Overview

### Hybrid Language Architecture
This is a **hybrid Python/Rust application** where functionality is split as follows:
- **Rust** (`rust/` directory): Provisioning, configuration, control, key-value store management
- **Python** (`src/` directory): Device interfacing, data readings, local Web UI

### Key Components

#### Python Components (`src/`)
- **Main Application**: `ammp_edge.py` - Primary Python entry point
- **Data Management**: `data_mgmt/` - Data processing and storage
- **Device Readers**: `reader/` - Protocol implementations for device communication
- **Web UI**: `web_ui/` - Flask-based local interface (port 8000)
- **Environment Scanning**: `env_scan_svc/` - Device discovery service
- **Node Management**: `node_mgmt/` - Edge node configuration

#### Rust Components (`rust/src/`)
- **Commands**: `command/` - CLI subcommands for edge operations
- **Interfaces**: `interfaces/` - MQTT and HTTP API communication
- **Data Management**: `data_mgmt/` - Data handling in Rust
- **Key-Value Store**: Local SQLite-based storage system

#### Device Drivers (`drivers/`)
- JSON-based device driver definitions
- Maps device variables to reading methods
- Can be built-in (in repo) or add-ons (via configuration)

### Inter-Process Communication
1. **MQTT Broker**: Local mosquitto instance for service communication
2. **Key-Value Store**: Persistent SQLite database at `$AE_DATA_DIR/kvs-db/kvstore.db`
3. **Cache**: Volatile SQLite cache at `$AE_TEMP_DIR/ae-kvcache.db`

### Remote Interfaces
1. **HTTP API**: Default to https://edge.ammp.io/api/v0 (configurable via `http_api_base_url` key)
2. **MQTT Bridge**: Connects to production (mqtt.ammp.io:8883) or staging (mqtt.stage.ammp.io:8883)

## Environment Variables
- `LOG_LEVEL`: debug/info (default: info)
- `AE_ROOT_DIR`: Application root (default: $SNAP or .)
- `AE_DATA_DIR`: Data storage (default: $SNAP_COMMON or $AE_ROOT_DIR/data)
- `AE_TEMP_DIR`: Temporary directory (default: /tmp)
- `MQTT_BRIDGE_HOST`/`MQTT_BRIDGE_PORT`: Local MQTT broker (default: localhost:1883)

## Python Entry Points
Available via `pyproject.toml`:
- `ammp_edge`: Main edge application
- `wifi_ap_control`: WiFi access point management
- `env_scan_svc`: Environment scanning service

## Development Notes
- **Production deployment**: Uses snap packages with automatic CI/CD from `main` branch
- **Local Web UI**: Accessible on port 8000 when running
- **Mock services**: `mock-sma-stp` container provides ModbusTCP testing interface
- **Driver development**: Test drivers as add-ons before incorporating into repository
- **Python version**: Targets Python 3.12
- **Package management**: Uses uv for fast dependency resolution and virtual environment management
- **Code quality**: Uses ruff for formatting/linting and ty for type checking