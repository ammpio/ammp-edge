#!/bin/bash

set -e

CMD="$1";
shift;

case "$CMD" in
    "ae-init-and-run" )
        ae init
        ammp_edge
        ;;
    "web-ui" )
        while ! test -f "$DATA_DIR/kvs-db/kvstore.db"; do
            sleep 1
            echo "Waiting for database file to be initialized"
        done
        python3 -m flask run
        ;;
    *)
    echo >&2 "Invalid option: $CMD";
    exit 1
    ;;
esac
