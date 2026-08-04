# Security

Rust's memory-safety guarantees, explicit error handling, and type system make
the implementation of `seleniumbase-rs` more robust than many dynamic-language
alternatives. However, browser automation has its own security model. A memory-safe
crate can still leak credentials, execute untrusted JavaScript, or expose the host
machine to a compromised page.

This page explains the security properties that Rust provides, the risks that
remain, and the operational practices that reduce them.

## What Rust gives you

- **Memory safety in safe code**: buffer overflows, use-after-free, and data races
  are ruled out by the compiler for code that does not use `unsafe`.
- **Explicit error handling**: `Result` types make failure paths visible instead
  of silently propagating `None` or exceptions.
- **Locked dependencies**: `Cargo.lock` records exact dependency versions, making
  builds reproducible and easier to audit.
- **Feature gates**: optional integrations such as cloud uploads, Playwright, and
  the MCP server are compiled in only when explicitly enabled, reducing the
  default attack surface.

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
- **MCP server access**: the `seleniumbase-mcp` binary can control the browser and
  execute JavaScript in any page. An untrusted MCP client can therefore perform
  any action the browser can.
- **Supply-chain attacks**: a compromised dependency can still run code at build
  or runtime, even in a Rust project.

## Secure operating guidance

### Credentials and secrets

- Keep credentials in environment variables, a secrets manager, or your CI
  provider's secret store. Never commit them to source control.
- Use dedicated test accounts with the least privilege necessary.
- Rotate test credentials regularly and audit who has access to them.

### Untrusted input

- Treat page content, downloaded files, generated selectors, imported Python
  code, and recorded actions as untrusted input.
- Review generated Rust files from `sbase import-python` or the recorder before
  compiling or running them.
- Do not run generated tests against production-like data or credentials until
  you have inspected them.

### MCP server

- Build the `seleniumbase-mcp` binary only when needed and distribute it only to
  trusted hosts.
- Connect only trusted MCP clients. The server runs with the privileges of the
  user that launched it and can execute JavaScript in the active page.
- Consider running the MCP server in a sandbox or container with limited network
  access.

### Dependency auditing

- Run `cargo audit` in CI to detect known vulnerabilities in dependencies.
- Review `Cargo.lock` changes in pull requests, especially for optional features
  that pull in large dependency trees.
- Pin git dependencies to a specific tag or revision.

### Isolation

- Run tests that visit untrusted pages in disposable containers, virtual
  machines, or CI workers that are reset after each run.
- Do not run the browser as root or with unnecessary privileges.
- Use headless mode in CI, but remember that headless mode is not a security
  boundary by itself.

### Stealth mode limitations

Stealth modes change browser fingerprints to reduce detection. They do **not**
provide isolation, authorization, malware protection, or encryption. A browser
in UC mode can still be exploited by a malicious page.

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

