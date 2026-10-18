---
title: "Homebrew Tap"
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
