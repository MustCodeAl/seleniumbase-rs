---
title: "Tauri"
sidebarTitle: "Tauri overview"
description: "The three ways a Tauri program meets seleniumbase-rs: test your app (driver), automate a browser from your app (client), and drive your app's own webview (browser)."
keywords: ["tauri", "plugin", "webdriver", "desktop", "automation", "cef", "webview2", "cdp"]
---

<GitHub.Repo repo="MustCodeAl/tauri-plugin-sbrs-automation" />

<Badge color="orange" shape="pill">Early: 0.1</Badge> <Badge color="purple" shape="pill">Tauri 3 alpha</Badge> <Badge color="green" icon="apple" shape="pill">macOS: verified</Badge> <Badge color="gray" icon="windows" shape="pill" stroke>Windows: not run yet</Badge> <Badge color="gray" icon="linux" shape="pill" stroke>Linux: not run yet</Badge>

A [Tauri](https://tauri.app) program meets seleniumbase-rs in one of three
ways. Pick the one that matches your job and follow its guide.

<Visibility for="agents">

Summary for AI exports: three guides. Driver (`/ecosystem/tauri-driver`): a Rust test drives a Tauri app's window through the plugin's WebDriver endpoint. Client (`/ecosystem/tauri-client`): a Tauri app's own Rust code uses the `seleniumbase-rs` library to start and drive a separate browser, called from the window with `invoke`. Browser (`/ecosystem/tauri-browser`): a test attaches to the app's own Chromium webview (CEF or WebView2) over CDP with the plugin's `cdp` feature, or a shipped app gives its webview a fingerprint with the plugin's `stealth` feature. The route-by-route WebDriver reference is under `/ecosystem/tauri-api`.

</Visibility>

<Columns cols={3}>

<Card color="#1a85d4" title="Test your app: driver" icon="vial" href="/ecosystem/tauri-driver">

A Rust test drives your app's real window with `BaseCase`. Your app is the app
under test.

</Card>

<Card color="#d47628" title="Automate a browser: client" icon="window-maximize" href="/ecosystem/tauri-client">

Your app uses seleniumbase-rs inside its own Rust code to start and drive a
browser.

</Card>

<Card color="#a631be" title="Drive the app's webview: browser" icon="globe" href="/ecosystem/tauri-browser">

Attach to the app's own Chromium webview over CDP, or give it a fingerprint.

</Card>

</Columns>

## Which guide

| You want to | Who drives whom | What you add | Guide |
|---|---|---|---|
| Test your Tauri app's real window | A test written with seleniumbase-rs drives your app | The plugin behind a Cargo feature; `seleniumbase-rs` in the tests | [Driver](/ecosystem/tauri-driver) |
| Start a browser from your app and drive it | Your app uses seleniumbase-rs as a library and drives a separate Chrome | `seleniumbase-rs` in the app; no plugin | [Client](/ecosystem/tauri-client) |
| Drive the app's own Chromium webview | A test attaches over CDP, or the shipped app drives its own webview | The plugin's `cdp` or `stealth` feature; an app that owns CEF or WebView2 | [Browser](/ecosystem/tauri-browser) |

## The plugin

The plugin, `tauri-plugin-sbrs-automation`, has three parts. Each is for one
of the guides:

| Part | What it does | Guide |
|---|---|---|
| Default | A W3C WebDriver endpoint inside the app, on `127.0.0.1` behind a secret token, on in debug builds. It exists because macOS (WKWebView) has no WebDriver driver. | [Driver](/ecosystem/tauri-driver) |
| `cdp` feature | Launch settings that turn on the debugging port of an app-owned CEF or WebView2, and the discovery route `GET /cdp`. | [Browser](/ecosystem/tauri-browser) |
| `stealth` feature | A fingerprint for the app's own webviews, through the webview's in-process channels: no port, no listener. Fit for a release build. | [Browser](/ecosystem/tauri-browser) |

The [client guide](/ecosystem/tauri-client) needs only the `seleniumbase-rs`
crate.

## Status

<Note>

Early (plugin 0.1, Tauri 3 alpha). The WebDriver endpoint is verified on macOS
with a real app. Windows and Linux have not been run yet. The CEF and WebView2
paths (the `cdp` and `stealth` features) are verified against mock servers and
fake channels, not against a real CEF or WebView2 app. Each guide says what was
run.

</Note>

<Card color="#199f43" title="WebDriver API reference" icon="code" href="/ecosystem/tauri-api/overview">

Every route of the plugin's endpoint with its fields, replies and errors, and
the [CDP discovery route](/ecosystem/tauri-api/cdp).

</Card>

## A complete app

The [Tauri profile manager](https://github.com/MustCodeAl/tauri-plugin-sbrs-automation/tree/main/examples/tauri-profile-manager)
is an example app. It starts browsers from its own Rust code (client), opens
profiles in windows of its own with a fingerprint (the stealth part of the
browser guide), and has an end-to-end test that drives its window through the
endpoint (driver).

## License

MIT.
