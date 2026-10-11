# syntax=docker/dockerfile:1

# -----------------------------------------------------------------------------
# Build stage
# -----------------------------------------------------------------------------
FROM rust:1.99-bookworm AS builder

WORKDIR /usr/src/seleniumbase-rs

# The default build has no `gui` or `tui` feature, and none of its crates link
# a system library (the TLS stack is rustls), so the toolchain image is enough:
# no GTK, X11 or OpenSSL packages are installed.
#
# Cache dependencies by copying the manifests first and creating a skeleton that
# matches every target the manifest declares. Cargo refuses to read a manifest
# whose `[[bench]]`, `[[example]]`, `[[test]]` or `[[bin]]` files are missing, even
# when the build asks for one binary.
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src/cli/bin src/bin benches examples tests && \
    touch src/lib.rs \
          benches/cdp_latency.rs benches/cpu.rs \
          examples/dialog.rs examples/dialogs.rs examples/gui_automation.rs \
          examples/capture_tls.rs examples/capture_hints.rs examples/capture_h2.rs \
          examples/cookie_jar.rs examples/browser_scraper.rs examples/custom_profile.rs \
          examples/http_session.rs examples/stealth_session.rs \
          tests/scrape_domain_purity.rs && \
    echo 'fn main() {}' > src/cli/bin/sbase.rs && \
    echo 'fn main() {}' > src/bin/mcp_server.rs

# Build the sbase binary to download and compile all dependencies.
RUN cargo build --release --locked --bin sbase && rm -rf src

# Copy the source and build the real binary. `tests/golden` is compiled into the
# binary (the `sbase mkdir` and `sbase mkfile` templates), so it comes along.
COPY src ./src
COPY tests/golden ./tests/golden
RUN touch src/lib.rs src/cli/bin/sbase.rs
RUN cargo build --release --locked --bin sbase

# -----------------------------------------------------------------------------
# Runtime stage
# -----------------------------------------------------------------------------
FROM debian:bookworm-slim

# Chromium is all the pure-CDP engine needs. chromium-driver is for the
# WebDriver engine; drop it from images that only use `sb_cdp`.
RUN apt-get update && apt-get install -y --no-install-recommends \
    chromium \
    chromium-driver \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create a non-root user for running browsers.
RUN groupadd -r sbase && useradd -r -g sbase -m -d /home/sbase sbase

WORKDIR /home/sbase

# Copy the compiled binary.
COPY --from=builder /usr/src/seleniumbase-rs/target/release/sbase /usr/local/bin/sbase

# Nothing in this image listens on a port: `sbase` is a command-line tool and
# drives the browser over a local pipe or socket, so there is no EXPOSE. (An
# EXPOSE line also makes the build wrapper treat the image as a web app and
# probe a port that nobody answers.)
#
# Build it with `docker build --no-slim -t seleniumbase-rs .`. Slim finds what to
# keep by watching a run of the image, and `sbase --help` never starts Chromium,
# so a slimmed image would lose the browser.

USER sbase

ENTRYPOINT ["/usr/local/bin/sbase"]
CMD ["--help"]
