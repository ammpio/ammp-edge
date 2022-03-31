#!/bin/sh

export MOSQUITTO_DIR=$SNAP_COMMON/mosquitto
CONFIG_FILE=$MOSQUITTO_DIR/mosquitto.conf

mkdir -p $MOSQUITTO_DIR $MOSQUITTO_DIR/conf.d

envsubst < $SNAP/mosquitto.conf.tpl > $CONFIG_FILE

$SNAP/usr/sbin/mosquitto -c $CONFIG_FILE $@