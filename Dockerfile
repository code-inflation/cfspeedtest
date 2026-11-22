FROM rust:slim-trixie AS builder
WORKDIR /usr/src/cfspeedtest
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo install --locked --path .

FROM debian:trixie-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates tini && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/cfspeedtest /usr/local/bin/cfspeedtest

# tini will be PID 1 and handle signal forwarding and process reaping
ENTRYPOINT ["/usr/bin/tini", "--", "cfspeedtest"]
