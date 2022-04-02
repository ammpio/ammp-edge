#!/bin/sh

export MOSQUITTO_DIR=$SNAP_COMMON/mosquitto
export INCLUDE_DIR=$MOSQUITTO_DIR/conf.d
CONFIG_FILE=$MOSQUITTO_DIR/mosquitto.conf

mkdir -p $MOSQUITTO_DIR $INCLUDE_DIR

envsubst < $SNAP/mosquitto.conf.tpl > $CONFIG_FILE

export NODE_ID=$(ammp-kvs get node_id)
export ACCESS_KEY=$(ammp-kvs get access_key)

envsubst < $SNAP/mqtt-bridge.conf.tpl > $INCLUDE_DIR/mqtt-bridge.conf

$SNAP/usr/sbin/mosquitto -c $CONFIG_FILE $@