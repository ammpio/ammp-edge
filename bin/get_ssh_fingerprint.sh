#!/bin/bash -e

ssh-keygen -lf <(ssh-keyscan -t ecdsa -T 5 localhost 2>/dev/null) 2>/dev/null | cut -d ' ' -f 2 | cut -d ':' -f 2
