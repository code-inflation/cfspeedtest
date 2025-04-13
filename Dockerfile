FROM rust:slim-bullseye as builder
WORKDIR /usr/src/cfspeedtest
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo install --path .

FROM debian:bullseye-slim
RUN apt-get update && apt-get install -y --no-install-recommends tini && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/cfspeedtest /usr/local/bin/cfspeedtest

# tini will be PID 1 and handle signal forwarding and process reaping
ENTRYPOINT ["/usr/bin/tini", "--", "cfspeedtest"]
