---
title: "Docker Guide"
sidebarTitle: "Docker"
---

export const LayoutTree = ({ children }) => (
  <div className="not-prose layout-tree" style={{ margin: "1.25rem 0", fontFamily: "var(--font-sans)", fontSize: "14px", color: "var(--text-secondary)" }}>
    <style>{`
      .layout-tree summary { list-style: none; }
      .layout-tree summary::-webkit-details-marker { display: none; }
      .layout-tree .lt-row:hover { background-color: var(--surface-subtle); }
      .layout-tree details:not([open]) summary .lt-chevron { transform: rotate(-90deg); }
      .layout-tree .lt-hl { color: var(--primary); background-color: color-mix(in srgb, var(--primary) 8%, transparent); }
      .dark .layout-tree .lt-hl { color: var(--primary-light); }
      .layout-tree .lt-ic-gray { color: #84848a; }
      .dark .layout-tree .lt-ic-gray { color: #adadb1; }
      .layout-tree .lt-ic-blue { color: #1a85d4; }
      .dark .layout-tree .lt-ic-blue { color: #69b1ff; }
      .layout-tree .lt-ic-cyan { color: #1ca1c7; }
      .dark .layout-tree .lt-ic-cyan { color: #68cdf2; }
      .layout-tree .lt-ic-green { color: #199f43; }
      .dark .layout-tree .lt-ic-green { color: #5ecc71; }
      .layout-tree .lt-ic-indigo { color: #693acf; }
      .dark .layout-tree .lt-ic-indigo { color: #9d6afb; }
      .layout-tree .lt-ic-orange { color: #d47628; }
      .dark .layout-tree .lt-ic-orange { color: #ffa359; }
      .layout-tree .lt-ic-pink { color: #d32a61; }
      .dark .layout-tree .lt-ic-pink { color: #ff678d; }
      .layout-tree .lt-ic-purple { color: #a631be; }
      .dark .layout-tree .lt-ic-purple { color: #d568ea; }
      .layout-tree .lt-ic-red { color: #d52c36; }
      .dark .layout-tree .lt-ic-red { color: #ff6762; }
      .layout-tree .lt-ic-yellow { color: #d5a910; }
      .dark .layout-tree .lt-ic-yellow { color: #ffd452; }
    `}</style>
    {children}
  </div>
)

export const LayoutRow = ({ icon, name, note, highlight = false, button = false }) => (
  <div className={highlight ? "lt-row lt-hl" : "lt-row"} style={{ display: "flex", alignItems: "flex-start", gap: "6px", minHeight: "30px", padding: "5px 8px", margin: "0 2px", borderRadius: "6px", lineHeight: "20px", cursor: button ? "pointer" : "default" }}>
    <span style={{ width: "16px", height: "20px", flexShrink: 0, display: "flex", alignItems: "center", justifyContent: "center", color: highlight ? "inherit" : "var(--text-muted)" }}>{icon}</span>
    <span style={{ whiteSpace: "nowrap" }}>{name}</span>
    {note ? <span style={{ color: "var(--text-muted)", marginLeft: "6px" }}>{note}</span> : null}
  </div>
)

export const LayoutChevron = () => (
  <svg className="lt-chevron" viewBox="0 0 16 16" width="16" height="16" aria-hidden="true" style={{ transition: "transform .15s" }}>
    <path fill="currentColor" d="M12.4697 5.46973C12.7626 5.17684 13.2374 5.17684 13.5303 5.46973C13.8232 5.76262 13.8232 6.23738 13.5303 6.53028L8.53028 11.5303C8.23738 11.8232 7.76262 11.8232 7.46973 11.5303L2.46973 6.53028C2.17684 6.23738 2.17684 5.76262 2.46973 5.46973C2.76262 5.17684 3.23738 5.17684 3.53028 5.46973L8 9.93946L12.4697 5.46973Z" />
  </svg>
)

export const LayoutFileIcon = () => (
  <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
    <path fill="currentColor" opacity=".5" d="M8 1v3a3 3 0 0 0 3 3h3v5.5a2.5 2.5 0 0 1-2.5 2.5h-7A2.5 2.5 0 0 1 2 12.5v-9A2.5 2.5 0 0 1 4.5 1z" />
    <path fill="currentColor" d="M9.5 1a.5.5 0 0 1 .354.146l4 4A.5.5 0 0 1 14 5.5V6h-3a2 2 0 0 1-2-2V1z" />
  </svg>
)

export const LayoutDir = ({ name, note, open = false, highlight = false, children }) =>
  children ? (
    <details open={open}>
      <summary>
        <LayoutRow icon={<LayoutChevron />} name={name} note={note} highlight={highlight} button />
      </summary>
      <div style={{ marginLeft: "17.5px", borderLeft: "1px solid var(--border-subtle)", paddingLeft: "6px" }}>
        {children}
      </div>
    </details>
  ) : (
    <LayoutRow icon={<span style={{ display: "flex", transform: "rotate(-90deg)" }}><LayoutChevron /></span>} name={name} note={note} highlight={highlight} />
  )

export const layoutIconColor = (name) => {
  const n = String(name).toLowerCase()
  const ext = n.includes(".") ? n.split(".").pop() : ""
  if (n === "dockerfile" || n.startsWith("dockerfile.") || n.startsWith("docker-compose")) return "blue"
  if (["rs", "json", "jsonc", "html", "htm", "zip", "gz", "tgz", "crx", "xpi", "swift", "svg"].includes(ext)) return "orange"
  if (["ts", "tsx", "mts", "cts", "py"].includes(ext)) return "blue"
  if (["js", "mjs", "cjs", "jsx"].includes(ext)) return "yellow"
  if (["md", "mdx", "sh", "bash", "zsh", "fish", "ps1"].includes(ext)) return "green"
  if (["css", "scss"].includes(ext)) return "indigo"
  if (["png", "jpg", "jpeg", "gif", "webp", "ico"].includes(ext)) return "pink"
  if (ext === "go") return "cyan"
  return "gray"
}

export const LayoutFile = ({ name, note, highlight = false }) => (
  <LayoutRow icon={<span className={highlight ? "" : "lt-ic-" + layoutIconColor(name)} style={{ display: "flex" }}><LayoutFileIcon /></span>} name={name} note={note} highlight={highlight} />
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
