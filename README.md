Modbus data logger

`datalog.py` carries out periodic Modbus polling according to contents of `readings.json`, `devices.json`, and `drivers.json` in `conf` directory. Readings are pushed to an Influx database or a custom endpoint (which must accept Influx-style JSON input). If immediate pushing is not available, JSON strings corresponding to the readings are saved in the selected queue file (default `/tmp/datalog_queue.json`)

The script can be installed as a daemon. The following commands can be used to set it up. More details at https://watson.eon-ogs.com/display/DC/EOGS+Datalogger.

Prerequisites:
- The `/opt/datalog` directory needs to exist
- The `datalog` user needs to exist
- The `datalog` user needs to have read access to `/opt/datalog` and read/write access to `/tmp`
- The user cloning the GitLab repository needs to have their SSH key installed in GitLAb

Run the following as either `root` or `datalog`:
```
cd /opt/datalog
git clone git@gitlab.com:ammp-services/datalog.git
mv datalog src
```
The following needs to be run as root (or with sudo) in order to install the service and set it to start on bootup:
```
ln -s /opt/datalog/src/datalog-svc.sh /etc/init.d/
update-rc.d datalog-svc.sh defaults
```
To start the service manually:
```
service datalog-svc start
```