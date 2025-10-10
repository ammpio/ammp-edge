listener 1883 127.0.0.1
allow_anonymous true
user root

persistence true
persistence_location $MOSQUITTO_DIR
autosave_interval 300

max_inflight_messages 2
max_queued_messages 525600

# No keepalive pings needed for local broker connections
keepalive_interval 3600

log_type all
log_timestamp false

include_dir $INCLUDE_DIR