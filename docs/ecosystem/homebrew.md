---
title: "Homebrew Tap"
---

# Homebrew tap for seleniumbase-rs

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
- a Fig completion spec (`$(brew --prefix)/share/sbrs/fig/sbrs.ts`) for
  inshellisense and other Fig-spec tools, and a usage spec
  (`$(brew --prefix)/share/sbrs/usage/sbrs.usage.kdl`) for jdx/usage and mise;
- Warp Workflows in `$(brew --prefix)/share/sbrs/warp-workflows`; copy them to
  `~/.warp/workflows` to see them in Warp's command palette.

Upgrade with `brew upgrade sbrs`. To try the latest `main`:
`brew install --HEAD mustcodeal/tap/sbrs`.

<Tip>

If zsh does not complete `sbrs`, make sure Homebrew's completions are on your
`fpath` before `compinit` (`brew shellenv` sets this up), then open a new tab.

</Tip>

For building from source instead, see [Building and installing](/building).
