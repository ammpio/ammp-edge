## AMMP Edge app

[![Snap Status](https://build.snapcraft.io/badge/ammpio/ammp-edge.svg)](https://build.snapcraft.io/user/ammpio/ammp-edge)

**TODO: This is placeholder documentation. To be updated**

ammp-edge is a part of AMMP, the Asset Monitoring and Management Platform: [www.ammp.io](https://www.ammp.io). ammp-edge can run on a device connected to a remote energy-generation asset, and gathers performance data over a range of protocols. ammp-edge is largely hardware-agnostic.

### Installing and running
ammp-edge is designed to be installed and run as [a snap](https://snapcraft.io). Commits to the `master` branch are automatically built and released to the snap store under the `edge` channel, and promoted to the `beta`/`candidate`/`stable` channels after testing. The build status is shown above.

To install and run ammp-edge on a system with the `snapd` package manager installed (e.g. Ubuntu Core or Ubuntu 16.04 or newer), simply run
```
snap install ammp-edge
```
If `snapd` is not installed, you can install it on [most common flavors of Linux](https://docs.snapcraft.io/core/install) with
```
sudo apt update
sudo apt install snapd
```

After installation, ammp-edge should run automatically as a daemon. You can check its status and follow its logs with
```
snap services ammp-edge
```
and
```
snap logs ammp-edge
```

It is also possible to clone the repository and run `ammp-edge.py` directly, for testing purposes. The software will recognize that it is not run in a snap environment and behave accordingly.

Currently, ammp-edge pulls its configuration from the AMMP API. Further documentation on this, and on alternative means of configuration, will be provided (TODO).

### Data collection protocols
The following protocols are currently supported:
- ModbusTCP
- RS-485 / RS-232 / ModbusRTU
- SNMP

