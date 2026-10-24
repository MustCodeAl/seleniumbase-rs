---
title: "CLI TL;DR"
sidebarTitle: "TL;DR"
---

The commands you reach for most, by task. Every flag here is in `sbrs <command> --help`;
the [CLI reference](/help/cli_reference) lists them all and [CLI usage](/tutorials/cli_usage)
explains them.

<Note>

Global options (`--headless`, `--browser`, `--uc`, `--proxy`, `--plugin`, ...) go **before** the
command: `sbrs --headless smoke https://example.com`. Each command starts its own browser and closes
it when it is done.

</Note>

## Set up

```bash
sbrs install                      # the chromedriver that matches the installed Chrome
sbrs install geckodriver          # or edgedriver; a version too: sbrs install chromedriver 155
sbrs doctor                       # what was found, what is missing, and how to fix it
sbrs version
```

## Open and check a page

```bash
sbrs open https://example.com                                  # prints the title
sbrs smoke https://example.com --title-contains Example        # fails unless the title has it
sbrs --headless --browser firefox smoke https://example.com
sbrs --uc smoke https://example.com                            # undetected Chrome
sbrs deferred --url https://example.com --assert title=Example --assert "text=h1:Example Domain"
```

## Save what a page shows

```bash
sbrs print-pdf https://example.com -o page.pdf
sbrs pdf-text https://example.com/report.pdf                   # text of a page or a PDF
sbrs download-file https://example.com/file.zip -o file.zip
```

## Write and run tests

```bash
sbrs mkdir tests/ui                                # a runnable test suite
sbrs mkfile tests/login.rs --url https://example.com
sbrs test                                          # cargo test, with the sbrs options passed on
sbrs test login -- --nocapture                     # tests whose name has "login"
sbrs test --example NAME                           # examples/NAME.rs instead of the tests
```

## Record, replay, translate

```bash
sbrs mkrec tests/login.rs --url example.com        # type c and Enter at the prompt to finish
sbrs replay recordings/login_rec.json
sbrs run-scenario --file login.json --dashboard dashboard.html
sbrs translate login.json es --copy                # writes login_es.json
```

## Bring tests from other tools

```bash
sbrs import-python test_login.py -o tests/login.rs           # SeleniumBase or Selenium Python
sbrs import-playwright login.spec.ts -o tests/login.rs       # JavaScript, TypeScript or Python
sbrs import-puppeteer recording.json -o tests/flow.rs        # Puppeteer or a Chrome Recorder export
```

## Stealth and DevTools

```bash
sbrs patch-chromedriver --path downloaded_drivers/chromedriver
sbrs patch-chrome --path "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
sbrs --cdp cdp --cmd Browser.getVersion                       # any DevTools Protocol command
sbrs --cdp cdp --cmd Network.setCacheDisabled --params '{"cacheDisabled":true}'
```

## AI assistants (MCP)

```bash
sbrs devtools                     # Chrome on 127.0.0.1:9222, prints configs for DevTools MCP servers
sbrs devtools --headless --port 0
sbrs-mcp --server cdp             # the MCP server on stdio: cdp, driver, sb (the default) or dev
sbrs-mcp install claude-code      # register the browser and dev servers with a client
sbrs-mcp clients                  # the clients it knows and what is registered
```

See [MCP server](/tutorials/mcp_server) for the client configurations.

## Plugins

```bash
sbrs plugin list                                   # the built-in plugins and whether they are on
sbrs plugin info shield
sbrs --plugin shield open https://example.com
sbrs --plugin shield --plugin-opt 'shield.level="strict"' open https://example.com
```

## Shell

```bash
sbrs completions zsh > ~/.zfunc/_sbrs              # bash, zsh, fish, elvish, powershell
sbrs prompt                                        # Chrome's major version and the driver, for a prompt
sbrs encrypt "secret text"                         # prints a token
sbrs decrypt TOKEN
```

<Tip>

The same examples, in the [tldr-pages](https://github.com/tldr-pages/tldr) format, are in
`contrib/tldr/` of the repository.

</Tip>
