---
title: "Pure CDP: Network"
sidebarTitle: "Network"
description: "Intercept requests, record a HAR, trace a tab, and download files on a pure CDP page."
---

<Note>

Methods are listed as `name(arguments)`, called on a `Page` unless the heading names another type. End an async call with `.await?`. Argument names are explained in [Reading the API reference](/tutorials/api_reference#reading-this-page).

</Note>

These calls watch or change one tab's network. A trace folder and a HAR can hold cookies, full URLs and anything the page showed: keep them out of version control and out of public CI artifacts.

## Interception

`page.intercept(rules)` starts an `Interception`. Rules run in the order you pass them. `log()` is the `Seen` requests so far (`request`, `outcome`). `stop()` ends the interception.

A `Request` has `url`, `method`, `resource_type` and `headers`. `ResourceType` is `Document`, `Stylesheet`, `Image`, `Media`, `Font`, `Script`, `Xhr`, `Fetch`, `WebSocket` or `Other`. `Outcome` is `Blocked`, `Fulfilled`, `Modified` or `Continued`.

A `Rule` with no condition matches every request. Conditions combine with "and". Naming a resource type more than once means any of those types. A URL glob uses `*` for any run of characters and `?` for one character.

| On `Rule` | Description |
|--------|-------------|
| `block()` | Fails the request before it reaches the network. |
| `fulfill(response)` | Answers with `response` and does not touch the network. |
| `modify()` | Sends the request on, with the changes below. |
| `url(glob)` | Only URLs that match `glob`. |
| `method(method)` | Only this HTTP method. |
| `resource_type(kind)` | Only this `ResourceType`. Call it again to allow another. |
| `with_header(name, value)` | Sets a request header. For a `modify` rule. |
| `remove_header(name)` | Removes a request header. |
| `redirect_to(url)` | Sends the request to another URL. |
| `change_method(method)` | Changes the HTTP method. |
| `with_body(body)` | Replaces the request body. |
| `matches(request)` | Whether this rule matches `request`. |

| On `Response` | Description |
|--------|-------------|
| `new(status, body)` | A response with status `status` and body bytes. |
| `json(status, value)` | A JSON body. |
| `html(status, html)` | An HTML body. |
| `header(name, value)` | Adds a response header. |

```rust
use seleniumbase_rs::json::json;
use seleniumbase_rs::sb_cdp::{Page, Response, Rule};

async fn stub_config(page: &Page) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    let rules = vec![
        Rule::fulfill(Response::json(200, &json!({"ok": true}))).url("*/api/config"),
    ];
    let capture = page.intercept(rules).await?;
    page.goto("https://example.com").await?;
    let _seen = capture.log();
    capture.stop().await?;
    Ok(())
}
```

## HAR

`start_har` records the tab's traffic as a HAR 1.2 log and returns a `HarRecorder`. `start_har_with(options)` takes `HarOptions`. `finish` stops the recording and returns a `Har`.

`HarOptions::new()` records no response bodies, with a 1 MiB cap ready for when bodies are turned on. `content(true)` fetches each response body into `content.text` (base64 for a binary body, with `content.encoding` set). A body over the cap is left out and `content.comment` says why. `max_content_bytes(bytes)` sets that cap. The default is `DEFAULT_MAX_CONTENT_BYTES` (1 MiB). A value above `MAX_CONTENT_BYTES` (64 MiB) counts as 64 MiB.

| On `Har` | Description |
|--------|-------------|
| `log` | The `HarLog`: `version`, `creator`, `browser`, `pages`, `entries`, `comment`. |
| `to_json()` | The log as a JSON string. |
| `save(path)` | Writes that JSON to `path`. |

The other HAR types (`HarEntry`, `HarRequest`, `HarResponse`, `HarTimings`, and the rest re-exported next to `Har`) are the HAR 1.2 records. Their fields are public.

## Trace

`start_trace(dir)` records the tab into a folder, with the default limits. `start_trace_with(dir, options)` takes a `TraceOptions`. Only what happens after the call returns is recorded. Each locator action (`click`, `fill`, `press`, and the others) and each `goto` is noted with its name, selector, time and result, then a screenshot and a DOM snapshot. Console messages and network traffic are kept beside them. The tab gets a small script (`window.__sbtrace`) that wraps the console methods, so the page can see a trace is running. Use it for debugging, not for a run that must look untouched.

`TraceRecorder::dir` is the folder. `stop` writes `index.html` (open it in a browser), `trace.json` and `network.har`, and returns a `Trace`. It also removes the page script. Dropping the recorder without `stop` does not write the viewer.

The folder must be absent (in a parent that exists) or an existing empty folder that is not a symbolic link. A second trace on the same tab fails with `InvalidConfig`.

What is left out of the recording: the text given to `fill`, `type_text`, `send_keys` and `set_value`; request bodies; `Authorization` and `Proxy-Authorization` values; the user name and password of a `goto` address. An address in the HAR is cut at 2048 characters. Screenshots still show what the page shows, including text typed into a plain field.

`TraceOptions::new()` is the default: 1000 actions, 64 MiB of screenshots and snapshots, 2000 console messages and 2000 requests. `max_actions`, `max_bytes`, `max_console_messages` and `max_requests` lower or raise those, up to 10_000 actions, 128 MiB, 20_000 console messages and 20_000 requests. The viewer repeats pictures inside `index.html` up to a separate budget of 192 MiB. Past the byte budget, later actions are still noted, without pictures.

`sbrs trace show FOLDER` opens that `index.html`.

## Downloads

The browser saves into `Browser::download_dir`. `set_download_dir` changes it. See [Browser](/api/cdp-browser).

| Method | Description |
|--------|-------------|
| `download_file(url, dir)` | The page fetches `url` and the browser saves the file in `dir` (created if needed). The name comes from `Content-Disposition`, else the last part of the address, else `download`. A file of that name is replaced. Returns the path the browser reports. The wait is the page timeout. |
| `save_file_as(url, path)` | The same fetch, saved at exactly `path`. `path` must end in a plain file name. Parent folders are created. Returns an absolute path. |
| `download_by_click(selector, dir)` | Clicks `selector` and returns the file that click downloads. A link that displays a page does not count as a download. |
| `assert_downloaded_file(name, timeout)` | Waits until `name` is a finished file in the download directory. `name` must be a plain file name. A `name.crdownload` file does not count. |
