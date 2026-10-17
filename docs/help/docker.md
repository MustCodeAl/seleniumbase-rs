---
title: "Docker Guide"
---

# Docker Guide

Run SeleniumBase for Rust tests inside a container for reproducible CI/CD builds.
This guide covers building the image, running the CLI, using Docker Compose, and
configuring CI.

## What you will learn

- How to build the Docker image.
- How to run `sbrs` inside a container.
- How to write a Docker Compose service.
- How to run headless browsers in Docker.

## Build the image

From the repository root:

```bash
docker build -t seleniumbase-rs .
```

## Run the CLI

The runtime image contains the compiled `sbrs` binary, Chromium and a matching
chromedriver. The image's entry point is `sbrs`, so the arguments after the image
name are `sbrs` arguments. Run it headless:

```bash
docker run --rm seleniumbase-rs --help
docker run --rm seleniumbase-rs doctor
docker run --rm --shm-size=1g seleniumbase-rs --headless smoke 'data:text/html,<title>Smoke</title><h1>hi</h1>' --title-contains Smoke
```

`sbrs open` and `smoke` need `--headless` or `SB_HEADLESS=true` in a container
(the flag beats the variable). The build, the test results on Linux arm64
and the commands that worked are in [Building](../building.md#container) and
[Operating systems](../platforms.md).

## Docker Compose

<CodeGroup>

```yaml compose.yaml
services:
  tests:
    build: .
    environment:
      - SB_HEADLESS=true
    command: ["--help"]
```

```bash Run with Compose
docker compose up --build
```

</CodeGroup>

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
pure CDP engine).

<Warning>

That weakens Chrome's isolation, so use it only in a container you control.

</Warning>

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
