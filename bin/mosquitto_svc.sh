#!/bin/sh

export MOSQUITTO_DIR=$SNAP_COMMON/mosquitto
export INCLUDE_DIR=$MOSQUITTO_DIR/conf.d
CONFIG_FILE=$MOSQUITTO_DIR/mosquitto.conf

mkdir -p $MOSQUITTO_DIR $INCLUDE_DIR

# Main config
envsubst < $SNAP/mosquitto.conf.tpl > $CONFIG_FILE

export NODE_ID=$(ae kvs-get node_id)
export ACCESS_KEY=$(ae kvs-get access_key)

# Bridge to prod broker - enable unless explicitly disabled
if [ $(ae kvs-get mqtt_bridge) = 'false' ]; then
  rm -f $INCLUDE_DIR/mqtt-bridge.conf
else
  envsubst < $SNAP/mqtt-bridge.conf.tpl > $INCLUDE_DIR/mqtt-bridge.conf
fi

# Bridge to stage broker - disable unless explicitly enabled
if [ $(ae kvs-get mqtt_bridge_stage) = 'true' ]; then
  envsubst < $SNAP/mqtt-bridge-stage.conf.tpl > $INCLUDE_DIR/mqtt-bridge-stage.conf
else
  rm -f $INCLUDE_DIR/mqtt-bridge-stage.conf
fi

$SNAP/usr/sbin/mosquitto -c $CONFIG_FILE $@