---
title: "Ecosystem"
sidebarTitle: "Overview"
---

seleniumbase-rs has companion projects in their own repositories. Their
documentation is part of this site.

<Visibility for="agents">

Summary for AI exports: the companion projects are seleniumbase-rs-plugins (browser plugins as Rust crates), seleniumbase-rs-stealth (experimental stealth scraping), tauri-plugin-sbrs-automation (Tauri automation), seleniumbase-rs-browser-extension (recorder, inspector and DevTools Recorder export) and homebrew-tap (`brew install mustcodeal/tap/sbrs` for `sbrs` and `sbrs-mcp`).

</Visibility>

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

<Columns cols={3}>

<Tile href="/plugins/overview" title="Plugins" description="Overview, the plugin list, writing your own, wasm, crx and block-resources.">
<Icon icon="plug" size={48} />
</Tile>

<Tile href="/stealth/overview" title="Stealth scraping" description="The tutorial, split into pages by topic.">
<Icon icon="user-secret" size={48} />
</Tile>

<Tile href="/ecosystem/tauri" title="Tauri plugin" description="A WebDriver endpoint inside your Tauri app.">
<Icon icon="window-maximize" size={48} />
</Tile>

<Tile href="/ecosystem/browser-extension" title="Browser extension" description="Recorder, inspector and DevTools Recorder export.">
<Icon icon="puzzle-piece" size={48} />
</Tile>

<Tile href="/ecosystem/homebrew" title="Homebrew tap" description="Install sbrs and sbrs-mcp with Homebrew.">
<Icon icon="beer-mug-empty" size={48} />
</Tile>

</Columns>
