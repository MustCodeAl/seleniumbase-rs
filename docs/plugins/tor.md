---
title: "tor Plugin"
sidebarTitle: "tor"
---

<Badge color="orange">Early</Badge> <Badge color="blue">Feature: plugin-tor</Badge>

Sends the traffic of the Chromium browser that seleniumbase-rs starts through a
Tor daemon on your machine, so that the sites you test do not see the address of
the person running the automation, and so that you can test your own onion
services. Names are sent to Tor as names, so Tor resolves them and `.onion`
addresses work. With the default `isolation`, every browser context gets
circuits of its own.

<Note>

It protects the person who runs the automation. It is **not** a way around bans
or rate limits, and it has no call that asks Tor for a new identity. **You run
Tor**: `sbrs` never installs, starts or stops a Tor daemon, and the plugin talks
to loopback addresses only. It is for a browser that the program starts on the
pure CDP engine: it refuses a browser the program attaches to, and it is refused
by the WebDriver engine. Pair it with [shield](/plugins/shield), which handles
what a proxy does not.

</Note>

<Visibility for="agents">

Summary for AI exports: tor is a built-in privacy plugin (feature `plugin-tor`, name `tor`) that routes the Chromium browser seleniumbase-rs starts through a Tor SOCKS listener the operator runs. It never installs or starts Tor and talks to loopback addresses only. Options live in `[plugins.tor]`: `socks` (`127.0.0.1:9050`, loopback only), `control` (a loopback control port, needs `control_auth`), `control_auth` (`{ cookie_path = "..." }` or `{ password_env = "NAME" }`), `isolation` (`per-context`, the default, or `browser`) and `require_bootstrapped` (true). In `before_launch` it refuses a launch that already has a proxy, checks that Tor answers as a SOCKS5 proxy (and, with a control port, that it finished bootstrapping), starts a local forwarder, sets the proxy, a host-resolver rule so that no name resolves on this machine, no QUIC and the WebRTC policy `disable_non_proxied_udp`. The forwarder adds the isolation parameter that Chrome never sends, so each browser context has its own circuits. It works with the pure CDP engine and a launched Chromium browser only. It is not a way around bans, it has no new-identity call, and nothing has been run against a real Tor daemon from seleniumbase-rs yet.

</Visibility>

## Turn it on

tor ships with seleniumbase-rs behind one cargo feature, and is switched on by
its name. See [Choosing plugins](/plugins/selecting) for every way to do that.

```toml Cargo.toml
[dependencies]
seleniumbase-rs = { git = "https://github.com/MustCodeAl/seleniumbase-rs", features = ["plugin-tor"] }
```

Start a Tor daemon yourself first (its SocksPort is `127.0.0.1:9050` unless you
changed it). Then switch the plugin on by name:

<Tabs>

<Tab title="sbase_config.toml" icon="gear">

```toml sbase_config.toml
[plugins.tor]
socks = "127.0.0.1:9050"
isolation = "per-context"
```

The table gives the options; the plugin itself is turned on by name too. A
settings file in the current folder cannot turn it on alone (it sets the proxy
and dials a port, so it declares `launch.proxy` and `host.net`): name it with
`--plugin tor` or `SB_PLUGINS=tor`.

</Tab>

<Tab title="sbrs command line" icon="terminal">

The pure CDP engine of `sbrs` is the one the plugin works with:

```sh Terminal
sbrs --plugin tor --plugin-opt 'tor.socks="127.0.0.1:9150"' devtools
```

and the `cdp` server of `sbrs-mcp`:

```sh Terminal
sbrs-mcp --server cdp --plugin tor
```

</Tab>

<Tab title="Rust code" icon="code">

```rust
use seleniumbase_rs::sb_cdp::LaunchOptions;

let options = LaunchOptions::builder()
    .headless(true)
    .plugins_named(["tor"])
    .build()?;
# Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
```

</Tab>

</Tabs>

A launch that already has a proxy (`--proxy`, a `--proxy-*` argument or
`--no-proxy-server`) is refused: remove it, or chain it in Tor's `torrc`.

## What it does

In `before_launch`, in this order:

1. It refuses the launch if a proxy is already set, before it changes anything:
   a proxy, or a `--proxy-*` switch. A PAC file address (`proxy_pac_url`) and a
   proxy bypass list reach plugins as the switches Chrome gets
   (`--proxy-pac-url`, `--proxy-bypass-list`), so they are refused too.
2. It checks Tor: something must answer as a SOCKS5 proxy at `socks`, and with a
   `control` port and `require_bootstrapped` (the default), Tor must report that
   it finished bootstrapping. A failure stops the launch before the browser
   starts, with a message that names the address.
3. It starts a local forwarder on a free loopback port and sets the proxy to
   `socks5://127.0.0.1:<port>`. It adds the host-resolver rule
   `MAP * ^NOTFOUND , EXCLUDE 127.0.0.1 , EXCLUDE localhost`, which stops any
   lookup the browser starts by itself from using this machine's resolver, turns
   QUIC off, sets the WebRTC policy `disable_non_proxied_udp` and the preference
   `net.network_prediction_options = 2`. It seals the proxy, the proxy of new
   contexts and the WebRTC policy, so no plugin that runs later can change them.

## Circuits for each context

Chrome speaks SOCKS5 without authentication and ignores a proxy URL that has a
user name, so it cannot ask Tor for circuits of its own. The plugin's forwarder
does it: it passes every connection to Tor with a random isolation parameter
(SOCKS credentials) of its own, and Tor's `IsolateSOCKSAuth`, on by default for a
SocksPort, puts streams with different parameters on different circuits.

With `isolation = "per-context"` (the default) every browser context made while
the plugin runs gets a forwarder of its own, and the forwarder stops when the
context closes. The browser's default context and its other tabs share the
browser's forwarder. With `isolation = "browser"` the whole browser shares one.
At most 128 contexts may be alive at once, and contexts are made one at a time.

## Options

`[plugins.tor]` in a settings file. An option that is not listed is an error, so
a typo cannot turn a protection off without a word.

<ParamField body="socks" type="string" default="127.0.0.1:9050">

Where Tor's SocksPort listens. Only a loopback address is accepted: any other is
an error that says so.

</ParamField>

<ParamField body="control" type="string" default="none">

Where Tor's control port listens, such as `127.0.0.1:9051`. It is used for one
thing: to read whether Tor finished bootstrapping. Only a loopback address is
accepted, and `control_auth` is required with it.

</ParamField>

<ParamField body="control_auth" type="table" default="none">

How to authenticate to the control port: `{ cookie_path = "/var/run/tor/control.authcookie" }`
(`SAFECOOKIE`: the raw cookie is never sent) or `{ password_env = "NAME" }`
(`HASHEDPASSWORD`; NAME is an environment variable that holds the password, which
is never an option value). Name exactly one.

</ParamField>

<ParamField body="isolation" type="string" default="per-context">

`per-context` or `browser`.

</ParamField>

<ParamField body="require_bootstrapped" type="boolean" default="true">

Refuse to start the browser until Tor reports that it finished bootstrapping. It
needs `control`; without it the plugin can only check that something answers as a
SOCKS5 proxy.

</ParamField>

Not supported: a call that asks Tor for a new identity (`SIGNAL NEWNYM`: it
invites evading rate limits, and the control port is read only here), and the
`COOKIE` and `NULL` control authentications.

## What it may do

`sbrs plugin info tor` shows what the plugin declares:

| Capability | Why |
|---|---|
| `launch.proxy`, `launch.network`, `launch.prefs` | It sets the proxy, the host-resolver rule and QUIC, and a preference |
| `contexts` | It gives each browser context a forwarder of its own |
| `host.net:loopback` | It connects to the SOCKS port and the control port, and nothing that is not on this machine |
| `host.fs.read` for one path | Only when `control_auth` has a `cookie_path`: the cookie file is the only file it reads |

## What seleniumbase-rs does for it

The plugin only fills in settings. The host applies them:

| Plugin setting | Reaches the browser as |
|---|---|
| `LaunchSettings::proxy` | `--proxy-server=socks5://127.0.0.1:<port>` |
| `host_resolver_rules` | One `--host-resolver-rules` switch |
| `disable_quic` | `--disable-quic` |
| `webrtc_policy` | `--force-webrtc-ip-handling-policy` |
| `prefs` | The profile's `Default/Preferences` (a dotted name is a path into the file) |
| `ContextSettings::proxy` | The `proxyServer` of `Target.createBrowserContext` |

A proxy you give with `--proxy` or the builder reaches the plugin as
`LaunchSettings::proxy` before any plugin runs, not as a `--proxy-server`
argument, so the plugin sees it and refuses the launch. The plugin is made again
for each browser a pool launches, so pooled browsers do not share forwarders, and
its forwarders stop when a context is disposed of and when the browser closes.

## Limits

<Warning>

**Nothing here has run against a real Tor daemon from seleniumbase-rs.** The
tests in seleniumbase-rs build the plugin, check its options and the launch
check, and show the command line refusing what it should; they never contact a
SOCKS port. The plugin crate's own tests use a fake SOCKS5 listener, and the
behaviour of Chrome that the design rests on was checked there with Chrome 156.
Try it against your own daemon, and check your address on a page that shows it,
before you rely on it.

</Warning>

- **Launched Chromium on the pure CDP engine only.** A browser the program
  attaches to (`cdp_url`) is refused, because its proxy cannot be set, and the
  WebDriver engine is refused for the browser contexts it cannot give.
- **Tor must have `IsolateSOCKSAuth`.** Per-context circuits rely on it, which is
  on unless the daemon turns it off with `NoIsolateSOCKSAuth`. The plugin does not
  check the daemon's SocksPort flags.
- **It does not make you anonymous by itself.** A page can still tell what the
  browser is (its fingerprint) and what you do in it. It hides the address, not
  the person.
- **It is not a sandbox.** Like every native plugin, tor is Rust code that runs
  with your program's rights.
