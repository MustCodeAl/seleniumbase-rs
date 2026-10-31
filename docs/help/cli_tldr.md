---
title: "CLI TL;DR"
sidebarTitle: "TL;DR"
---

The commands you reach for most, by task. Every flag here is in `sbrs <command> --help`;
the [CLI reference](/help/cli_reference) lists them all and [CLI usage](/tutorials/cli_usage)
explains them.

<Note>

Global options (`--headless`, `--browser`, `--uc`, `--proxy`, `--plugin`, `--url`, ...) go before or
after the command, once each: `sbrs --headless smoke https://example.com` and
`sbrs smoke https://example.com --headless` are the same. Each command starts its own browser and
closes it when it is done, so a command that acts on a page needs it: `--url URL` (or
`SB_START_PAGE`), as in `sbrs --url https://example.com get-text --css h1`. `open`, `smoke`,
`download-file`, `print-pdf`, `pdf-text` and `assert-pdf-text` take their own URL and refuse `--url`.

</Note>

## Set up

```bash
sbrs install chromedriver         # the chromedriver that matches the installed Chrome
sbrs install chromedriver 155     # a version too: 155, latest, previous, keep, 141.0.7390.76
sbrs get geckodriver              # `get` is `install`; also edgedriver, uc_driver, cft, chs, chromium
sbrs install -p chromedriver      # also copy it to SB_DRIVER_PATH_DIR, else /usr/local/bin
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

Every other page command takes its page from `--url`:

```bash
sbrs --url https://example.com get-title
sbrs --url https://example.com get-text --css h1               # prints the element's text
sbrs --url https://example.com screenshot --path page.png
sbrs --headless --url https://example.com assert-element --css h1
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
sbrs translate login.json --es --copy              # writes login_es.json
sbrs translate login.json --zh -p -n               # prints the Chinese scenario with line numbers
```

## Generate and show files

```bash
sbrs mkpres tests/deck.rs --es                     # a test that builds a three-slide deck
sbrs mkchart tests/chart.rs --ja                   # a test that shows a chart on a slide
sbrs print tests/login.rs -n                       # a file, with line numbers
sbrs convert login.py                              # a Selenium IDE export into login_SB.rs
sbrs objectify tests/login.rs                      # selectors into page_objects.rs, used in the test
sbrs extract-objects tests/login.rs                # only write page_objects.rs
sbrs inject-objects tests/login.rs -c              # only use the page objects, a comment per line
sbrs revert-objects tests/login.rs -c              # back to the selectors, byte for byte
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

## Proxy, Selenium Server and Grid

```bash
sbrs proxy                                         # a forward proxy on 127.0.0.1:8899, no authentication
sbrs --proxy 127.0.0.1:8899 open https://example.com
sbrs download server                               # the Selenium Server jar, into downloaded_drivers/
sbrs grid-hub start                                # also stop, restart and status
sbrs grid-node start --hub 127.0.0.1
```

## Look things up

```bash
sbrs install --help                                # the flags of one command, ending with Examples:
sbrs methods                                       # the common test methods, as called on sb
sbrs options                                       # each browser option: flag, SB_* variable, settings key
sbrs behave-options                                # how Python's behave -D options map to sbrs test
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
sbrs plugin run crawler --input crawl.json --out pages.jsonl   # the task of a plugin: fetcher, extractor, batcher, crawler or imap
```

The built-in plugins are shield, block-resources, crx, fetcher, extractor, batcher, crawler,
browser, imap and tor; [Choosing plugins](/plugins/selecting) has every way to turn them on.

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
