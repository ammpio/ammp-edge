FROM rust:bullseye AS rust-builder

WORKDIR /code
COPY rust .

RUN cargo build --release

FROM python:3.12-bullseye

COPY --from=rust-builder /code/target/release/ae /usr/local/bin/

WORKDIR /srv/ammp-edge

RUN apt update && \
    apt install -y nmap && \
    rm -rf /var/lib/apt/lists/*

COPY drivers drivers
COPY resources resources
COPY tests/config/provisioning provisioning
COPY src src

COPY tests/bin/run-process.sh /usr/local/bin/

WORKDIR src

RUN pip install .

ENTRYPOINT [ "run-process.sh" ]
