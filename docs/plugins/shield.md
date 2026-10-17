---
title: "shield Plugin"
---

# shield

<Badge color="orange">Early</Badge> <Badge color="blue">Feature: plugin-shield</Badge>

Privacy hardening for the Chromium browser that seleniumbase-rs starts. It
limits what your own browser sends about you: it turns off the browser's
background reporting, keeps WebRTC from leaking the machine's addresses, blocks
third-party cookies and refuses to keep a profile on disk.

It does **not** spoof fingerprints or hide automation. That is the job of
[seleniumbase-rs-stealth](https://github.com/MustCodeAl/seleniumbase-rs-stealth).

## Turn it on

shield ships with seleniumbase-rs behind one cargo feature, and is switched on
by its name. See [Choosing plugins](selecting.md) for every way to do that.

```toml
[dependencies]
seleniumbase-rs = { git = "https://github.com/MustCodeAl/seleniumbase-rs", features = ["plugin-shield"] }
```

Then, with no code, in `sbase_config.toml`:

```toml
[plugins]
enable = ["shield"]

[plugins.shield]
level = "strict"
```

or `SB_PLUGINS=shield`, or on the command line of `sbrs` and `sbrs-mcp`, which
have shield compiled in:

```sh
sbrs --plugin shield --plugin-opt 'shield.level="strict"' open https://example.com
```

or in code:

```rust
use seleniumbase_rs::sb_cdp::LaunchOptions;

let options = LaunchOptions::builder()
    .headless(true)
    .plugins_named(["shield"])
    .build()?;
# Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
```

shield needs a Chromium-based browser. It is refused with Firefox and Safari,
and it works in both engines (the pure CDP engine and `BaseCase`).

## What it turns off

In `before_launch`, in this order:

1. With `ephemeral_profile` (the default) it refuses a launch that set a
   persistent profile, before it changes anything.
2. It adds these browser switches, which stop the browser's background network
   use:

   | Switch | Effect |
   |---|---|
   | `--disable-background-networking` | Several subsystems that make requests in the background |
   | `--disable-component-update` | Component update checks |
   | `--disable-domain-reliability` | Domain Reliability Monitoring |
   | `--disable-sync` | Syncing browser data to a Google Account |
   | `--no-pings` | Hyperlink auditing pings |
   | `--disable-default-apps` | Installing default apps on first run |

3. It turns off the features `OptimizationHints` and `MediaRouter`. seleniumbase-rs
   puts them into the one `--disable-features` switch it passes, next to the
   `Translate` it turns off itself.
4. It sets privacy preferences: no network prediction, search suggestions, Link
   Doctor error pages or Google spelling service, and no Privacy Sandbox ad
   topics, Protected Audience or ad measurement. With `block_third_party_cookies`
   it blocks third-party cookies. With `safe_browsing = false` (and only then) it
   sets the preference that turns Safe Browsing off.
5. Unless `webrtc` is `off`, it sets Chrome's `disable_non_proxied_udp` IP-handling
   policy (`--force-webrtc-ip-handling-policy`).
6. At the `strict` level it makes the browser unable to resolve the hosts
   listed under [Levels](#levels).

In `on_page_created` it blocks the `extra_block_hosts` (and their subdomains, on
any port) with `Network.setBlockedURLs`. seleniumbase-rs merges that list with
the blocklists of other plugins and its own ad-block list, so none replaces the
others.

## Options

`[plugins.shield]` in a settings file. An option that is not listed is an error,
so a typo cannot turn a protection off without a word.

| Option | Default | Meaning |
|---|---|---|
| `level` | `basic` | `strict` also makes the browser unable to resolve a few Google hosts |
| `webrtc` | `relay-only` | `off`, `policy` or `relay-only` |
| `block_third_party_cookies` | `true` | `false` leaves the browser's own setting; it never allows them |
| `ephemeral_profile` | `true` | `false` allows a persistent profile (`user_data_dir`) |
| `extra_block_hosts` | `[]` | Plain host names, no scheme, port, path or `*` |
| `safe_browsing` | `true` | `false` sets the preference that turns it off; nothing else does |
| `sweep_stale_profiles` | `false` | **Not implemented**; `true` is an error |
| `clear_between_tests` | `false` | **Not implemented**; `true` is an error |

The last two need hooks the plugin API does not have yet. They are refused
instead of accepted, so no settings file promises a protection that is not
running.

## Levels

| Level | Adds |
|---|---|
| `basic` | The switches, features and preferences above, and the WebRTC policy |
| `strict` | Everything in `basic`, and `--host-resolver-rules` that make these hosts fail to resolve: `update.googleapis.com`, `clientservices.googleapis.com`, `optimizationguide-pa.googleapis.com`, `content-autofill.googleapis.com`, `clients4.google.com`, `clients2.google.com` |

`strict` is wide. The block is a browser-wide rule in the network stack, so a
page that loads something from one of those hosts fails too.
`clients2.google.com` also serves Chrome Web Store and extension update
downloads, and `clients4.google.com` is also a connectivity-check host.

## WebRTC modes

| `webrtc` | What it does |
|---|---|
| `off` | Leaves WebRTC as the browser has it |
| `policy` | Sets the `disable_non_proxied_udp` policy |
| `relay-only` | The policy, and a small script in every tab that forces `RTCPeerConnection` to use relay candidates only |

A relay needs a TURN server, so a call between peers without one does not
connect with `relay-only`. Choose `policy` if you need those calls.

## What seleniumbase-rs does for it

The plugin only fills in settings. The host applies them:

| Plugin setting | Reaches the browser as |
|---|---|
| `host_resolver_rules` | One `--host-resolver-rules` switch, rules joined with commas |
| `disable_features`, `enable_features` | One `--disable-features` and one `--enable-features` switch, merged with seleniumbase-rs's own `Translate` |
| `webrtc_policy` | `--force-webrtc-ip-handling-policy` |
| the profile | Shown to the plugin as persistent when `user_data_dir` is set, so `ephemeral_profile` can refuse it |
| raw `Network.setBlockedURLs` | Merged into the tab's one blocklist |

## Limits

- **Nothing here has run in a real Chrome.** shield's own tests use the
  recording fakes of `seleniumbase-rs-plugin-testing`, and the checks in
  seleniumbase-rs run on its scripted mock browser. The switches, features,
  hosts and preference names were checked against the Chromium source, not
  against a running browser. Re-check them against each new Chrome stable.
- **`extra_block_hosts` is per tab session.** The block goes out as
  `Network.setBlockedURLs` on each tab's own protocol session. Whether
  cross-site iframes, workers and popups are covered depends on the browser
  applying the list to those targets. This was not verified. Treat it as a
  block for the tab's own requests, not as a firewall.
- **shield overwrites.** It sets its preferences and the WebRTC policy even if
  another plugin or the caller set them. Switches and features are only added.
  Put shield last if its values must win.
- **Safe Browsing.** shield sets no Safe Browsing preference or rule unless
  `safe_browsing = false`. What Chromium's own `--disable-background-networking`
  does to Safe Browsing list updates was not checked.
- **It is not a sandbox.** Like every native plugin, shield is Rust code that
  runs with your program's rights.
