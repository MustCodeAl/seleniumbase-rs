---
title: "Community plugins"
sidebarTitle: "Overview"
description: "Community and standalone plugins in the seleniumbase-rs ecosystem, how to scaffold and publish your own plugin with sbrs plugin new or scaffold_plugin, and how capability security applies."
---

<Badge color="blue">Feature: plugins</Badge>

Beyond the ten built-in plugins compiled into `sbrs` (`plugins-builtin`) and the `wasm` sandbox host, the `seleniumbase-rs` plugin ecosystem includes **standalone and community plugin crates** built on `seleniumbase-rs-plugin`.

Community plugins can be distributed as independent crates (discovered on GitHub/GitLab via the `seleniumbase-rs-plugin` topic or workspace sub-crates in [seleniumbase-rs-plugins](https://github.com/MustCodeAl/seleniumbase-rs-plugins)) and attached to any `PluginSet` with the same capability declarations, hook ordering, and mock testing harness as built-in plugins.

## Featured community plugins

| Crate / Plugin | Repository | What it provides |
|---|---|---|
| [`antibot-detector`](/plugins/community/antibot-detector) | `MustCodeAl/seleniumbase-rs-plugins` | Multi-signal detection for Cloudflare, DataDome, Akamai, Kasada, PerimeterX, Imperva, AWS WAF, FingerprintJS, reCAPTCHA, and hCaptcha |
| [`behavior-dataset`](/plugins/community/behavior-dataset) | `MustCodeAl/seleniumbase-rs-plugins` | Synthetic human kinematic & Fitts's-law interaction batch generator with JSON, JSONL, and CSV ML exports |
| [`langchain`](/plugins/community/langchain) | `MustCodeAl/seleniumbase-rs-plugins` | `SeleniumBaseLoader` (Markdown + head metadata for RAG) and `SeleniumBaseTool` (agent action schema & runner) |
| [`owasp`](/plugins/community/owasp) | `MustCodeAl/seleniumbase-rs-plugins` | Passive OWASP Top 10:2021 auditor (headers, CSP, CORS, cookies, JWTs, DOM/storage, client-side SCA, SARIF 2.1.0, optional PTK integration) |
| [`tauri-plugin-sbrs-automation`](/ecosystem/tauri) | `MustCodeAl/tauri-plugin-sbrs-automation` | Desktop app Tauri v2 integration for profile orchestration and browser automation |

## Scaffold a new community plugin

`sbrs` ships a project generator (`sbrs plugin new`, also exposed on the `dev` MCP server as `scaffold_plugin`) that creates a ready-to-build plugin crate with `Plugin`, `Configurable`, tests against `seleniumbase-rs-plugin-testing`, optional CI configs, and AI assistant rules:

```sh Terminal
sbrs plugin new my-custom-plugin \
  --template library \
  --backend cdp \
  --focus scraping \
  --dev-env mise \
  --ai-agent antigravity
```

Key flags on `sbrs plugin new`:

- `--template` (`library` or `cli`): Generate a standalone plugin library or pair it with a CLI harness.
- `--backend` (`cdp`, `webdriver`, or `both`): Configure capability requirements and examples for your target engine.
- `--focus` (`general`, `cli`, `scraping`): Tailor starter boilerplate (`network`, `Task`, or passive hooks).
- `--dev-env` (`cargo`, `mise`, `docker`, `gitlab-ci`, `github-actions`): Scaffold CI/container config alongside `Cargo.toml`.
- `--ai-agent` (`none`, `cursor`, `claude-code`, `gemini`, `antigravity`, `codex`): Emit tailored project instructions referencing `seleniumbase-rs-plugin` rules.

See [Writing your own](/plugins/writing-your-own) for the step-by-step plugin trait guide, capability review guidelines, and testing with `seleniumbase-rs-plugin-testing`.

## Capability security and version alignment

- **Least-privilege grants**: `PluginSet::strict()` refuses legacy plugins without `Metadata::new(...)`, and `PluginSet::with` denies Critical capabilities (`raw:*`, `host.process`, `cdp:Target`, `cdp:Browser`) unless explicitly granted with `with_trusted` or `with_grants`.
- **Aligned plugin API source**: When adding community crates alongside built-in plugins, check that both resolve the same `seleniumbase-rs-plugin` source in `Cargo.lock` so `Plugin` trait objects match.

<Columns cols={2}>

<Card title="antibot-detector" icon="shield-halved" color="#d32a61" href="/plugins/community/antibot-detector">
  Detect Cloudflare, DataDome, Akamai, Kasada, PerimeterX, Imperva, AWS WAF, and CAPTCHAs.
</Card>

<Card title="behavior-dataset" icon="wave-square" color="#693acf" href="/plugins/community/behavior-dataset">
  Generate labeled synthetic human vs baseline trajectories for ML training.
</Card>

<Card title="langchain" icon="robot" color="#199f43" href="/plugins/community/langchain">
  Load web pages into Markdown documents and expose browser actions to LLM agents.
</Card>

<Card title="owasp" icon="lock" color="#1a85d4" href="/plugins/community/owasp">
  Passive headers, CSP, JWT, cookie, DOM, and SCA security auditing with SARIF output.
</Card>

</Columns>
