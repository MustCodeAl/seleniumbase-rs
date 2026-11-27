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

Wire fingerprints, identities, the browser scraper, `HttpSession`, `MitmProxy` and `HybridSession`. Depend on the `seleniumbase-rs-stealth` facade, not on the three implementation crates. See [Stealth scraping](/stealth/session/overview) and the [tutorial](/stealth/session/first-fetch).

## Tauri automation

<GitHub.Repo repo="MustCodeAl/tauri-plugin-sbrs-automation" />

Three guides, one per job: [test your app's real window](/ecosystem/integrations/tauri/driver) with `BaseCase` and `sbrs`, [automate a browser from your app](/ecosystem/integrations/tauri/client), and [drive the app's own Chromium webview](/ecosystem/integrations/tauri/browser) over CDP or give it a fingerprint. See [Tauri plugin](/ecosystem/integrations/tauri/overview).

## Browser extension

<GitHub.Repo repo="MustCodeAl/seleniumbase-rs-browser-extension" />

A recorder, an element inspector and a DevTools Recorder export for Chrome, Edge and Firefox. See [Browser extension](/ecosystem/integrations/browser-extension).

## Homebrew tap

<GitHub.Repo repo="MustCodeAl/homebrew-tap" />

`brew install mustcodeal/tap/sbrs` for the `sbrs` CLI and the `sbrs-mcp` MCP server. See [Homebrew tap](/ecosystem/integrations/homebrew).

<Columns cols={3}>

<Card title="Plugins" icon="plug" color="#d47628" href="/plugins/overview">
  Overview, the plugin list, writing your own, wasm, crx and block-resources.
</Card>

<Card title="Stealth scraping" icon="user-secret" color="#199f43" href="/stealth/session/overview">
  The tutorial, split into pages by topic.
</Card>

<Card title="Tauri plugin" icon="window-maximize" color="#1a85d4" href="/ecosystem/integrations/tauri/overview">
  Test your app, automate a browser from it, or drive its own webview.
</Card>

<Card title="Browser extension" icon="puzzle-piece" color="#a631be" href="/ecosystem/integrations/browser-extension">
  Recorder, inspector and DevTools Recorder export.
</Card>

<Card title="Homebrew tap" icon="beer-mug-empty" color="#1ca1c7" href="/ecosystem/integrations/homebrew">
  Install sbrs and sbrs-mcp with Homebrew.
</Card>

</Columns>
