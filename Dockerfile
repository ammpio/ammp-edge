FROM ubuntu:22.04
# FROM python:3.10-slim

WORKDIR /srv/ammp-edge

RUN apt update && apt install -y python3 python3-pip libsnmp-dev
# RUN apt update && apt install -y libsnmp-dev

COPY drivers drivers
COPY provisioning provisioning
COPY resources resources
COPY src src

WORKDIR src

RUN pip install . --extra-index-url https://ammplipy.ammp.io/

# ENTRYPOINT [ "sleep", "600" ]
ENTRYPOINT [ "ammp_edge" ]
