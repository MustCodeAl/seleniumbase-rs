# syntax=docker/dockerfile:1

# -----------------------------------------------------------------------------
# Build stage
# -----------------------------------------------------------------------------
FROM rust:1.99-bookworm AS builder

WORKDIR /usr/src/seleniumbase-rs

# Cargo reads CARGO_BUILD_JOBS from the environment. Left unset, a build uses every core;
# pass `--build-arg CARGO_BUILD_JOBS=2` on a machine with little memory.
ARG CARGO_BUILD_JOBS

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
    echo 'fn main() {}' > src/cli/bin/sbrs.rs && \
    echo 'fn main() {}' > src/bin/mcp_server.rs

# Build the sbrs binary to download and compile all dependencies.
RUN cargo build --release --locked --bin sbrs && rm -rf src

# Copy the source and build the real binary. `tests/golden` is compiled into the
# binary (the `sbrs mkdir` and `sbrs mkfile` templates), so it comes along.
COPY src ./src
COPY tests/golden ./tests/golden
# The usage spec is embedded in the binary (`sbrs completions usage`).
COPY completions ./completions
RUN touch src/lib.rs src/cli/bin/sbrs.rs
RUN cargo build --release --locked --bin sbrs

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
RUN groupadd -r sbrs && useradd -r -g sbrs -m -d /home/sbrs sbrs

WORKDIR /home/sbrs

# The WebDriver engine uses `downloaded_drivers/chromedriver` in the working directory when its
# version fits the browser, and downloads a driver only when that file is missing or does not
# fit. Debian's chromium-driver is built with the Chromium installed above, so link it there:
# the image then needs no network to start a browser.
RUN mkdir -p downloaded_drivers && \
    ln -s /usr/bin/chromedriver downloaded_drivers/chromedriver && \
    chown -R sbrs:sbrs downloaded_drivers

# Copy the compiled binary.
COPY --from=builder /usr/src/seleniumbase-rs/target/release/sbrs /usr/local/bin/sbrs

# Nothing in this image listens on a port: `sbrs` is a command-line tool and
# drives the browser over a local pipe or socket, so there is no EXPOSE.
#
# Build it with `docker build -t seleniumbase-rs .`.

USER sbrs

ENTRYPOINT ["/usr/local/bin/sbrs"]
CMD ["--help"]
