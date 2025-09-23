FROM rust:bullseye AS rust-builder

WORKDIR /code
COPY rust .

RUN cargo build --release

FROM python:3.12-bullseye AS python-builder

RUN pip install uv

WORKDIR /build

COPY pyproject.toml LICENSE README.md ./
COPY src src

RUN uv venv /opt/venv
ENV PATH="/opt/venv/bin:$PATH"
RUN uv pip install .

FROM python:3.12-bullseye

COPY --from=rust-builder /code/target/release/ae /usr/local/bin/

RUN apt update && \
    apt install -y nmap && \
    rm -rf /var/lib/apt/lists/*

COPY --from=python-builder /opt/venv /opt/venv

ENV PATH="/opt/venv/bin:$PATH"

WORKDIR /srv/ammp-edge

COPY drivers drivers
COPY resources resources
COPY tests/config/provisioning provisioning

COPY tests/bin/run-process.sh /usr/local/bin/

ENTRYPOINT [ "run-process.sh" ]
