# syntax=docker/dockerfile:1

# -----------------------------------------------------------------------------
# Build stage
# -----------------------------------------------------------------------------
FROM rust:1.82-bookworm AS builder

WORKDIR /usr/src/seleniumbase-rs

# Cache dependencies by copying manifest files first.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs
RUN cargo build --release --bin sbase && rm -rf src

# Copy source and build the real binary.
COPY src ./src
RUN touch src/cli/bin/sbase.rs
RUN cargo build --release --bin sbase

# -----------------------------------------------------------------------------
# Runtime stage
# -----------------------------------------------------------------------------
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    chromium \
    chromium-driver \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create a non-root user for running browsers.
RUN groupadd -r sbase && useradd -r -g sbase -m -d /home/sbase sbase

WORKDIR /home/sbase

# Copy the compiled binary and any runtime assets.
COPY --from=builder /usr/src/seleniumbase-rs/target/release/sbase /usr/local/bin/sbase

# Selenium/WebDriver default port.
EXPOSE 4444

USER sbase

ENTRYPOINT ["/usr/local/bin/sbase"]
CMD ["--help"]
