## AMMP Edge app

**TODO: This is placeholder documentation. To be updated**

ammp-edge is a part of AMMP, the Asset Monitoring and Management Platform: [www.ammp.io](https://www.ammp.io). ammp-edge can run on a device connected to a remote energy-generation asset, and gathers performance data over a range of protocols. ammp-edge is largely hardware-agnostic.

### Installing and running
ammp-edge is designed to be installed and run as [a snap](https://snapcraft.io). Commits to the `main` branch are automatically built and released to the snap store under the `edge` channel, and promoted to the `beta`/`candidate`/`stable` channels after testing. The build status is shown above.

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

### Running locally for development
After cloning the repository, you can run
```
make local-prepare
```
in order to set up the necessary directory structure under `.local/`, and copies of the config files that will be used. Note that you will need to have the `nmap` binary installed somewhere in your path.

Then
```
make local-run
```
to spin up the necessary Docker containers and run the `ammp-edge` code.

### Interfaces
Currently, ammp-edge pulls its configuration from the AMMP API and interfaces with the AMMP MQTT broker. This can be altered by modifying the `remote.yaml` configuration.
