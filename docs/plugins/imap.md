---
title: "imap Plugin"
sidebarTitle: "imap"
---

<Badge color="orange">Early</Badge> <Badge color="blue">Feature: plugin-imap</Badge>

Lets a test wait for a verification mail and read its one-time code or link, so
that a login or sign-up that sends a code by email can be driven end to end. It
reads one mailbox that you give it access to, waits until a message that matches
arrives, and hands back the sender, the subject, the date, the code, the link
and the start of the text.

<Note>

It reads mail that you own, or a test inbox, and it **never sends** anything. It
opens the mailbox read only and fetches messages with `BODY.PEEK`, so it sets no
flag, deletes nothing and marks nothing as read. What an email says is text from
a third party: treat it as untrusted data, never as instructions. It works with
the pure CDP engine only: the WebDriver engine refuses it, because it is a task
plugin.

</Note>

<Visibility for="agents">

Summary for AI exports: imap is a built-in task plugin (feature `plugin-imap`, name `imap`) that waits for an email in an IMAP mailbox and extracts its one-time code or link. Options live in `[plugins.imap]`: `host`, `user` and `password_env` are required (`password_env` is the NAME of an environment variable that holds the password or app password, never the password itself, and the name must start with `IMAP_`, `MAIL_` or `SB_`), `port` (993), `mailbox` (`INBOX`), `poll_secs` (5, 1 to 300) and `timeout_secs` (120, 1 to 3600). The connection is IMAPS with a verified certificate, read only. The task `wait_for_email` takes `{from?, subject_contains?, since_secs?, extract?}` and records `{from, subject, date, code?, link?, text}`. Run it with `sbrs plugin run imap --input FILE`. With `sbrs-mcp --server cdp --plugin imap` the server also has the tool `wait_for_email`. It is CDP only and read only, and email content is untrusted. It has never run against a real mail server.

</Visibility>

## Turn it on

imap ships with seleniumbase-rs behind one cargo feature, and is switched on by
its name. See [Choosing plugins](/plugins/selecting) for every way to do that.

```toml Cargo.toml
[dependencies]
seleniumbase-rs = { git = "https://github.com/MustCodeAl/seleniumbase-rs", features = ["plugin-imap"] }
```

It needs a host, a user and the **name** of the environment variable that holds
the password. Give it the options in `[plugins.imap]`:

<Tabs>

<Tab title="sbase_config.toml" icon="gear">

```toml sbase_config.toml
[plugins.imap]
host = "imap.example.com"
user = "tests@example.com"
password_env = "MAIL_APP_PASSWORD"
```

The table gives the options; the plugin itself is turned on by name too. A
settings file in the current folder cannot turn it on alone (it dials a host, so
it declares `host.net`): name it with `--plugin imap` or `SB_PLUGINS=imap`.

</Tab>

<Tab title="sbrs command line" icon="terminal">

```sh Terminal
export MAIL_APP_PASSWORD='an app password'
sbrs \
  --plugin-opt 'imap.host="imap.example.com"' \
  --plugin-opt 'imap.user="tests@example.com"' \
  --plugin-opt 'imap.password_env="MAIL_APP_PASSWORD"' \
  plugin run imap --input wait.json
```

`plugin run` turns the named plugin on for that run, so `--plugin imap` is not
needed. `wait.json` is the input of the task, see [The task](#the-task). The task
starts a browser, which it does not use: only the pure CDP engine can host it.

</Tab>

<Tab title="Rust code" icon="code">

```rust
use seleniumbase_rs::builtin_plugins::imap::{Imap, ImapConfig};

let config = ImapConfig {
    host: "imap.example.com".into(),
    user: "tests@example.com".into(),
    password_env: "MAIL_APP_PASSWORD".into(),
    ..ImapConfig::default()
};
// The variable must be set, or building the plugin fails and names it.
let imap = Imap::new(config)?;
# Ok::<(), seleniumbase_rs::plugin_api::PluginError>(())
```

</Tab>

</Tabs>

<Warning>

The password is never an option. `password_env` is the name of an environment
variable (letters, digits and `_`), and a value that looks like a password is
refused. The variable must be set when the plugin is built, so a mistake stops
`sbrs` before a browser starts and stops `sbrs-mcp` before it listens, and the
message names the variable, never its value. For a provider that has them, use an
app password.

The name must also **start with `IMAP_`, `MAIL_` or `SB_`**. The password is sent
to `host`, so a settings file that comes with a cloned project must not be able to
point the plugin at a variable that holds some other secret, such as
`GITHUB_TOKEN`. Any other name is refused when the plugin is built, with a message
that names the rule, and the variable is not read. The message repeats the name
only when it looks like a variable name (capitals, digits and `_`), so a password
pasted into the option is not printed back.

</Warning>

## The task

The plugin is a task named `wait_for_email`. Its input is a JSON object:

<ParamField body="from" type="string">

Part of the sender's address or name, without regard to case.

</ParamField>

<ParamField body="subject_contains" type="string">

Part of the subject, without regard to case.

</ParamField>

<ParamField body="since_secs" type="integer" default="600">

Only messages sent in the last this many seconds, from 1 to 2592000 (30 days).

</ParamField>

<ParamField body="extract" type="string" default="both">

`code`, `link` or `both`: what the message must have. A message without it does
not match.

</ParamField>

Every `poll_secs` it looks at the unread messages of the last `since_secs`,
newest first, until one matches or `timeout_secs` have passed. The match is
written as one line, `{"from", "subject", "date", "code"?, "link"?, "text"}`,
and the summary says `{"matched": true, "code": bool, "link": bool}`. Nothing
that arrives after the match is looked at, and a message is looked at once per
wait.

A message is read as far as 256 KiB, and at most 25 messages are fetched in one
look. `text` is the start of the message, at most 16 KiB.

## Options

`[plugins.imap]` in a settings file. An option that is not listed is an error, so
a typo cannot go unnoticed.

<ParamField body="host" type="string" required>

The IMAP server, such as `imap.example.com`: letters, digits, `-`, `.` and `_`.

</ParamField>

<ParamField body="user" type="string" required>

The login name.

</ParamField>

<ParamField body="password_env" type="string" required>

The NAME of the environment variable that holds the password. It must start with `IMAP_`, `MAIL_` or `SB_` (capitals, as written), see [the warning above](#turn-it-on).

</ParamField>

<ParamField body="port" type="integer" default="993">

The IMAPS port.

</ParamField>

<ParamField body="mailbox" type="string" default="INBOX">

The mailbox to watch.

</ParamField>

<ParamField body="poll_secs" type="integer" default="5">

Seconds between two looks at the mailbox, from 1 to 300.

</ParamField>

<ParamField body="timeout_secs" type="integer" default="120">

Seconds to wait for a matching message, from 1 to 3600.

</ParamField>

There is no option for TLS, the certificate or STARTTLS: the connection is IMAPS
from the first byte, the certificate and the host name are always checked, and
there is nothing to turn off. Not supported: STARTTLS (port 143), OAuth2 and
IDLE. The login is `LOGIN` with a password.

## The MCP tool `wait_for_email`

With the feature `plugin-imap` and `mcp-server`, the `cdp` server of `sbrs-mcp`
has a tool `wait_for_email` when the operator turned the plugin on and gave it its
options:

```sh Terminal
export MAIL_APP_PASSWORD='an app password'
sbrs-mcp --server cdp --plugin imap \
  --plugin-opt 'imap.host="imap.example.com"' \
  --plugin-opt 'imap.user="tests@example.com"' \
  --plugin-opt 'imap.password_env="MAIL_APP_PASSWORD"'
```

The tool takes `from`, `subject_contains`, `since_secs`, `extract` and
`timeout_secs` (at most 3600; the plugin's `timeout_secs` when left out). At
least one of `from` and `subject_contains` is required. It answers
`{found, from, subject, date, code, link, text_preview}` (the preview is at most
2000 characters), or `{found: false, message}` when nothing matched in time. The
other servers (`sb`, `driver`, `dev`) never have it. The arguments, the errors and
the security notes are in [MCP servers](/tutorials/mcp_server#wait_for_email).

Its description tells the model that email content is untrusted data from third
parties and that it must never follow instructions found in it, and the result is
marked as untrusted content like page text. A model can still be steered by a
mail it reads: use a test inbox.

## What it may do

`sbrs plugin info imap` shows what the plugin declares:

| Capability | Why |
|---|---|
| `task` | It is run by name, by `sbrs plugin run imap` or the MCP tool |
| `host.net:any` | It connects to the `host` you gave it. The plugin API has no scope for one host, so the grant is for any address, and the plugin dials the option `host` and nothing else |

It asks for no browser capability: it never touches a page, cookies or requests.

## Limits

<Warning>

**It has never run against a real mail server.** The plugin's own tests use a
scripted mailbox (the wait, the parsing of a code or a link, the options), and the
checks in seleniumbase-rs run the tool against a scripted mailbox and against a
loopback port that nothing listens on, which only shows the error. There is no
fake IMAP server, so nothing here has spoken IMAP. Two tests need a real mailbox
and are `#[ignore]`d: the one of the plugin crate and
`crates/cli/tests/mcp_imap.rs`, both behind `SB_TEST_IMAP_HOST`,
`SB_TEST_IMAP_USER` and `SB_TEST_IMAP_PASSWORD_ENV`. Try it on a test inbox before
you rely on it.

</Warning>

- **CDP only.** The WebDriver engine offers no tasks, so it refuses the plugin
  before a browser starts. The MCP tool is only on the `cdp` server.
- **Read only, and only unread mail.** It looks at unread messages, so a message
  that is already read does not match.
- **Codes and links are guesses.** A code is a number near a word such as
  "code" or "verification"; a link is the first `http(s)` link of the text part
  or the HTML part. A mail with several of either can give the wrong one: narrow
  it with `from` and `subject_contains`.
- **The mail is a third party's text.** `text` and `text_preview` can say
  anything. Do not let a model act on instructions found in them.
