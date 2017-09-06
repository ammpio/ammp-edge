Modbus data logger

`datalog2.py` carries out periodic Modbus polling according to contents of `readings.json`, `devices.json`, and `drivers.json` in `conf` directory. Readings are pushed to an Influx database, or saved as Influx-friendly JSON strings in selected queue file.

The script can be run in a daemon-like manner (to be refined):
```
cd /opt/datalog
nohup python3 -u datalog.py -d -I 60 -r > log/datalog.log 2>&1 &
```
