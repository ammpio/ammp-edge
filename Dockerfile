FROM rust:bullseye AS rust-builder

WORKDIR /code
COPY rust .

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/code/target \
    cargo build --release && \
    cp target/release/ae /code/ae

FROM python:3.12-bullseye AS python-builder

RUN pip install uv

WORKDIR /build

COPY pyproject.toml LICENSE README.md ./
COPY src src

RUN --mount=type=cache,target=/root/.cache/uv \
    uv venv /opt/venv
ENV PATH="/opt/venv/bin:$PATH"
RUN --mount=type=cache,target=/root/.cache/uv \
    uv pip install .

FROM python:3.12-bullseye

RUN apt update && \
    apt install -y nmap && \
    rm -rf /var/lib/apt/lists/*

COPY --from=rust-builder /code/ae /usr/local/bin/

COPY --from=python-builder /opt/venv /opt/venv

ENV PATH="/opt/venv/bin:$PATH"

WORKDIR /srv/ammp-edge

COPY drivers drivers
COPY resources resources
COPY tests/config/provisioning provisioning

COPY tests/bin/run-process.sh /usr/local/bin/

ENTRYPOINT [ "run-process.sh" ]
