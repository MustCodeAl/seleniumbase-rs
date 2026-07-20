# Security

Rust prevents broad classes of memory-safety defects in safe code and makes
shared-state boundaries explicit. Cargo also supports locked dependency graphs,
feature-gated integrations, and tools such as `cargo audit`.

These properties improve the implementation foundation, but browser automation
is not automatically secure. A test can still expose credentials, execute
untrusted JavaScript, visit a hostile page, or grant an MCP client excessive
control.

## Secure operating guidance

- Keep credentials in environment variables or a secret manager.
- Use dedicated test accounts with least privilege.
- Treat page content, downloaded files, generated selectors, and imported code
  as untrusted input.
- Review generated Rust before compiling or running it.
- Connect only trusted clients to `seleniumbase-mcp`; it can control a browser
  and execute JavaScript.
- Run `cargo audit` in CI and review lockfile changes.
- Isolate tests that visit untrusted pages in disposable containers or workers.

Stealth modes change browser fingerprints. They do not provide isolation,
authorization, or malware protection.

