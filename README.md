# AMMP Edge app

ammp-edge is a component of AMMP, the Asset Monitoring and Management Platform: [www.ammp.io](https://www.ammp.io). ammp-edge can run on a device connected to an energy system, and gathers performance data over a range of protocols. ammp-edge is largely hardware-agnostic.

## Installing and running for production use

For production, ammp-edge is designed to be installed and run as [a snap](https://snapcraft.io). Commits to the `main` branch are automatically built and released to the snap store under the `edge` channel, and promoted to the `beta`/`stable` channels after testing.

See the Snapcraft documentation for further information on snap setup and service execution.

## Running locally for development

After cloning the repository, you can run
```
make docker-run
```
in order to build and run the relevant processes in a Docker environment.

### Application components

The following Docker containers are run:
- The `ammp-edge-main` application container
- The `ammp-edge-web-ui` user interface, which can be accessed on port 8000
- A `mosquitto` container with an MQTT broker used for local interfacing between different parts of the application (in a testing environment this is not bridged to the AMMP broker)
- A `mock-sma-stp` container that emulates the ModbusTCP interface on an SMA PV inverter, and is used for testing

## Overview of codebase and operation

The application was originally written in Python, with various components being migrated to Rust more recently. The Python code is under `src`, while the Rust code is under `rust`. Broadly speaking, the current split of functionality is as follows:
- Provisioning, configuration, and control: Rust
- Interfacing with devices and taking data readings: Python
- Local Web UI: Python

### Environment variables

The following variables are used to define operation:
- `LOGGING_LEVEL`: set to e.g. `debug` or `info` (the default)
- `AE_ROOT_DIR`: the root of the application; by default mapped to `$SNAP` in production, or `.` otherwise (assumed read-only)
- `AE_DATA_DIR`: the data storage directory; by default mapped to `$SNAP_COMMON` in production, or `$AE_ROOT_DIR/data` (assumed read-write, non-volatile)
- `AE_TEMP_DIR`: a temporary directory; by default mapped to `/tmp` (assumed read-write, volatile)
- `MQTT_BRIDGE_HOST` and `MQTT_BRIDGE_PORT`: the hostname and port of the local MQTT broker; defaults to `localhost:1883`

### Local interfaces between application components

Three local interfaces are in use for inter-process communication:
- The MQTT broker running locally
- A persistent key-value store implemented in SQLite, under `$AE_DATA_DIR/kvs-db/kvstore.db`
- A volatile key-value cache implemented in SQLite, under `$AE_TEMP_DIR/ae-kvcache.db`

### Remote interfaces

There are two main remote interfaces:

#### HTTP API
As needed, the application establishes a connection to the ammp-edge API. The default path is https://edge.ammp.io/api/v0, but can be overridden by setting the `http_api_base_url` key in the key-value store.

This is used for provisioning, and can be used for on-demand metadata/configuration retrieval by the ammp-edge application. 

#### MQTT bridge
After provisioning, a persistent MQTT bridge is established between the local MQTT broker and the ammp-edge broker in the cloud. There are two pre-configured bridges:
- Bridge to production broker at mqtt.ammp.io:8883. Enabled or disabled via `mqtt_bridge_prod` boolean in key-value store. Enabled by default
- Bridge to staging broker at mqtt.stage.ammp.io:8883. Enabled or disabled via `mqtt_bridge_stage` boolean in key-value store. Disabled by default.
The above can be used in any combination (e.g. it's possible to enable production and staging at the same time).

The MQTT bridge is used for the majority of interfacing with the cloud, such as:
- Publishing ammp-edge metadata upon service start
- Publishing data read out by the ammp-edge
- Publishing environment scans
- Receiving new configurations
- Receiving commands

## Local development without Docker

The following approaches can be used for convenience during development. Note that you some [#environment-variables](environment variables) may need to be set in order to ensure proper operation.

### Running Python code

If you would prefer to run the Python portion of the code directly, rather than building and running it in Docker, you can set up the environment as follows:
```
python -m venv venv
. venv/bin/activate
cd src
pip install -U . --extra-index-url https://ammplipy.ammp.io/
```
The extra index is currently needed to obtain builds of [https://pypi.org/project/pyjsonata/](pyjsonata) for relevant architectures/Python versions.

See the `setup.py` file for the available entrypoints.

The code has been most extensively tested on Python 3.10, but should run on most recent versions.

### Running Rust code

After installing Rust, enter the `rust` directory and run
```
cargo run
```
This will output the available subcommands (which can be viewed as entry points); you can run any of them with e.g. `cargo run mqtt-pub-meta`.
