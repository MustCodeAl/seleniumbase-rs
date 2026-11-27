---
title: "browser Plugin (Containerized Persistent Sessions)"
sidebarTitle: "browser"
---

<Badge color="blue">Feature: plugin-browser</Badge> <Badge color="green">Containerized sessions</Badge>

Containerizes browser sessions and persists full session state across calls and runs. Cookies, `localStorage`, `sessionStorage`, authentication state, open tabs, and regional identity stay alive across workflows so you can build multi-page flows without reconnecting or repeating logins.

Browser extension loading (`.crx`, `.zip`, unpacked folders) lives in the [`crx` plugin](/plugins/builtin/crx). The two plugins have zero overlap and compose cleanly together (`--plugin crx --plugin browser`).

<Visibility for="agents">

Summary for AI exports: `browser` (`seleniumbase-rs-plugin-browser`) is a built-in plugin for containerized persistent browser sessions. It persists cookies, per-origin `localStorage` and `sessionStorage`, open tab URLs, and container identity (`country`, `locale`, `timezone`, `geolocation`, `user_agent`, sticky proxy index) to `<session_dir>/<session>.json` (atomic `0600` owner-only writes, v2 JSON format with v1 cookie-only backward compatibility) with `ttl_secs` expiry, geolocation targeting across 40 built-in country profiles, IP auto-rotation (`sticky`, `round-robin`, `per-context`) across launches and isolated contexts, headed interactive sign-in capture, and a multi-step dynamic SPA flow/extraction task (`sbrs plugin run browser`). Does not load extensions and does not conflict with `crx`.

</Visibility>

## Why use containerized browser sessions

- **One-line persistent containers**: Start a named container with `BrowserSession::container("checkout-account")?` or `BrowserSession::for_country("de-shop", "DE")?`.
- **Keep cookies, auth, storage, and open tabs alive**: Restores live cookies (`Storage.setCookies`), hydrates per-origin `localStorage` and `sessionStorage` before page scripts run, and re-opens saved `http://`/`https://` tabs (`restore_tabs = true`) across runs.
- **Dynamic SPA extraction and user interactions**: Execute multi-step flows (`goto`, `new_tab`, `wait_for`, `click`, `type`, `scroll`, `extract`, `save`) inside the same live container via `sbrs plugin run browser` without reconnecting between pages.
- **Geolocation targeting**: Set `country` (ISO 3166-1 alpha-2, 40 built-in country profiles) or explicit `locale`, `timezone`, and `geolocation` coordinates to align `--lang` and CDP `Emulation.*` overrides automatically.
- **IP auto-rotation**: Provide a proxy list (`proxies`) and choose `rotation` (`sticky`, `round-robin`, `per-context`) to rotate exit IPs across launches and isolated contexts. Credentials are redacted from `Debug`, logs, and errors.
- **Session TTL**: Configure `ttl_secs` so expired containers reset cleanly instead of reusing stale auth state.

## Quick start

<Tabs>

<Tab title="Rust (one line)" icon="code">

```rust,no_run
use seleniumbase_rs::plugin_api::PluginSet;
use seleniumbase_rs_plugin_browser::BrowserSession;

// Persistent named container with cookies, Web Storage, open tabs, and identity
let plugins = PluginSet::strict().with(BrowserSession::container("account-a")?)?;

// Geo-targeted container (derives de-DE, Europe/Berlin, Berlin coordinates)
let de_plugins = PluginSet::strict().with(BrowserSession::for_country("de-account", "DE")?)?;
Ok::<(), seleniumbase_rs::plugin_api::PluginError>(())
```

</Tab>

<Tab title="Rust (full builder)" icon="sliders">

```rust,no_run
use seleniumbase_rs::plugin_api::PluginSet;
use seleniumbase_rs_plugin_browser::{BrowserSession, RotationMode, SessionConfig};

let config = SessionConfig::builder()
    .container("checkout-flow")
    .session_dir("saved_sessions")
    .country("DE")
    .proxies([
        "http://user:pass@de1.proxy.example:8080",
        "http://user:pass@de2.proxy.example:8080",
    ])
    .rotation(RotationMode::PerContext)
    .ttl_secs(3600)
    .save_on_close(true)
    .restore_tabs(true)
    .build()?;

let plugins = PluginSet::strict().with(BrowserSession::new(config)?)?;
Ok::<(), seleniumbase_rs::plugin_api::PluginError>(())
```

</Tab>

<Tab title="sbase_config.toml" icon="gear">

```toml sbase_config.toml
[plugins]
enable = ["browser"]

[plugins.browser]
container = "checkout-flow"
country = "DE"
ttl_secs = 3600
save_on_close = true
restore_tabs = true
proxies = ["http://user:pass@de1.proxy.example:8080"]
rotation = "sticky"
```

</Tab>

<Tab title="SPA flow task" icon="terminal">

```sh Terminal
sbrs --plugin browser \
  --plugin-opt 'browser.session="checkout-flow"' \
  --plugin-opt 'browser.save_on_close=true' \
  plugin run browser --input '{
    "url": "https://app.example/dashboard",
    "steps": [
      { "action": "wait_for", "selector": ".orders-table", "timeout_ms": 8000 },
      { "action": "click", "selector": "button.load-more" },
      { "action": "scroll", "dy": 600 },
      { "action": "new_tab", "url": "https://app.example/account" }
    ],
    "extract": {
      "account_email": ".profile-email"
    },
    "save": true
  }'
```

</Tab>

</Tabs>

## Options

| Option | Type | Default | Description |
|---|---|---|---|
| `session` (alias `container`) | `string` | `None` | Container name (`1..=64` ASCII alnum, `.`, `_`, `-`). Stored at `<session_dir>/<name>.json` |
| `session_dir` | `path` | `"saved_sessions"` | Folder for container state bundles (created with `0700` permissions on Unix) |
| `save_on_close` | `bool` | `false` | Persist cookies, Web Storage, open tabs, and identity in `before_close` (pure CDP engine) |
| `restore_tabs` | `bool` | `true` | Re-open saved `http://`/`https://` tab URLs when restoring a container |
| `ttl_secs` | `u64` (`1..=2592000`) | `None` | Container expiration time in seconds; expired snapshots reset cleanly on load |
| `country` | `string` (ISO-2) | `None` | Regional target deriving default locale, timezone, and coordinates (40 countries built in) |
| `locale` | `string` | derived / `None` | Explicit BCP 47 locale (`--lang` and `Emulation.setLocaleOverride`) |
| `timezone` | `string` | derived / `None` | Explicit IANA timezone (`Emulation.setTimezoneOverride`) |
| `geolocation` | `[f64, f64]` | derived / `None` | Explicit `[latitude, longitude]` (`Emulation.setGeolocationOverride`) |
| `user_agent` | `string` | `None` | Explicit `User-Agent` override kept with container identity |
| `proxies` | `string[]` or CSV `string` | `[]` | Proxy pool (`http://`, `https://`, `socks5://`, `socks5h://`); credentials redacted in logs and `Debug` |
| `rotation` | `"sticky" \| "round-robin" \| "per-context"` | `"sticky"` | How exit proxies rotate across launches and isolated contexts |
| `interactive` | `bool` | `false` | Headed manual sign-in mode for `sbrs plugin run browser`, capturing cookies, storage, and open tabs |
| `pause_message` | `string` | built-in prompt | Custom prompt displayed during interactive sign-in |

## Container storage & security

- Container files are written atomically via temp sibling + `fsync` + `rename` with `0600` owner-only permissions on Unix (capped at 8 MiB, 5,000 cookies, 128 origins, 32 open tabs).
- Format v2 stores cookies, per-origin `localStorage`/`sessionStorage`, open tabs, `identity`, and `expires_at_ms`. Legacy v1 cookie-only files load transparently.
- Cookie values, storage values, and proxy credentials are never included in `Debug`, logs, or error messages.
