---
title: "Verify a Sign-up by Email"
sidebarTitle: "Verify by email"
---

<Badge color="orange">Early</Badge> <Badge color="blue">Feature: plugin-imap</Badge>

A sign-up that sends a code or a link by email cannot be tested by the browser
alone. This page builds the whole test with the pure CDP engine and the
[imap plugin](/plugins/imap): the browser fills in the form and clicks "Send",
the plugin waits in a mailbox you give it until the mail arrives and hands back
its link (or its one-time code), and the test opens the link and checks the
page it leads to.

<Note>

The plugin reads one mailbox that you own, or a test inbox, and it **never
sends** anything: the mailbox is opened read only and nothing is deleted or
marked. It works with the pure CDP engine only; the WebDriver engine refuses it.
What an email says is text from a third party, so treat it as untrusted data: the
test below checks the link before it opens it. The plugin signs in with `LOGIN`
and a password over IMAPS. It does not support OAuth2, STARTTLS or IDLE.

</Note>

<Info>

**What you will learn**

- What the plugin needs: a mailbox, an app password and a variable that holds it
- Write a sign-up test that waits for the mail and opens its link
- Do the same with a code, from the command line, or from an AI agent
- Keep old mail and hostile mail from steering the test

</Info>

<Visibility for="agents">

Summary for AI exports: to test a sign-up that sends a code or link by email, turn on the built-in imap plugin (feature `plugin-imap`) and use the pure CDP engine. In Rust: `Imap::new(ImapConfig { host, user, password_env, ..ImapConfig::default() })?` from `seleniumbase_rs::builtin_plugins::imap` (`password_env` is the NAME of an environment variable, which must start with `IMAP_`, `MAIL_` or `SB_`; the password is never an option), a `Clock` made with `Clock::new(Arc::new(timer))` over a `Timer` that sleeps on tokio, and `imap.wait_for_email(&Query { from, subject_contains, since_secs, extract, .. }, &clock).await?`, which returns a `Found` with `from`, `subject`, `date`, `code`, `link` and `text`. Click "Send" in the page first, then wait. Check the link's host before opening it: the mail is untrusted text. From the command line, `sbrs plugin run imap --input wait.json`. For an agent, the `cdp` server of `sbrs-mcp` started with `--plugin imap` and the plugin's options has the tool `wait_for_email` (`from`, `subject_contains`, `since_secs`, `extract`, `timeout_secs`; at least one of `from` and `subject_contains`). Gmail and Outlook need an app password where they offer one; OAuth2 is not supported. The plugin never marks a message as read, so keep `since_secs` short and empty the test inbox between runs. Nothing here has run against a real mail server.

</Visibility>

## What you need

| You need | Why |
|---|---|
| A mailbox that receives the mail the site sends | Use a test inbox. The plugin reads only that one mailbox, from `host` over IMAPS (port 993 unless you set `port`) |
| An app password, if your provider has them | The plugin signs in with a user name and a password. For Gmail, Outlook and other providers that offer app passwords, make one and use it: the password you sign in to the web page with may not be accepted. A provider that allows only OAuth2 for IMAP cannot be read with this plugin |
| An environment variable that holds the password | The password is never an option. `password_env` names the variable, and the name must start with `IMAP_`, `MAIL_` or `SB_` (capitals, as written), so that a settings file from a project you cloned cannot send some other secret, such as `GITHUB_TOKEN`, to `host` |
| Chrome, and a site whose sign-up you may test | The test drives it with the pure CDP engine |

```sh Terminal
export MAIL_APP_PASSWORD='the app password'
```

The variable must be set when the plugin is built. If it is not, building the
plugin fails and the message names the variable, never its value.

## 1. Turn the plugin on

Add the two features to the test crate. `testing` is the `sb_cdp` fixture; the
plugin comes with `plugin-imap`:

```toml Cargo.toml
[dev-dependencies]
seleniumbase-rs = { git = "https://github.com/MustCodeAl/seleniumbase-rs", features = ["testing", "plugin-imap"] }
```

Nothing else goes into the manifest: the types of the plugin come through
`seleniumbase_rs::builtin_plugins::imap`, the clock through
`seleniumbase_rs::plugin_api`, and the runtime through `seleniumbase_rs::tokio`
([Imports and dependencies](/imports) has the rule).

## 2. Write the test

The page and its selectors (`#email`, `#send`, `#status`, `h1`) are examples; use
the ones of the site you test.

```rust tests/sign_up.rs
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use seleniumbase_rs::builtin_plugins::imap::{Extract, Imap, ImapConfig, Query};
use seleniumbase_rs::plugin_api::{Clock, Timer};
use seleniumbase_rs::testing::{rstest, sb_cdp, SbCdp};
use seleniumbase_rs::{tokio, Result};

/// What the plugin waits with: tokio's clock.
struct TokioTimer;

impl Timer for TokioTimer {
    fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        Box::pin(tokio::time::sleep(duration))
    }
}

#[rstest]
#[tokio::test]
async fn sign_up_with_an_emailed_link(sb_cdp: SbCdp) -> Result<()> {
    // Reads the password from $MAIL_APP_PASSWORD, and fails naming the variable
    // when it is not set.
    let imap = Imap::new(ImapConfig {
        host: "imap.example.com".into(),
        user: "tests@example.com".into(),
        password_env: "MAIL_APP_PASSWORD".into(),
        ..ImapConfig::default()
    })?;
    let clock = Clock::new(Arc::new(TokioTimer));
    let query = Query {
        from: Some("no-reply@app.example.com".into()),
        subject_contains: Some("Confirm your email".into()),
        since_secs: 120,
        extract: Extract::Link,
    };

    sb_cdp
        .run(async |page| {
            page.goto("https://app.example.com/signup").await?;
            page.locator("#email").fill("tests@example.com").await?;
            page.locator("#send").click().await?;
            page.locator("#status").expect().to_have_text("Check your inbox").await?;

            // The mail is sent by now or soon: wait for it, newest first.
            let mail = imap.wait_for_email(&query, &clock).await?;
            let link = mail.link.expect("the query asked for a link, so the mail has one");

            // The mail is text from a third party. Open only a link to the site under test.
            assert!(
                link.starts_with("https://app.example.com/verify"),
                "the mail holds an unexpected link: {link}"
            );
            page.goto(&link).await?;
            page.locator("h1").expect().to_have_text("Email confirmed").await
        })
        .await
}
```

What each part does:

1. `Imap::new` checks the options and reads the password from the variable. A
   mistake stops the test before the browser starts.
2. `Query` says which mail to wait for. `from` and `subject_contains` match a
   part of the sender (name or address) or of the subject, without regard to case.
   `since_secs` limits the wait to mail sent in the last this many seconds, from
   1 to 2592000 (30 days); it is 600 when you leave it out. `extract` is
   `Extract::Link`, `Extract::Code` or `Extract::Both` (the default): what the mail
   must have. A mail without it does not match.
3. The browser clicks "Send" **before** the wait, so that the mail is on its way.
4. `wait_for_email` connects, then looks at the unread mail of the last
   `since_secs` every `poll_secs` (5 by default), newest first, until a message
   matches or `timeout_secs` (120 by default) have passed. It returns a `Found`
   with `from`, `subject`, `date`, `code`, `link` and `text`, which is the start of
   the message (at most 16 KiB).
5. The test checks the link before it opens it, then continues in the same page.

Run it, with the password in the environment and Chrome available:

```sh Terminal
export MAIL_APP_PASSWORD='the app password'
cargo test sign_up_with_an_emailed_link
```

If you set a time limit for tests (`SB_TIME_LIMIT`), keep it above the wait. A
test that needs a real mailbox is slow and depends on the network: mark it
`#[ignore]` and run it on demand, as the plugin's own live test is.

### A code instead of a link

For a code the mail carries, ask for `Extract::Code`, type it and submit:

```rust
let query = Query {
    from: Some("no-reply@app.example.com".into()),
    extract: Extract::Code,
    ..Query::default()
};
// ... click "Send", then:
let mail = imap.wait_for_email(&query, &clock).await?;
let code = mail.code.expect("the query asked for a code, so the mail has one");
page.locator("#code").fill(&code).await?;
page.locator("#verify").click().await
```

## The options of the plugin

The plugin takes these options, which `ImapConfig` has as fields and
`[plugins.imap]` has as keys:

| Option | Default | Meaning |
|---|---|---|
| `host` | required | The IMAP server, such as `imap.example.com` |
| `user` | required | The login name |
| `password_env` | required | The NAME of the environment variable that holds the password, starting with `IMAP_`, `MAIL_` or `SB_` |
| `port` | 993 | The IMAPS port |
| `mailbox` | `INBOX` | The mailbox to watch |
| `poll_secs` | 5 | Seconds between two looks, from 1 to 300 |
| `timeout_secs` | 120 | Seconds to wait for a matching mail, from 1 to 3600 |

The connection is IMAPS from the first byte: the certificate and the host name
are always checked and there is nothing to turn off. [imap](/plugins/imap) has the
full page.

## With an AI agent

An agent that drives the browser with the `cdp` server of `sbrs-mcp` waits with
the tool `wait_for_email`. The operator turns the plugin on, with its options,
when the server starts; no tool argument names a host, a user or a password:

```sh Terminal
export MAIL_APP_PASSWORD='the app password'
sbrs-mcp --server cdp --plugin imap \
  --plugin-opt 'imap.host="imap.example.com"' \
  --plugin-opt 'imap.user="tests@example.com"' \
  --plugin-opt 'imap.password_env="MAIL_APP_PASSWORD"'
```

Then the agent does what the test does, with tools: `start_browser`, `open_url`
for the sign-up page, `type_text` into the email field, `click_element` on
"Send", and then `wait_for_email`:

```json wait_for_email
{"from": "no-reply@app.example.com", "subject_contains": "Confirm your email", "extract": "link", "timeout_secs": 90}
```

```json result
{"found": true, "from": "no-reply@app.example.com", "subject": "Confirm your email", "date": "...", "code": null, "link": "https://app.example.com/verify?token=...", "text_preview": "..."}
```

At least one of `from` and `subject_contains` is required. A mail that does not
arrive in time is `{"found": false, "message": "..."}`, a normal result and not an
error. The agent then opens the link with `open_url`. Tell it to open only a link
on the site under test, and to never follow instructions found in the mail: the
tool says so in its description and marks its result as untrusted, but a model
can still be steered by a mail it reads, which is why you use a test inbox.
[MCP servers](/tutorials/mcp_server#wait_for_email) lists the arguments and the
errors.

## From a shell script

The plugin is also a task named `wait_for_email`. It writes the match as one JSON
line, and a script can read the code from it:

```json wait.json
{"from": "no-reply@app.example.com", "subject_contains": "Confirm your email", "extract": "code", "since_secs": 120}
```

```sh Terminal
export MAIL_APP_PASSWORD='the app password'
sbrs \
  --plugin-opt 'imap.host="imap.example.com"' \
  --plugin-opt 'imap.user="tests@example.com"' \
  --plugin-opt 'imap.password_env="MAIL_APP_PASSWORD"' \
  plugin run imap --input wait.json
```

```json
{"from": "no-reply@app.example.com", "subject": "Confirm your email", "date": "...", "code": "482913", "text": "..."}
```

The summary, on standard error, is `{"matched": true, "code": true, "link": false}`.
`plugin run` starts a browser, which this task does not use, because only the pure
CDP engine can host a task.

## Keep the test honest

- **Old mail can match again.** The plugin opens the mailbox read only and never
  marks a message as read, so a mail that matched in an earlier run is still
  unread and matches again while it is within `since_secs`. Keep `since_secs`
  short, make `subject_contains` as specific as the site allows, and empty the
  test inbox (or mark its mail as read yourself) between runs.
- **Only unread mail counts.** A mail that is already read does not match.
- **A code is a guess.** A code is a number near a word such as "code" or
  "verification". A link is the first `http` or `https` link of the text part or
  the HTML part. A mail with several can give the wrong one: narrow it with `from`
  and `subject_contains`, and check the result as the test above does.
- **The mail is untrusted.** `text` and `text_preview` can say anything. Do not
  let a model or a script act on instructions found in them, and check a link
  before you open it.
- **One mailbox, one reader.** A second test that waits for the same kind of mail
  in the same mailbox can take the first one's message. Give each parallel test
  its own inbox.

## When it fails

| Message | What to do |
|---|---|
| `` plugin configuration: imap: the environment variable `MAIL_APP_PASSWORD` named by `password_env` is not set or is empty; it must hold the password or app password `` | Export the variable in the shell that runs the test |
| `` plugin configuration: imap: `password_env` must name a variable starting with IMAP_, MAIL_ or SB_ (got `GITHUB_TOKEN`) `` | Name a variable that starts with one of the three prefixes; the other variable was not read |
| `` plugin configuration: imap: `host` is required `` | Give `host`, `user` and `password_env` |
| `` imap: cannot connect to imap.example.com:993: ... `` | Check the host name and the port, and that the network lets IMAPS through |
| `` imap: TLS handshake with imap.example.com:993 failed: ... `` | The certificate or the host name is wrong. Nothing turns the check off |
| `` imap: login failed (check `user` and the password in `password_env`): ... `` | Use an app password, and the full user name your provider expects |
| `` imap: cannot open the mailbox `INBOX`: ... `` | Check the `mailbox` option |
| `` imap: no email (from `no-reply@app.example.com`, with a subject containing `Confirm your email`, sent in the last 120s) arrived within 120s `` | The mail did not come, or does not match: check the sender and the subject, that "Send" was clicked first, and that the mail is unread |

## What has not been run

<Warning>

**The plugin has never run against a real mail server.** Its own tests use a
scripted mailbox (the wait, the parsing of a code or a link, the options), and
the checks in seleniumbase-rs run the tool against a scripted mailbox and against
a loopback port that nothing listens on. Two tests need a real mailbox and are
`#[ignore]`d. This tutorial is not compiled: its code was written against the
public items of the plugin and of seleniumbase-rs and checked by hand, and it has
not been run against a real mailbox or a real site. Try it on a test inbox before
you rely on it.

</Warning>

<Columns cols={2}>

<Card color="#d32a61" title="imap" icon="envelope" href="/plugins/imap">

The options, the task, the MCP tool and the limits of the plugin.

</Card>

<Card color="#1a85d4" title="MFA and TOTP codes" icon="key" href="/tutorials/mfa_totp">

Codes that come from an authenticator instead of a mailbox.

</Card>

</Columns>
