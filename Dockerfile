FROM rust:bullseye as rust-builder

WORKDIR /code
COPY rust .
COPY resources resources

RUN cargo build --release

FROM python:3.10-bullseye

COPY --from=rust-builder /code/target/release/ae /usr/local/bin/

WORKDIR /srv/ammp-edge

RUN apt update && \
    apt install -y libsnmp-dev nmap && \
    rm -rf /var/lib/apt/lists/*

COPY drivers drivers
COPY resources resources
COPY tests/config/provisioning provisioning
COPY src src

COPY tests/bin/run-process.sh /usr/local/bin/

WORKDIR src

RUN pip install . --extra-index-url https://ammplipy.ammp.io/

ENTRYPOINT [ "run-process.sh" ]
