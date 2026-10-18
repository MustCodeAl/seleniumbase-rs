---
title: "Homebrew Tap"
sidebarTitle: "Homebrew tap"
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

[SeleniumBase for Rust](https://github.com/MustCodeAl/seleniumbase-rs): browser
automation and testing from Rust, with the `sbrs` CLI and the `sbrs-mcp` MCP server.

<GitHub.Repo repo="MustCodeAl/homebrew-tap" />

<CodeGroup>

```bash Install
brew install mustcodeal/tap/sbrs
```

```bash Upgrade
brew upgrade sbrs
```

```bash Latest main
brew install --HEAD mustcodeal/tap/sbrs
```

</CodeGroup>

Homebrew builds it from the release source (it needs Rust, which Homebrew
installs for the build). You get:

- `sbrs` and `sbrs-mcp` on your `PATH`;
- shell completions for bash, zsh, fish and PowerShell, installed where your
  shell finds them. **Warp** uses them too: it falls back to your shell's
  completions for commands it has no built-in spec for (on by default);
- the completion specs and the Warp Workflows below, under `$(brew --prefix)`.

<LayoutTree>
  <LayoutDir name="$(brew --prefix)" open>
    <LayoutDir name="share" open>
      <LayoutDir name="sbrs" open>
        <LayoutDir name="fig" open>
          <LayoutFile name="sbrs.ts" note="A Fig completion spec, for inshellisense and other Fig-spec tools" />
        </LayoutDir>
        <LayoutDir name="usage" open>
          <LayoutFile name="sbrs.usage.kdl" note="A usage spec, for jdx/usage and mise" />
        </LayoutDir>
        <LayoutDir name="warp-workflows" note="Warp Workflows: copy them to ~/.warp/workflows to see them in Warp's command palette" />
      </LayoutDir>
    </LayoutDir>
  </LayoutDir>
</LayoutTree>

Upgrade with `brew upgrade sbrs`. To try the latest `main`:
`brew install --HEAD mustcodeal/tap/sbrs`.

<Tip>

If zsh does not complete `sbrs`, make sure Homebrew's completions are on your
`fpath` before `compinit` (`brew shellenv` sets this up), then open a new tab.

</Tip>

<Panel>

<Card title="Building and installing" icon="hammer" href="/building">

For building from source instead of Homebrew.

</Card>

</Panel>
