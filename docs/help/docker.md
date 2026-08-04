# Docker Guide

Run SeleniumBase for Rust tests inside a container for reproducible CI/CD builds.
This guide covers building the image, running the CLI, using Docker Compose, and
configuring CI.

## What you will learn

- How to build the Docker image.
- How to run `sbase` inside a container.
- How to write a Docker Compose service.
- How to run headless browsers in Docker.

## Build the image

From the `rust-port` directory:

```bash
docker build -t seleniumbase-rs .
```

From the repository root:

```bash
docker build -f rust-port/Dockerfile -t seleniumbase-rs ./rust-port
```

## Run the CLI

The runtime image contains the compiled `sbase` binary and Chromium. Run a
headless command, for example:

```bash
docker run --rm seleniumbase-rs sbase --help
```

## Docker Compose

```yaml
services:
  tests:
    build: .
    environment:
      - SB_HEADLESS=true
    command: ["sbase", "--help"]
```

Run with Compose:

```bash
docker compose up --build
```

## CI/CD

See `.github/workflows/` for ready-to-use GitHub Actions jobs:

- `build.yml` — build and test on every push.
- `clippy.yml` — lint check.
- `examples.yml` — run example suite.
- `docker-rs.yml` — build and push the Rust Docker image (repository root only).

## Headless in Docker

Always run browsers in headless mode inside containers; the image already adds
`--no-sandbox` for Chromium when headless mode is enabled:

```rust
let config = BrowserConfig::default().with_headless(true);
```

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| `chromedriver` not found in container | Wrong base image stage | Use the runtime stage produced by the Dockerfile. |
| Browser crashes with sandbox error | Missing `--no-sandbox` | Enable headless mode or pass `--no-sandbox` via extra args. |
| Environment variable ignored | Used `SBASE_` prefix | Use the `SB_` prefix. |
