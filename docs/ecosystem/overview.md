---
title: "Ecosystem"
---

# Ecosystem

seleniumbase-rs has companion projects in their own repositories. Their
documentation is part of this site.

## Plugins

<GitHub.Repo repo="MustCodeAl/seleniumbase-rs-plugins" />

Browser plugins as Rust crates: block resources, load `.crx` extensions, run WebAssembly plugins, and a template for your own. See [Plugins](/plugins/overview).

## Stealth scraping

<Badge color="orange">Experimental</Badge>

<GitHub.Repo repo="MustCodeAl/seleniumbase-rs-stealth" />

Wire fingerprints, identities, the browser scraper, `HttpSession`, `MitmProxy` and `StealthSession` as three crates. See [Stealth scraping](/stealth/overview).

## Tauri automation

<GitHub.Repo repo="MustCodeAl/tauri-plugin-sbrs-automation" />

Test a Tauri app's real window with `BaseCase` and `sbrs` on macOS, Windows and Linux. See [Tauri plugin](/ecosystem/tauri).

## Browser extension

<GitHub.Repo repo="MustCodeAl/seleniumbase-rs-browser-extension" />

A recorder, an element inspector and a DevTools Recorder export for Chrome, Edge and Firefox. See [Browser extension](/ecosystem/browser-extension).

## Homebrew tap

<GitHub.Repo repo="MustCodeAl/homebrew-tap" />

`brew install mustcodeal/tap/sbrs` for the `sbrs` CLI and the `sbrs-mcp` MCP server. See [Homebrew tap](/ecosystem/homebrew).

<Columns cols={2}>

<Card title="Plugins" icon="plug" href="/plugins/overview">

Overview, the plugin list, writing your own, wasm, crx and block-resources.

</Card>

<Card title="Stealth scraping" icon="user-secret" href="/stealth/overview">

The tutorial, split into pages by topic.

</Card>

<Card title="Tauri plugin" icon="window-maximize" href="/ecosystem/tauri">

A WebDriver endpoint inside your Tauri app.

</Card>

<Card title="Browser extension" icon="puzzle-piece" href="/ecosystem/browser-extension">

Recorder, inspector and DevTools Recorder export.

</Card>

<Card title="Homebrew tap" icon="beer-mug-empty" href="/ecosystem/homebrew">

Install `sbrs` and `sbrs-mcp` with Homebrew.

</Card>

</Columns>
