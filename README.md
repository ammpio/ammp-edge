# AMMP Edge app

ammp-edge is a component of AMMP, the Asset Monitoring and Management Platform: [www.ammp.io](https://www.ammp.io). ammp-edge can run on a device connected to an energy system, and gathers performance data over a range of protocols. ammp-edge is largely hardware-agnostic.

## Installing and running for production use

For production, ammp-edge is designed to be installed and run as [a snap](https://snapcraft.io). See https://snapcraft.io/ammp-edge for details, including installation instructions for the relevant environment.

CI/CD is set up so that commits to the `main` branch are automatically built and released to the snap store under the `edge` channel. After testing, the snap is promoted to the `beta`/`stable` channels as appropriate.

## Usage documentation

This is available the [AMMP Platform Documentation](https://ammpio.atlassian.net/wiki/spaces/APD).

If you are a third party who would like to use this software and access the documentation, please reach out to contact@ammp.io.

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

### Drivers and driver development

Readings from supported devices are based on drivers for the relevant devices. The [drivers](drivers) directory contains a number of JSON files, each being such a device driver. These are used to map a particular variable to be read to the underlying method of reading it from the device.

Drivers can be either built-in (when present in this repo) or add-ons (when supplied via a configuration). In general, drivers are initially tested as add-ons, before being finalized and incorporated into the repository.

## Overview of codebase and operation

The application was originally written in Python, with various components being migrated to Rust more recently. The Python code is under `src`, while the Rust code is under `rust`. Broadly speaking, the current split of functionality is as follows:
- Provisioning, configuration, and control: Rust
- Interfacing with devices and taking data readings: Python
- Local Web UI: Python

### Environment variables

The following variables are used to define operation:
- `LOG_LEVEL`: set to e.g. `debug` or `info` (the default)
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

The following approaches can be used for convenience during development. Note that you some [environment variables](#environment-variables) may need to be set in order to ensure proper operation.

### Python Development Environment

If you would prefer to run the Python portion of the code directly, rather than building and running it in Docker, you can set up the environment as follows:

```bash
make python-dev-setup
```

### Running Python code

See the `setup.py` file for the available entrypoints.

The code has been most extensively tested on Python 3.10, but should run on most recent versions.

### Code Quality Tools

The following tools are configured for code quality:

**Black** and **isort** for code formatting:
```bash
make python-format
```

### Running Rust code

After installing Rust, enter the `rust` directory and run
```
cargo run
```
This will output the available subcommands (which can be viewed as entry points); you can run any of them with e.g. `cargo run mqtt-pub-meta`.

### Web UI service

The Web UI service provides an interface to setup the dataloggers.

This service is a `Flask` application that uses Jinja2 as template engine and Python 3.10.

*Create a new page within `web-ui` service*

1. Create a new route endpoint in `web_ui/__init__.py` for the new page
2. Create a template file for this new endpoint in `web_ui/templates/`. Make sure to extend the `base.html` so that the new page can inherit the `style` or `scripts` from `base` page
3. Depending on the user flow, add link to this new page either in `templates/index.html` or other pages.

*Testing the newly updated page(s)*

- Locally, the application runs on port `8000`. Simply navigate to `localhost:8000` to checkout the Web UI application. 
- To test the Web UI on datalogger, follow the instruction from the article [Accessing a datalogger's local Web UI remotely](https://ammpio.atlassian.net/wiki/spaces/APD/pages/2463399969/Accessing+a+datalogger+s+local+Web+UI+remotely)
