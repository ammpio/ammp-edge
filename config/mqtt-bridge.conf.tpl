connection $NODE_ID-brg

address mqtt.ammp.io:8883

topic d/# in 1 "" a/$NODE_ID/
topic u/# out 1 "" a/$NODE_ID/

bridge_protocol_version mqttv50
bridge_insecure false

cleansession false
remote_clientid $NODE_ID-brg
start_type automatic

notifications true
notification_topic u/bridge_state

remote_username $NODE_ID
remote_password $ACCESS_KEY

restart_timeout 10 120

bridge_cafile $SNAP/resources/certs/ca.crt
