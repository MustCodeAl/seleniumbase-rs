# Docker Guide

Run SeleniumBase for Rust tests inside a container for reproducible CI/CD builds.

## Build the image

```bash
docker build -t seleniumbase-rs .
```

## Run a test

```bash
docker run --rm seleniumbase-rs cargo run --example basic_test
```

## Docker Compose

```yaml
version: "3"
services:
  tests:
    build: .
    environment:
      - SBASE_HEADLESS=true
    command: cargo test
```

Run with Compose:

```bash
docker-compose up --build
```

## CI/CD

See `.github/workflows/` for ready-to-use GitHub Actions jobs:

- `build.yml` — build and test on every push.
- `clippy.yml` — lint check.
- `examples.yml` — run example suite.

## Headless in Docker

Always run browsers in headless mode inside containers:

```rust
let config = BrowserConfig::default().with_headless(true);
```
