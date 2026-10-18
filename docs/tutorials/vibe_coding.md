---
title: "Vibe coding with seleniumbase-rs"
sidebarTitle: "Vibe coding"
---

"Vibe coding" here means: you describe a browser test in plain words, an AI agent
drives a real browser to work out how, and it hands you a Rust test to review and
keep. The MCP servers in this crate are built for that loop. They let the agent
look at a page, try a selector, see why a step failed, and turn what worked into
code, instead of guessing at selectors blind.

<Info>

This page is a workflow, not a promise. The agent writes a first draft; you
decide whether it is a good test.

</Info>

## Set it up once

Build the server and point your MCP client (Claude Code, Claude Desktop, or any
client that speaks MCP over stdio) at it, as in the
[MCP Servers](/tutorials/mcp_server) page:

<Steps>

<Step title="Build the server">

```bash
cargo build --release --bin sbrs-mcp --features mcp-server
```

</Step>

<Step title="Register it with your client">

```json
{
  "mcpServers": {
    "seleniumbase": {
      "command": "/absolute/path/to/target/release/sbrs-mcp",
      "args": ["--server", "cdp"],
      "env": { "SB_MCP_OUTPUT_DIR": "/tmp/seleniumbase-output" }
    }
  }
}
```

</Step>

</Steps>

Use `--server cdp` to start: it drives Chrome directly, so there is no
chromedriver to match. Use `--server sb` when the test needs the full
`BaseCase` toolset. The agent calls `start_browser` first and `close_browser`
last. Anything the tools write (screenshots, bundles) stays inside
`SB_MCP_OUTPUT_DIR`.

## The loop

<Steps>

<Step title="Say what the test should prove">

One behaviour, one page. "Log in as the demo user and check the welcome heading
says Welcome."

</Step>

<Step title="The agent explores">

It opens the page and uses `inspect_page` to see what
is there, `suggest_selectors` to find selectors that work for an element, and
`diagnose_selector` when one does not match (it says why and what to try).

</Step>

<Step title="The agent acts and checks">

It clicks, types and asserts through the usual
tools. Each call that changes the page or checks something is remembered.

</Step>

<Step title="It exports the test">

`export_session_as_test` turns the successful steps
of the session into a `BaseCase` test. Failed calls and read-only lookups are
left out, and a step the generator cannot express becomes a `// skipped:`
comment rather than silently disappearing.

</Step>

<Step title="You run it">

Put the test in `tests/` and run `cargo test`. Wrapping it in
`run_browser_test` (see [Reliability](/reliability)) closes the browser
even when an assertion fails.

</Step>

<Step title="When it fails, read the failure">

A failed MCP call comes back with its
kind, the call, the page's state (URL, title, ready state, script errors,
selector matches), a hint and the next tools to try. For a bigger problem ask
for `get_console_messages`, `get_failed_requests`, `audit_page` (an
accessibility check) or `capture_debug_bundle` (a redacted bundle you can
attach to a bug report). If the setup itself looks wrong, `diagnose_environment`
reports the Chrome and chromedriver versions, the redacted `SB_*` settings and
the output directory without starting a browser.

</Step>

<Step title="Repeat">

Repeat until the test fails for the right reason, then passes.

</Step>

</Steps>

## Prompts that work

- "Open `/simple/login`, log in as demo_user, and export a test that checks the
  heading. Use selectors that do not depend on page layout."
- "This selector stopped matching. Diagnose it and suggest one that survives a
  re-render."
- "Run the exported test, and if it fails explain why in terms of the page, not
  the stack trace."
- "Check this page with the accessibility audit and list what a test should
  assert."

## Review before you trust it

<Warning>

- **Read the exported test.** Check that every assertion can fail. A test that
  only clicks proves nothing.
- **Prefer stable selectors.** Ask for `data-testid`, `name` or an `id`; a
  selector tied to layout breaks on the next redesign.
- **Check for `// skipped:` lines.** They mark steps the generator could not
  turn into code.
- **Keep secrets out of the chat.** The debug bundle redacts what it can, but
  look before you share one, and do not type real passwords into prompts.
- **Point it at a test or staging site**, not production, and be careful with
  actions that cannot be undone.
- **Do not let the agent loosen a check to make a test pass.** If it suggests
  deleting an assertion or adding a long `sleep`, that is a flaky test in the
  making.

</Warning>

## Working on the crate itself with an agent

The repository is set up for coding assistants:

- `CLAUDE.md` is a map of the modules, the build and check commands and the
  things that have bitten before. Have the agent read it first.
- The gate is `cargo fmt --all -- --check`,
  `cargo clippy --all-targets --features "mcp-server test-util" -- -D warnings`
  and `cargo test --features "mcp-server test-util"`. Ask the agent to run it
  and to report the exit codes, not to say "it should pass".
- `rust-analyzer.toml` loads the optional features so analysis covers
  `src/mcp` and the test helpers.
- `parity/api.toml` records the status of every upstream API; a new API gets an
  entry there, and the generated `docs/parity.md` must be refreshed.

## What to expect honestly

<Note>

Most of the tool behaviour is covered by unit tests and a mock browser, and a
small set of real-Chrome tests; only part of it has been run against real sites.
Agent-written selectors can be brittle, and an exported test is a draft that
shows what worked once. Treat the loop as a fast way to a reviewed test, not as a
substitute for the review.

</Note>
