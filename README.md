Modbus data logger

`datalog.py` carries out periodic Modbus polling according to contents of `readings.json`, `devices.json`, and `drivers.json` in `conf` directory. Readings are saved as Influx-friendly JSON strings in selected queue file.
`influx_push.py` takes readings from the queue file and pushes them to an Influx database, as determined by settings in `conf/dbconf.json`.

Both files can be run in a daemon-like manner (to be refined):
```
cd /opt/datalog
nohup python3 -u datalog.py -d -I 60 -r > log/datalog.log 2>&1 &
nohup python3 -u influx_push.py > log/influx_push.log 2>&1 &
```
