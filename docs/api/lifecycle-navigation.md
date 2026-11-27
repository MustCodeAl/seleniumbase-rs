---
title: "Lifecycle and navigation"
sidebarTitle: "Lifecycle and navigation"
description: "Create and close a BaseCase session, open pages and move through history."
---

<Note>

Methods are listed as `name(arguments)`, called on a `BaseCase` as `sb.name(...).await?`. The argument names, the selector syntax and the return types are explained in [Reading the API reference](/api/api_reference#reading-this-page).

</Note>

## Lifecycle

| Method | Description |
|--------|-------------|
| `BaseCase::new(config).await` | Creates and connects a browser session from a `BrowserConfig`. |
| `BaseCase::without_session(config)` | Creates a `BaseCase` without connecting a driver (used by some runners). |
| `sb.quit().await` | Ends the session; closes a browser it launched, or detaches from an external app and leaves it running. |
| `sb.reconnect().await` | Replaces the WebDriver session while keeping the driver process. |

On a [direct CDP attachment](/ecosystem/tauri-api/cdp) (`BrowserConfigBuilder::cdp_url`,
`cdp_discovery`) the whole `BaseCase` API runs through the pure CDP engine, as Python's
CDP Mode gives `sb.cdp` the methods of `sb`: navigation, finding and clicking, typing, the
waits and assertions, frames, windows and tabs (a window handle is the target id), cookies,
scripts, screenshots, alerts and the mouse. `BrowserSession::pure_cdp_page()` gives the
`sb_cdp::Page` for what `BaseCase` has no method for. A method that needs a driver
(`chromedriver_version`, `cdp_events`, `install_addon`) answers `SeleniumBaseError::Unsupported`
and names the alternative. `reconnect()` re-attaches to the endpoint of the configuration.
WebDriver session reuse and Recorder Mode are refused for direct attachments. The design is
[BaseCase over pure CDP](/design/basecase_over_cdp).

<Warning>

Always call `quit()` in a `Drop` guard, `sb_test!` macro, or `finally` block so
browser processes do not leak when a test fails.

</Warning>

## Navigation

| Method | Description |
|--------|-------------|
| `open(url)` | Navigates to `url`. |
| `open_url(url)` | Alias for `open`. |
| `open_if_not_url(url)` | Opens `url` only if it is not the current URL. |
| `go_back()` | Browser back. |
| `go_forward()` | Browser forward. |
| `refresh_page()` | Reloads the page. |
| `current_url()` | Returns the current URL. |
| `title()` | Returns the page title. |
| `page_source()` | Returns the full page source. |
| `open_new_window()` | Opens a new browser window. |
| `open_new_tab()` | Opens a new browser tab. |

## WebDriver BiDi

WebDriver BiDi provides bidirectional event streaming and network interception alongside classic commands. Dropping the receiver closes the channel, which automatically terminates the background task and unsubscribes from the wire event stream.

| Method | Return Type | Description |
|---|---|---|
| `bidi_listen_logs()` | `Result<Receiver<LogEntryAdded>>` | Subscribes to BiDi `log.entryAdded` events with default buffer capacity (256) forwarding console logs and JavaScript errors. |
| `bidi_listen_logs_with_buffer(buffer)` | `Result<Receiver<LogEntryAdded>>` | Subscribes to `log.entryAdded` events with a custom channel buffer capacity. |
| `bidi_listen_network()` | `Result<Receiver<BidiNetworkEvent>>` | Subscribes to BiDi network events (`beforeRequestSent`, `responseStarted`, `responseCompleted`, `fetchError`, `authRequired`) with default buffer (256). |
| `bidi_listen_network_with_buffer(buffer)` | `Result<Receiver<BidiNetworkEvent>>` | Subscribes to BiDi network events with a custom channel buffer capacity. |
| `bidi_listen_requests()` | `Result<Receiver<BeforeRequestSent>>` | Subscribes exclusively to outgoing request events (`network.beforeRequestSent`). |
| `bidi_listen_responses()` | `Result<Receiver<ResponseCompleted>>` | Subscribes exclusively to completed response events (`network.responseCompleted`). |
| `bidi_add_intercept(phases, patterns)` | `Result<InterceptGuard>` | Adds a BiDi interception rule pausing requests matching `phases` (e.g. `vec![InterceptPhase::BeforeRequestSent]`) and optional URL patterns. |
| `bidi_continue_request(request)` | `Result<()>` | Continues an intercepted network request unmodified via BiDi `network.continueRequest`. |
| `bidi_continue_response(request)` | `Result<()>` | Continues an intercepted response unmodified via BiDi `network.continueResponse`. |
| `bidi_fail_request(request)` | `Result<()>` | Fails an intercepted request with network error via BiDi `network.failRequest`. |
