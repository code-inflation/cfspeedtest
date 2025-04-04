FROM --platform=$BUILDPLATFORM rust:slim-bullseye as builder

# Install cross-compilation tools if needed
ARG BUILDPLATFORM
ARG TARGETPLATFORM
RUN if [ "$TARGETPLATFORM" = "linux/arm64" ] && [ "$BUILDPLATFORM" = "linux/amd64" ]; then \
    dpkg --add-architecture arm64 && \
    apt-get update && \
    apt-get install -y --no-install-recommends \
    gcc-aarch64-linux-gnu libc6-dev-arm64-cross && \
    rustup target add aarch64-unknown-linux-gnu && \
    rm -rf /var/lib/apt/lists/*; \
    fi

# Set the correct target
ARG RUST_TARGET="x86_64-unknown-linux-gnu"
RUN if [ "$TARGETPLATFORM" = "linux/arm64" ]; then \
    echo "RUST_TARGET=aarch64-unknown-linux-gnu"; \
    export RUST_TARGET="aarch64-unknown-linux-gnu"; \
    fi

# Create a new empty project for caching dependencies
WORKDIR /usr/src/cfspeedtest
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src && \
    echo "fn main() {}" > src/main.rs && \
    echo "pub fn dummy() {}" > src/lib.rs && \
    cargo fetch

# Build the actual application
COPY src ./src
RUN if [ "$TARGETPLATFORM" = "linux/arm64" ]; then \
    RUSTFLAGS="-C linker=aarch64-linux-gnu-gcc" \
    cargo build --release --target aarch64-unknown-linux-gnu && \
    cp target/aarch64-unknown-linux-gnu/release/cfspeedtest /usr/local/bin/; \
    else \
    cargo build --release && \
    cp target/release/cfspeedtest /usr/local/bin/; \
    fi

FROM --platform=$TARGETPLATFORM debian:bullseye-slim
RUN apt-get update && apt-get install -y --no-install-recommends tini && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/bin/cfspeedtest /usr/local/bin/cfspeedtest

# tini will be PID 1 and handle signal forwarding and process reaping
ENTRYPOINT ["/usr/bin/tini", "--", "cfspeedtest"]
