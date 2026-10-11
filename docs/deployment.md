# Deployment

How to run `seleniumbase-rs` somewhere other than your laptop: a container, a CI
job, a Selenium Grid, or as an MCP server for an AI agent. To build the binaries
first, see [Building and installing](building.md).

## The shape of a deployment

`sbase` and `seleniumbase-mcp` are single native binaries with no Python or Node
runtime. What they drive is a Chrome or Chromium, so every deployment needs:

1. the binary,
2. a browser (and, for the WebDriver engine, a chromedriver of the same major
   version; the pure CDP engine needs none),
3. configuration through environment variables, and
4. somewhere for artifacts (screenshots, reports, debug bundles) to go.

Chrome, not the library, dominates memory and CPU. The process that drives the
browsers stayed near 9 MiB in the [benchmarks](benchmarks.md), so size the machine
for the browsers and for how many you run at once (`SB_THREADS`).

## Configuration is environment variables

Settings come from the environment, so the same image runs everywhere with
different values. The ones to know:

| Variable | Effect |
|---|---|
| `SB_HEADLESS` | run the browser without a window (always in containers and CI) |
| `SB_BROWSER`, `SB_MODE` | which browser and which engine mode |
| `SB_PROXY`, `SB_PROXY_PAC_URL` | route traffic through a proxy |
| `SB_THREADS` | how many tests run in parallel |
| `SB_TIMEOUT` | the default wait, in seconds |
| `SB_USER_AGENT`, `SB_LOCALE`, `SB_MOBILE` | identity settings |
| `SB_WINDOW_WIDTH`, `SB_WINDOW_HEIGHT` | window size |
| `SB_SCREENSHOT_DIR`, `SB_USER_DATA_DIR` | where artifacts and profiles go |
| `SB_NO_SANDBOX` | pass `--no-sandbox` to Chrome (see below) |
| `SB_MCP_OUTPUT_DIR` | where the MCP server may write files (default `./mcp_output`) |

Pass secrets (proxy logins, cloud credentials) through your platform's secret
store as environment variables, never into the image or the repository. A
`Proxy` with credentials does not print its password in `Debug`, `Display` or the
command line, but the process environment is still readable by whoever can run
code in the container.

## Containers

The repository's `Dockerfile` builds `sbase` and copies it into a
`debian:bookworm-slim` image with Chromium and its driver. It runs as a non-root
user, listens on no port, and its entry point is `sbase`:

```bash
docker build -t seleniumbase-rs .
docker run --rm seleniumbase-rs --help
docker run --rm -e SB_HEADLESS=true -e SB_NO_SANDBOX=1 seleniumbase-rs doctor
```

Notes:

- Because the entry point is `sbase`, the arguments after the image name are
  `sbase` arguments. Do not repeat `sbase`.
- Chrome refuses to start its sandbox in many container setups. `SB_NO_SANDBOX=1`
  (or `LaunchOptionsBuilder::no_sandbox(true)` in the pure CDP engine) passes
  `--no-sandbox`. That weakens the browser's isolation, so use it only in a
  container you control and give the container no more privileges than it needs.
- Chromium in Docker often needs a larger shared-memory area than the default;
  `docker run --shm-size=1g ...` is the usual remedy for crashes under load.
  This is general Chromium advice, not something this project measured.
- Use `sbase doctor` as a readiness check: it compares the chromedriver's major
  version with Chrome's.
- If your build pipeline optimises images by watching a run (Docker Slim),
  exclude this one; `sbase --help` never starts Chromium, so the browser would be
  removed.

## CI

`.github/workflows/ci.yml` is the working example: it builds and tests on Linux,
macOS and Windows, installs a Chrome and chromedriver for the tests that need
one, and runs the format, lint and test checks. `docker.yml` builds the image and
`completions.yml` generates shell completions. For your own pipeline the steps
are:

```bash
sbase install                     # fetch the chromedriver that matches Chrome
export SB_HEADLESS=true
cargo test --features "test-util" # tests that need no browser
cargo test -- --ignored           # the tests that launch Chrome, if you want them
```

Run browser tests in a controlled number of parallel workers and keep
`cargo nextest`'s zero-retry profile: retries hide flaky tests (see
[Reliability](reliability.md)). Upload the HTML and JSON reports and the
screenshots as build artifacts; the `s3`, `azure` and `gcp` features upload them
to a bucket directly.

## Selenium Grid

`sbase grid` starts a Selenium Grid hub or node from a Selenium Server jar that
you supply; the crate never downloads one. Point `BrowserConfig::webdriver_url`
(or the `--server` option) at the hub. See `sbase grid --help` for the options.

## The MCP server

`seleniumbase-mcp` speaks the Model Context Protocol over standard input and
output, so it is started by the client that uses it, one process per client:

```bash
seleniumbase-mcp --server cdp
```

- It does not open a network port. Do not wrap it in a public network service
  without adding your own authentication; anyone who can call its tools can
  drive the browser.
- Files its tools write stay inside `SB_MCP_OUTPUT_DIR`. Give that directory its
  own volume with a size limit.
- Point it at test or staging sites. A tool call can click and type anywhere the
  browser can reach.
- Use the `diagnose_environment` tool to check the Chrome and chromedriver
  versions, the redacted `SB_*` settings and the output directory without
  starting a browser. For using it with an agent, see
  [Vibe coding](tutorials/vibe_coding.md).

## Stopping cleanly

Run each test through `run_browser_test`, which closes the browser even when the
body fails. For long-running processes the crate keeps a shutdown registry so the
browsers it launched are closed when the process is told to stop; send `SIGTERM`
and give the process a few seconds rather than killing it, or Chrome processes
can be left behind.

## Checklist

- [ ] Browser and chromedriver versions match (or you use the pure CDP engine).
- [ ] The container runs as a non-root user, with only the privileges it needs.
- [ ] `SB_HEADLESS=true`; `SB_NO_SANDBOX` set only where required.
- [ ] Secrets come from the platform's secret store.
- [ ] Parallelism (`SB_THREADS`) fits the machine's memory.
- [ ] Reports and screenshots are collected as artifacts.
- [ ] The MCP server is reachable only by its client.
