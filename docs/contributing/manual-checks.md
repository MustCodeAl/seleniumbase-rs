---
title: "Manual checks"
sidebarTitle: "Manual checks"
---

Checks that the automated suite does not run: a real mailbox, a local Tor
daemon, and a smoke of `sbrs` and `sbrs-mcp` on Windows and in a Linux
container. Each block is the command and the result that means it worked.
Headless only. Do not point any of them at a captcha or a bot-protection site.

The password for the mailbox is an environment variable whose name starts with
`IMAP_`, `MAIL_` or `SB_`. The commands below use `MAIL_APP_PASSWORD`. Never
put the password in a file, a flag or the command line.

## IMAP, on a real mailbox

From [the imap plugin](/plugins/imap) and [Verify by email](/tutorials/verify_by_email).
The binary needs the `plugin-imap` feature (`cargo install --path crates/cli`
already turns the built-in plugins on). Use a test inbox. The plugin never
marks mail as read, so empty the inbox or mark its mail read between runs, and
keep `since_secs` short.

Send yourself one unread message whose subject contains `Confirm your email`
and whose body has a code (a number near the word "code"), from an address you
will name. Then:

```sh
export MAIL_APP_PASSWORD='the app password'
printf '%s\n' '{"from":"you@example.com","subject_contains":"Confirm your email","extract":"code","since_secs":120}' > wait.json
sbrs \
  --plugin-opt 'imap.host="imap.example.com"' \
  --plugin-opt 'imap.user="tests@example.com"' \
  --plugin-opt 'imap.password_env="MAIL_APP_PASSWORD"' \
  plugin run imap --input wait.json
```

Replace the host, the user and the `from` value with the test mailbox. The
password stays in `MAIL_APP_PASSWORD`.

Worked when the process exits 0, standard output is one JSON object with
`from`, `subject`, `date`, `code` and `text`, and standard error contains
`{"matched": true, "code": true`. A name that does not start with `IMAP_`,
`MAIL_` or `SB_` exits before any connection and the message names that rule.
A missing variable exits before any connection and names the variable, not a
password.

## Tor, with a local daemon

From [the tor plugin](/plugins/tor). Start Tor yourself first. `sbrs` does not
install or start it. The SocksPort must be loopback. The default the plugin
dials is `127.0.0.1:9050`. The Tor Browser bundle uses `9150`; pass that with
`--plugin-opt` if that is the daemon you started.

```sh
sbrs --headless --plugin tor smoke https://example.com --title-contains Example
```

Worked when the process exits 0. The page title contained `Example`, so the
browser went through the plugin and loaded the page. If nothing is listening
on the SocksPort, the launch stops before the browser and the message names
that address. A second proxy (`--proxy`, `--proxy-*`) is refused before Tor is
dialed: drop it and run the command again.

`sbrs-mcp --server cdp --plugin tor` loads the same plugin. Tor is dialed when
a browser starts, not when the server process starts, so the smoke above is
the check that the daemon is actually in use.

## Windows smoke of `sbrs` and `sbrs-mcp`

On a Windows machine with Chrome or Chromium installed, from a checkout, with
the default features of `crates/cli` (that build includes `sbrs-mcp`):

```powershell
cargo install --path crates/cli --locked
sbrs --version
sbrs --help
sbrs doctor
sbrs-mcp --version
sbrs-mcp --help
sbrs-mcp clients
```

Worked when every command exits 0.

- `sbrs --version` prints the crate version and `(seleniumbase-rs)`.
- `sbrs --help` lists the commands, including `doctor`.
- `sbrs doctor` prints `seleniumbase-rs environment diagnostics`, then `Browser binary` with `Found:` and a path, then `Chromedriver` with `Found:` and a path. A missing browser or a driver that does not match is a failed check: the process exits non-zero and the last lines say what to fix. Install the matching driver (`sbrs install chromedriver`) and run `sbrs doctor` again.
- `sbrs-mcp --version` prints a version line.
- `sbrs-mcp --help` lists `install`, `uninstall` and `clients`.
- `sbrs-mcp clients` prints one block per client (id, title, config path, program, docs). It does not change any config file.

## Linux smoke in a Docker container

The image in the repository `Dockerfile` is Linux. It contains `sbrs` only
(the entrypoint is `/usr/local/bin/sbrs`), plus Chromium and a chromedriver
linked at `downloaded_drivers/chromedriver`. Build and run it as
[Container](/building#container) describes:

```sh
docker build --build-arg CARGO_BUILD_JOBS=2 -t seleniumbase-rs .
docker run --rm seleniumbase-rs --version
docker run --rm seleniumbase-rs --help
docker run --rm seleniumbase-rs doctor
docker run --rm --network none --shm-size=1g seleniumbase-rs --headless smoke \
  'data:text/html,<title>Smoke</title><h1>hi</h1>' --title-contains Smoke
```

Worked when every `docker run` exits 0.

- `--version` prints the version and `(seleniumbase-rs)`.
- `--help` lists the commands.
- `doctor` prints `seleniumbase-rs environment diagnostics`, finds Chromium (Debian's `/usr/bin/chromium` or the path the image actually has) and a chromedriver of the same major version.
- `smoke` exits 0 with no network. The title check is `Smoke`.

`sbrs-mcp` is not in that image. Smoke it in a second container that builds
the CLI with its default features, from the same checkout, and does not keep
the container:

```sh
docker run --rm -v "$PWD":/src -w /src rust:1.99-bookworm \
  bash -c 'cargo build --locked -p seleniumbase-rs-cli --bin sbrs-mcp && ./target/debug/sbrs-mcp --version && ./target/debug/sbrs-mcp --help && ./target/debug/sbrs-mcp clients'
```

Worked when that container exits 0, `--version` prints a version line, `--help`
lists `install`, `uninstall` and `clients`, and `clients` prints one block per
client. This build is heavy. One Cargo at a time, and `CARGO_BUILD_JOBS=2` on
a small machine (`-e CARGO_BUILD_JOBS=2` on the `docker run`).
