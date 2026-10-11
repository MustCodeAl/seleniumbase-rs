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

From the repository root:

```bash
docker build -t seleniumbase-rs .
```

## Run the CLI

The runtime image contains the compiled `sbase` binary and Chromium. Run a
headless command, for example:

The image's entry point is `sbase`, so the arguments after the image name are
`sbase` arguments:

```bash
docker run --rm seleniumbase-rs --help
```

## Docker Compose

```yaml
services:
  tests:
    build: .
    environment:
      - SB_HEADLESS=true
    command: ["--help"]
```

Run with Compose:

```bash
docker compose up --build
```

## CI/CD

See `.github/workflows/` for the GitHub Actions jobs:

- `ci.yml` — format, lint, build and test on Linux, macOS and Windows.
- `docker.yml` — build the Docker image.
- `completions.yml` — generate the shell completions.

More on running in CI and containers: [Deployment](../deployment.md).

## Headless in Docker

Always run browsers in headless mode inside containers (`SB_HEADLESS=true`).
The image does not add `--no-sandbox` for you: if Chromium fails with a sandbox
error, set `SB_NO_SANDBOX=1` (or `LaunchOptionsBuilder::no_sandbox(true)` in the
pure CDP engine). That weakens Chrome's isolation, so use it only in a container
you control.

```rust
let config = BrowserConfig::default().with_headless(true);
```

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| `chromedriver` not found in container | Wrong base image stage | Use the runtime stage produced by the Dockerfile. |
| Browser crashes with sandbox error | Chrome cannot start its sandbox in the container | Set `SB_NO_SANDBOX=1` (see above). |
| Browser crashes under load | Small default `/dev/shm` | Start the container with `--shm-size=1g`. |
| Environment variable ignored | Used `SBASE_` prefix | Use the `SB_` prefix. |
