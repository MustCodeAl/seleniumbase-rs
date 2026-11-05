---
title: "Pure CDP: Browser"
sidebarTitle: "Browser"
description: "Launch or attach to Chrome, open tabs and contexts, and close the browser."
---

<Note>

Methods are listed as `name(arguments)`, called on a `Browser` unless the heading names another type. End an async call with `.await?`. Argument names are explained in [Reading the API reference](/tutorials/api_reference#reading-this-page). The nesting of `Browser`, `Page` and `Locator` is on [Pure CDP](/api/cdp-overview).

</Note>

Import the types from `seleniumbase_rs::sb_cdp`. `Browser::launch` starts Chrome. `Browser::connect` attaches to one that is already listening. `close` shuts down a browser this process launched. Closing a browser that was only attached leaves the other process running.

A `LaunchOptions` is built with `LaunchOptions::builder()`, then `build()`, which returns a `Result`. `build` fails with `InvalidConfig` when incognito and guest are both set, when `use_chromium` is combined with an explicit executable path, or when the proxy string cannot be parsed. With the `plugins` feature it also fails with `Plugin` when a named plugin is unknown, refuses its options, or is named twice. `LaunchOptions` implements `Default`.

## Launch and attach

| Method | Description |
|--------|-------------|
| `launch(options)` | Starts Chrome from `LaunchOptions` and returns the `Browser`. |
| `connect(host, port)` | Attaches to a browser already listening on `host` and `port`. |
| `connect_with_plugins(host, port, plugins)` | Attaches, and runs the given `PluginSet`. Feature `plugins`. A launch-only plugin refuses. `close` leaves the other process running. |
| `over_channel(transport)` | Drives an embedded webview through a `CdpTransport` the app implements. Returns the browser and a `CdpInbox`. There is no debugging port, and this browser cannot open further tabs or windows. |
| `close()` | Closes the browser. |

With the `test-util` feature, `new_mocked()` returns `(Browser, MockCtrl)` and `launch_mocked(options, mock)` runs launch against that scripted transport, so tests need no Chrome.

## Address of the browser

| Method | Description |
|--------|-------------|
| `debugging_host()` | Host the DevTools endpoint is on. |
| `debugging_port()` | Port the DevTools endpoint is on. |
| `http_url()` | HTTP URL of the endpoint, such as `http://127.0.0.1:9222`. |
| `websocket_url()` | WebSocket URL of the browser target. |
| `devtools_endpoint()` | The endpoint as a `DevToolsEndpoint`. |
| `is_connected()` | Whether the connection is still up. |
| `extension_ids()` | Ids of extensions this launch loaded. |
| `events()` | An `Events` handle for browser-level DevTools events. |
| `execute(method, params)` | Sends one DevTools command. `params` is a JSON `Value`. Returns the result `Value`. |

## Tabs

| Method | Description |
|--------|-------------|
| `default_page()` | The first page, opening one if the browser has none. |
| `newest_page()` | The most recently opened page. `SessionNotStarted` when no tab is open. |
| `pages()` | Every open page, as `Page` handles. |
| `list_pages()` | Every open tab as `PageInfo` (`id`, `url`, `title`), without attaching. |
| `page(id)` | The page with this target id. |
| `select_page(selector)` | The tab with this target id, or else the first whose URL contains `selector`. An empty selector is `InvalidConfig`. The title is not matched. |
| `new_page()` | Opens a blank tab. |
| `new_page_at(url)` | Opens a tab at `url`. |
| `new_window()` | Opens a new window. |
| `new_window_at(url)` | Opens a new window at `url`. |

A `Page` from any of these waits 7 seconds for elements unless you call `with_timeout`.

## Permissions and downloads

`Permission` is a non-exhaustive enum: `Geolocation`, `Notifications`, `Microphone`, `Camera`, `Clipboard`, `Midi`, `Sensors`, `BackgroundSync`, `BackgroundFetch`, `ClipboardSanitizedWrite`, `DisplayCapture`, `DurableStorage`, `IdleDetection`, `LocalFonts`, `MidiSysex`, `Nfc`, `PaymentHandler`, `PeriodicBackgroundSync`, `StorageAccess`, `TopLevelStorageAccess`, `WakeLockScreen`, `WakeLockSystem`, `WindowManagement`. `Permission::ALL` is those 23.

| Method | Description |
|--------|-------------|
| `grant_permissions(permissions)` | Grants the listed permissions without a prompt. |
| `grant_permissions_for(origin, permissions)` | Grants them for one origin. |
| `grant_all_permissions()` | Grants `Permission::ALL`. |
| `reset_permissions()` | Clears the grants. |
| `set_download_dir(dir)` | Directory the browser saves downloads into. |
| `download_dir()` | That directory, as a `PathBuf`. |

Per-tab downloads (`download_file`, `download_by_click`) are on [Network](/api/cdp-network).

## Contexts

`new_context()` creates an isolated `BrowserContext`. `new_context_with(options)` takes a `ContextOptions`.

| On `ContextOptions` | Description |
|--------|-------------|
| `new()` | Empty options. |
| `proxy(proxy)` | A `Proxy` for this context. |
| `bypass(host)` | A host that skips that proxy. |

| On `BrowserContext` | Description |
|--------|-------------|
| `id()` | The context id. |
| `browser()` | The browser that owns it. |
| `new_page()` | A blank page in this context. |
| `new_page_at(url)` | A page at `url` in this context. |
| `dispose()` | Closes the context. |

`Browser::context_route(id)` returns the `ContextRoute` (`proxy`, `bypass`) recorded for a context, if any.

A pool of browsers and contexts is [Pool](/api/cdp-pool).

## LaunchOptionsBuilder

Chain these, then `build()`. Each returns the builder.

| Method | Description |
|--------|-------------|
| `headless(headless)` | Headless when true. |
| `url(url)` | Address to open first. |
| `executable_path(path)` | Chrome binary to run. |
| `use_chromium(use_chromium)` | Use Chromium rather than a branded Chrome. Cannot be combined with `executable_path`. |
| `incognito(incognito)` | Incognito profile. Cannot be combined with `guest`. |
| `guest(guest)` | Guest profile. Cannot be combined with `incognito`. |
| `user_data_dir(dir)` | Profile directory. Unset, the launch uses a temporary profile. |
| `user_agent(user_agent)` | `User-Agent` string. |
| `lang(lang)` | Browser language, such as `"en-US"`. |
| `window_size(width, height)` | Initial window size in pixels (`u32`). |
| `window_position(x, y)` | Initial window position in pixels (`i32`). |
| `maximize(on)` | Opens the window maximized and sends no `window_size`. A headless browser gets neither. |
| `mobile(on)` | Phone emulation for every tab: a 390 by 715 screen at pixel ratio 3, touch, and an Android user agent. A user agent set on the builder is kept. Turning this off also forgets `device_metrics`. `build` refuses it together with `fingerprint`. |
| `device_metrics(metrics)` | The phone screen `mobile` emulates, and turns mobile emulation on. `metrics` is `seleniumbase_rs::browser::options::DeviceMetrics::new(width, height, pixel_ratio)` (`u32`, `u32`, `f64`). |
| `dark_mode(on)` | Forces Chrome's dark theme on web pages. For `prefers-color-scheme` only, use `color_scheme` on [State](/api/cdp-state). |
| `devtools(on)` | Opens DevTools with each tab. A headless browser has no window, and gets nothing. |
| `no_sandbox(no_sandbox)` | Passes `--no-sandbox`. Also enabled by `SB_NO_SANDBOX`. |
| `disable_web_security(on)` | Allows cross-origin requests and ignores certificate errors. |
| `swiftshader(on)` | Software WebGL (SwiftShader). |
| `ad_block(ad_block)` | Blocks a built-in list of ad hosts. |
| `proxy(proxy)` | Proxy string. Parsed by `Proxy::parse`. |
| `proxy_bypass_list(list)` | Hosts that skip the proxy (`ProxyBypassList`). |
| `proxy_pac_url(url)` | A PAC file, written `[user:password@]URL`. The URL must end in `.pac`, or `build` fails. Chrome is not given the login. |
| `host_resolver_rules(rules)` | One host-resolver rule, such as `MAP example.com 127.0.0.1`. Rules from every call are joined. |
| `arg(arg)` | One extra Chrome command-line argument. |
| `switch(switch, on)` | Turns one typed `LaunchSwitch` on or off. |
| `disable_features(features)` | Chrome features to disable. |
| `enable_features(features)` | Chrome features to enable. |
| `extension(dir)` | Unpacked extension directory, loaded over the protocol after start. An attached browser loads none. |
| `preference(name, value)` | A Chrome preference merged into the profile before start. A dotted `name` is a path. |
| `virtual_display(display)` | A headed browser on Xvfb, for a Linux machine with no display. Other systems, and a headless browser, start none. A screen smaller than 1024 by 768 is raised to that. Unless `headless` is set, the browser is headed. |
| `startup_timeout(timeout)` | How long to wait until the browser is reachable (`Duration`). |
| `debugging_port(port)` | Listen on this port of `127.0.0.1` instead of a free port (`0`, the default). |
| `cdp_url(url)` | Attach instead of launching. An HTTP debugger URL, a browser WebSocket, or an authenticated embedded-webview URL. `close` does not stop the other process. |
| `cdp_target(target)` | When attaching, the window to drive: a target id, or part of its URL. Ignored when this process launches Chrome. |
| `fingerprint(fingerprint)` | Apply a `Fingerprint` to every tab as it opens. A setting made before this call wins; one made after replaces the fingerprint's. Flags in the fingerprint's `cmd_params` are not passed (add trusted flags with `arg`). |
| `webrtc_policy(policy)` | Chrome's WebRTC IP-handling policy (`WebRtcPolicy`). Unset, Chrome's own behaviour is kept. |
| `shield_webrtc(shield)` | Relay-only WebRTC in every tab. Pages that need a direct WebRTC call will not connect. |
| `browser_config(config)` | Copies what a `BrowserConfig` holds that this engine can use. WebDriver-only settings are logged at debug level and left out. Later builder calls replace these; a fingerprint is applied last. |
| `plugins(plugins)` | `PluginSet` whose hooks run at start, on each tab, and on close. Feature `plugins`. |
| `plugins_named(names)` | Built-in plugins by name, after `plugins`. Feature `plugins`. |
| `plugin_selection(selection)` | Plugins a `PluginSelection` names, with their options (the `[plugins]` table and `SB_PLUGINS`). Feature `plugins`. |

`LaunchOptions` also exposes what was set, without launching: `is_headless`, `proxy`, `url`, `user_agent`, `lang`, `cdp_url`, `cdp_target`, and `browser_args(profile_dir)` (the Chrome arguments for that profile directory).

## Proxy

`Proxy::parse(spec)` reads a proxy string. `server()` is the server, `has_credentials()` says whether a user and password were present, and `credentials()` is `Some((user, password))` or `None`.
