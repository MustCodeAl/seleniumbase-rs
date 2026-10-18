---
title: "Docker Guide"
sidebarTitle: "Docker"
---

export const LayoutTree = ({ children }) => (
  <div style={{ fontSize: "0.875rem", lineHeight: 1.7, border: "1px solid rgba(127,127,127,0.25)", borderRadius: "0.75rem", padding: "0.75rem 1rem", margin: "1rem 0" }}>
    {children}
  </div>
)

export const LayoutNote = ({ note }) =>
  note ? <span style={{ opacity: 0.7, marginLeft: "0.75rem" }}>{note}</span> : null

export const LayoutDir = ({ name, note, open = false, children }) =>
  children ? (
    <details open={open} style={{ margin: "0.1rem 0" }}>
      <summary style={{ cursor: "pointer" }}>
        <Icon icon="folder" size={14} /> <code>{name}/</code>
        <LayoutNote note={note} />
      </summary>
      <div style={{ marginLeft: "0.4rem", paddingLeft: "1rem", borderLeft: "1px solid rgba(127,127,127,0.3)" }}>
        {children}
      </div>
    </details>
  ) : (
    <div style={{ margin: "0.1rem 0", paddingLeft: "1.1rem" }}>
      <Icon icon="folder" size={14} /> <code>{name}/</code>
      <LayoutNote note={note} />
    </div>
  )

export const LayoutFile = ({ name, note, highlight = false }) => (
  <div style={{ margin: "0.1rem 0", paddingLeft: "1.1rem" }}>
    <Icon icon="file" size={14} /> <code style={highlight ? { fontWeight: 700 } : undefined}>{name}</code>
    <LayoutNote note={note} />
  </div>
)

Run SeleniumBase for Rust tests inside a container for reproducible CI/CD builds.
This guide covers building the image, running the CLI, using Docker Compose, and
configuring CI.

<Info>

**What you will learn**

- How to build the Docker image.
- How to run `sbrs` inside a container.
- How to write a Docker Compose service.
- How to run headless browsers in Docker.

</Info>

<Panel>

<Card title="Key facts" icon="docker">

The image's entry point is `sbrs`: the arguments after the image name are `sbrs` arguments.

Set `SB_HEADLESS=true` (or pass `--headless`) in a container.

For a sandbox error, set `SB_NO_SANDBOX=1`. For crashes under load, use `--shm-size=1g`.

</Card>

</Panel>

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
and the commands that worked are in [Building](/building#container) and
[Operating systems](/platforms).

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

<LayoutTree>
  <LayoutDir name=".github" open>
    <LayoutDir name="workflows" open>
      <LayoutFile name="ci.yml" highlight note="format, lint, build and test on Linux, macOS and Windows" />
      <LayoutFile name="docker.yml" note="build the Docker image" />
      <LayoutFile name="completions.yml" note="generate the shell completions" />
    </LayoutDir>
  </LayoutDir>
</LayoutTree>

<Card title="Deployment" icon="server" href="/deployment" horizontal>

More on running in CI and containers.

</Card>

## Headless in Docker

Always run browsers headless inside containers. `sbrs` and code that loads
`Settings` read `SB_HEADLESS=true`; `BrowserConfig::default()` is already
headless, and the pure CDP engine is headless on Linux unless told otherwise
(see [Headless and headed browsers](/tutorials/headless)).

The WebDriver engine always adds `--no-sandbox` on Linux. The pure CDP engine
does not: if Chromium fails with a sandbox error there, set `SB_NO_SANDBOX=1`
(or `LaunchOptionsBuilder::no_sandbox(true)`).

<Warning>

That weakens Chrome's isolation, so use it only in a container you control.

</Warning>

```rust
let config = BrowserConfig::default().with_headless(true);
```

## Troubleshooting

<AccordionGroup>

<Accordion title="chromedriver not found in container" icon="box">

**Likely cause.** Wrong base image stage.

**Fix.** Use the runtime stage produced by the Dockerfile.

</Accordion>

<Accordion title="Browser crashes with sandbox error" icon="shield">

**Likely cause.** Chrome cannot start its sandbox in the container.

**Fix.** Set `SB_NO_SANDBOX=1` (see above).

</Accordion>

<Accordion title="Browser crashes under load" icon="memory">

**Likely cause.** Small default `/dev/shm`.

**Fix.** Start the container with `--shm-size=1g`.

</Accordion>

<Accordion title="Environment variable ignored" icon="gear">

**Likely cause.** Used `SBASE_` prefix.

**Fix.** Use the `SB_` prefix.

</Accordion>

</AccordionGroup>
