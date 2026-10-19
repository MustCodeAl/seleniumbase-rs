---
title: "Security"
---

Rust's memory-safety guarantees, explicit error handling, and type system make
the implementation of `seleniumbase-rs` more robust than many dynamic-language
alternatives.

<Warning>

Browser automation has its own security model. A memory-safe
crate can still leak credentials, execute untrusted JavaScript, or expose the host
machine to a compromised page.

</Warning>

This page explains the security properties that Rust provides, the risks that
remain, and the operational practices that reduce them.

<Visibility for="agents">

Summary for AI exports: Rust gives memory safety in safe code, explicit errors, locked dependencies, feature gates and no `unsafe` in this crate. It does not protect against credential exposure, malicious pages, generated code, MCP server access or supply-chain attacks. Keep credentials in environment variables or a secrets manager, connect only trusted MCP clients, review generated Rust before running it, run untrusted pages in disposable containers, and treat stealth modes as not a security control.

</Visibility>

## What Rust gives you

- **Memory safety in safe code**: buffer overflows, use-after-free, and data races
  are ruled out by the compiler for code that does not use `unsafe`.
- **Explicit error handling**: `Result` types make failure paths visible instead
  of silently propagating `None` or exceptions.
- **Locked dependencies**: `Cargo.lock` records exact dependency versions, making
  builds reproducible and easier to audit.
- **Feature gates**: nothing is enabled by default. Optional pieces such as cloud
  uploads (`s3`, `azure`, `gcp`), Playwright, the MCP server, the terminal UI
  (`tui`), native desktop automation (`gui`) and the embedded database (`turso`)
  are compiled in only when explicitly enabled, reducing the default attack
  surface and dependency tree.
- **No `unsafe` in this crate**: the crate's own source has no `unsafe` blocks.
  Its dependencies may.

## What Rust does not protect against

Browser automation is inherently risky because it drives a full browser that can
fetch remote resources, execute JavaScript, and read or write files. Even with a
Rust harness, the following risks remain:

- **Credential exposure**: passwords, API keys, and session tokens stored in test
  code or logs can leak.
- **Malicious pages**: a test that visits an untrusted URL may download malware,
  exploit a browser zero-day, or phish the operator.
- **Generated code**: the Python importer, recorder, and Selenium IDE parser emit
  Rust code. If the input is attacker-controlled, the output must be reviewed
  before compilation.
- **MCP server access**: the `sbrs-mcp` binary can control the browser and
  execute JavaScript in any page. An untrusted MCP client can therefore perform
  any action the browser can.
- **Supply-chain attacks**: a compromised dependency can still run code at build
  or runtime, even in a Rust project.

## Protections built into the crate

These reduce some risks; none replaces the guidance below.

- **Encrypting secrets**: `sbrs encrypt` and `sbrs decrypt` (and
  `common::encryption`) use AES-256-GCM with a passphrase read from the
  `SB_ENCRYPTION_KEY` environment variable, never from an argument, so it stays
  out of shell history and process listings.
- **Passwords out of logs**: the `Debug` output of `StealthOptions` hides the
  password in a proxy URL and the values of extra headers, and `ProxyEntry`
  never shows a password in `Debug` or `Display`.
- **Stored profiles**: with the `turso` feature, `ProfileVault` keeps browser
  profiles, which hold cookies and proxy passwords, encrypted at rest.
- **Untrusted fingerprint payloads**: switches that run a program, load code,
  open remote debugging, or redirect traffic (for example
  `--renderer-cmd-prefix`, `--load-extension`, `--remote-debugging-port`,
  `--proxy-server`) are refused when they come from a fingerprint payload.
- **Confined output**: artifacts such as screenshots and recordings are written
  into `latest_logs`, and a file name with a directory part or `..` is refused.
  MCP file-writing tools are confined to `SB_MCP_OUTPUT_DIR` (default
  `./mcp_output`), and absolute paths and `..` are refused there too.
- **Closing browsers**: `sbrs` and `sbrs-mcp` close their browser on
  SIGINT and SIGTERM, within `SB_SHUTDOWN_TIMEOUT_SECS`, so an orchestrated stop
  does not leave a logged-in browser running.

## Secure operating guidance

### Credentials and secrets

- Keep credentials in environment variables, a secrets manager, or your CI
  provider's secret store. Never commit them to source control.
- Use dedicated test accounts with the least privilege necessary.
- Rotate test credentials regularly and audit who has access to them.

### Untrusted input

- Treat page content, downloaded files, generated selectors, imported Python
  code, and recorded actions as untrusted input.
- Review generated Rust files from `sbrs import-python` or the recorder before
  compiling or running them.
- Do not run generated tests against production-like data or credentials until
  you have inspected them.

### MCP server

<Danger>

Connect only trusted MCP clients. The server runs with the privileges of the
user that launched it and can execute JavaScript in the active page.

</Danger>

- Build the `sbrs-mcp` binary only when needed and distribute it only to
  trusted hosts.
- Consider running the MCP server in a sandbox or container with limited network
  access.

### Dependency auditing

- Run `cargo audit` or `cargo deny check` in CI to detect known vulnerabilities
  in dependencies. This repository's CI runs `cargo deny check` with the
  `deny.toml` at the repository root, which also bans wildcard version
  requirements and unknown registries and git sources, checks licenses, and
  lists each ignored advisory with the reason.
- Review `Cargo.lock` changes in pull requests, especially for optional features
  that pull in large dependency trees.
- The crate has no git dependencies. If you add one, pin it to a specific tag or
  revision (and allow it in `deny.toml`).

### Isolation

- Run tests that visit untrusted pages in disposable containers, virtual
  machines, or CI workers that are reset after each run.
- Do not run the browser as root or with unnecessary privileges.
- Use headless mode in CI, but remember that headless mode is not a security
  boundary by itself.

### Stealth mode limitations

<Warning>

Stealth modes change browser fingerprints to reduce detection. They do **not**
provide isolation, authorization, malware protection, or encryption. A browser
in UC mode can still be exploited by a malicious page.

</Warning>

## Threat model summary

| Threat | Mitigation |
|--------|------------|
| Credential leak in source code | Environment variables / secret manager |
| Malicious page exploits browser | Isolated containers, least privilege, network controls |
| Compromised dependency | `cargo audit`, lockfile review, minimal features |
| Untrusted MCP client | Allow-list clients, sandbox the server |
| Generated code contains backdoor | Manual review before compile/run |
| Memory-safety bug in harness | Rust safe code; audit any `unsafe` blocks |

## Security checklist for new projects

- [ ] Credentials are loaded from environment variables or a secrets manager.
- [ ] Test accounts have minimal permissions.
- [ ] `cargo audit` runs in CI.
- [ ] Generated Rust is reviewed before execution.
- [ ] MCP server is restricted to trusted clients.
- [ ] Untrusted-page tests run in isolated environments.
- [ ] The team understands that stealth modes are not security controls.

## Related reading

<Columns cols={2}>

<Card color="#1a85d4" title="MCP servers" icon="robot" href="/tutorials/mcp_server">

What each tool can do, and the security notes for the server.

</Card>

<Card color="#199f43" title="Binary Patching" icon="screwdriver-wrench" href="/tutorials/binary_patching">

Safety and licensing before you patch a driver or a browser.

</Card>

<Card color="#d47628" title="Operating systems" icon="desktop" href="/building#operating-systems">

Running as root, `--no-sandbox` and containers.

</Card>

<Card color="#a631be" title="Why Rust?" icon="gear" href="/why-rust">

When the crate is the right choice.

</Card>

</Columns>

