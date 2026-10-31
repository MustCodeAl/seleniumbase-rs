# tldr-pages entries for sbrs

Draft [tldr-pages](https://github.com/tldr-pages/tldr) entries for the `sbrs` CLI and the
`sbrs-mcp` MCP server of seleniumbase-rs. They are not part of the crate; they are kept
here so they can be proposed upstream.

## Pages

All pages are in `pages/common/`.

| File | Command |
| --- | --- |
| `sbrs.md` | the 8 most useful examples |
| `sbrs-install.md` | `sbrs install` (alias `sbrs get`) |
| `sbrs-mkdir.md`, `sbrs-mkfile.md` | scaffold a test suite or a test file |
| `sbrs-grid-hub.md`, `sbrs-grid-node.md` | Selenium Grid hub and node |
| `sbrs-record.md`, `sbrs-replay.md` | record and replay browser interactions |
| `sbrs-devtools.md` | Chrome with an open DevTools endpoint |
| `sbrs-prompt.md` | Chrome version and driver state for a shell prompt |
| `sbrs-pdf-text.md` | text of a page or PDF |
| `sbrs-plugin.md` | `sbrs plugin list`, `info` and `run`, and the global `--plugin` and `--plugin-opt` |
| `sbrs-mcp.md` | the `sbrs-mcp` binary |

The examples come from `sbrs --help` and `sbrs <command> --help` of version 0.4.0 and from
`docs/tutorials/mcp_server.md`. When a command changes, regenerate the matching page from its
help rather than from memory.

## Check

The pages follow the
[style guide](https://github.com/tldr-pages/tldr/blob/main/contributing-guides/style-guide.md).
Lint them with tldr-lint (Node.js needed):

```sh
bunx tldr-lint contrib/tldr/pages
# or: npm install --global tldr-lint && tldr-lint contrib/tldr/pages
```

It prints nothing and exits with 0 when all pages pass.

## Submit

Read the upstream
[CONTRIBUTING.md](https://github.com/tldr-pages/tldr/blob/main/CONTRIBUTING.md) first. Points
that matter here:

- The project should have been maintained for at least a year, unless the maintainers consider
  it notable. Check this before opening a pull request.
- The guide strongly discourages pages created with generative AI. Review every example
  against the real command and rewrite what you cannot vouch for, or hold the pages back.
- Sign the Contributor License Agreement when the bot asks.

Steps:

1. Fork `tldr-pages/tldr` and clone the fork.
2. Create a branch from `main`, for example `sbrs-add-page`.
3. Add `sbrs.md` first: `cp contrib/tldr/pages/common/sbrs.md <tldr clone>/pages/common/`.
   Subcommand pages need a base page, so `sbrs.md` has to come first (or in the same pull request).
4. Run `tldr-lint` on the copied pages inside the clone, then commit.
5. Open a pull request against `main` and enable "Allow edits by maintainers". Title it
   `sbrs: add page`. For the other pages use `sbrs-install: add page`, or a brace list such as
   `sbrs-{install, mkdir, mkfile}: add page`. Keep `sbrs-mcp.md` in a pull request of its own.
6. Answer review comments with further commits; avoid force-pushing.
