---
title: "Tauri WebDriver API"
description: "The W3C WebDriver endpoint that tauri-plugin-sbrs-automation serves inside a Tauri app: base URL, token, request and reply format, error codes, and every command group."
---

The [Tauri plugin](/ecosystem/tauri) serves a subset of W3C WebDriver over HTTP
from inside your app. `BaseCase`, `sbrs` and any other WebDriver client speak it
for you; these pages document each route as the plugin's code answers it, with
raw HTTP requests you can send with cURL.

<RequestExample>

```bash Start a session
# SBRS_AUTOMATION_TOKEN, or the token the app printed at start-up
TOKEN=secret
SESSION=$(curl -s -X POST "http://127.0.0.1:4445/$TOKEN/session" -d '{}' | jq -r .value.sessionId)
```

```bash Status
curl -s "http://127.0.0.1:4445/$TOKEN/status"
```

```bash Without the token
curl -s "http://127.0.0.1:4445/status"
```

```bash From a web page
curl -s "http://127.0.0.1:4445/$TOKEN/status" -H 'Origin: https://example.com'
```

</RequestExample>

<ResponseExample>

```json 200 Status
{
  "value": {
    "ready": true,
    "message": "tauri-plugin-sbrs-automation is ready"
  }
}
```

```json 404 Without the token
{
  "value": {
    "error": "unknown command",
    "message": "the URL does not start with the token",
    "stacktrace": ""
  }
}
```

```json 403 From a web page
{
  "value": {
    "error": "unknown error",
    "message": "requests from web pages are refused",
    "stacktrace": ""
  }
}
```

</ResponseExample>

## Base URL

```text
http://127.0.0.1:{port}/{token}/
```

Every path in this reference follows `http://127.0.0.1:4445/{token}`; a
session command follows `/session/{sessionId}`. When the endpoint starts, the
app prints the whole URL on stderr:

```text
tauri-plugin-sbrs-automation: WebDriver at http://127.0.0.1:4445/secret/
```

<ParamField path="token" type="string" required>

The secret first segment of every path: `SBRS_AUTOMATION_TOKEN`, or a random
one of 32 hexadecimal digits made when the app starts. A path that does not
start with `/{token}` followed by `/` or its end answers `404 unknown command`
(`the URL does not start with the token`).

</ParamField>

<ParamField path="port" type="integer" default="4445">

`SBRS_AUTOMATION_PORT`. If the port is taken (a second instance of the app, a
Selenium container), the app runs without the endpoint and says so on stderr.

</ParamField>

<Warning>

Give a WebDriver client the URL **with the trailing slash**. Clients join each
command path (`session`, ...) to the server URL as a relative URL, so without
the slash the token is replaced and every request is refused with `the URL does
not start with the token`.

</Warning>

The query string is ignored, and so are empty path segments (a doubled or a
trailing slash).

## Authentication

There is no credential header. The endpoint is guarded instead:

- it is on in debug builds only, unless `SBRS_AUTOMATION` turns it on (or off);
- it listens on `127.0.0.1` only;
- every path must start with the token;
- a request with an `Origin` header is refused.

<ParamField header="Origin" type="string">

Must be absent. Browsers send it with every request a web page makes and
WebDriver clients do not, so any request that has one answers `403 unknown
error` (`requests from web pages are refused`), whatever its path. This check
comes before the token's.

</ParamField>

## Requests

- The body is read as JSON whatever its `Content-Type`; an empty body counts as
  `{}`. A body that is not JSON answers `400 invalid argument` (`the body is
  not JSON: ...`), on any route. Bodies are cut at 16 MiB.
- The server answers one request at a time, on one thread: a long script or a
  navigation that waits for its page holds up the requests behind it.
- Most commands run as an operation of the page script (`bridge.js`) in the
  app's window, which has 30 seconds to answer (`500 timeout`, `the page did
  not answer ... in time`). Scripts get the session's script timeout instead.
  The window commands and screenshots ask the native window, not the page.

## Replies

Every reply is JSON, with `Content-Type: application/json; charset=utf-8`.
A command that succeeds answers `200` and its result under `value` (`null` for
commands that return nothing). A command that fails answers the HTTP status of
its W3C error code and an error object under `value`:

<ResponseField name="value" type="any" required>

The result of the command, or the error.

<Expandable title="error properties">

<ResponseField name="error" type="string" required>

The W3C error code, such as `no such element`. See [Errors](#errors).

</ResponseField>

<ResponseField name="message" type="string" required>

What went wrong, in words.

</ResponseField>

<ResponseField name="stacktrace" type="string" required>

Always the empty string.

</ResponseField>

</Expandable>

</ResponseField>

## Element references

An element travels as the W3C <Tooltip tip="Clients read an element only under this key; any other object is not an element to them.">web element reference</Tooltip>: an object with one key,
`element-6066-11e4-a52e-4f735466cecf`, whose value is the element's id.

```json
{ "element-6066-11e4-a52e-4f735466cecf": "e1-4fzyo82m" }
```

The page script makes the ids (`e`, a counter, `-`, eight random characters)
and keeps them for as long as the document lives: the same element always gets
the same id. An element removed from the page answers `404 stale element
reference`. Once the window loads another document, the old ids answer
`404 no such element` (`no element ... in this page`).

## Errors

The status is the one the W3C specification gives each code. The codes the
plugin produces, and when:

| Status | `error` | When |
|---|---|---|
| 400 | `invalid argument` | A body that is not JSON; a missing or wrong field (`url`, `using`, `value`, `script`, `text`, the timeouts, the window rectangle, the frame `id`, the actions); an unknown locator strategy. |
| 400 | `invalid selector` | A CSS selector or XPath the page cannot parse. |
| 400 | `element not interactable` | Clicking an element that is not displayed; sending text while an alert or a confirm is recorded. |
| 403 | `unknown error` | Any request with an `Origin` header. |
| 404 | `unknown command` | A path without the token; a route the plugin does not serve (`GET cookie is not supported`). |
| 404 | `invalid session id` | No session is open, or the id is not the current session's. |
| 404 | `no such element` | Nothing matches; an element id the page does not know; a pointer move onto an element outside the current frame. |
| 404 | `stale element reference` | The element is no longer in the page. |
| 404 | `no such window` | Switching to a handle other than `main`; the app has no window. |
| 404 | `no such frame` | A frame that cannot be entered (cross-origin, sandboxed, not a frame, no such index), or a frame entered that is gone. |
| 404 | `no such alert` | Reading the text with no dialog recorded. |
| 500 | `session not created` | The window did not load its page within 60 seconds, or the page could not be reset. |
| 500 | `timeout` | A navigation's page did not load within the page-load timeout; the page did not answer an operation within 30 seconds; WebKit did not draw a screenshot within 30 seconds. |
| 500 | `script timeout` | A script ran past the session's script timeout. |
| 500 | `javascript error` | A script threw; the page failed an operation without a code. |
| 500 | `unsupported operation` | Screenshots on Windows and Linux; wheel input, pen and touch pointers, `pointerCancel`; answering a dialog other than the page was answered; text for a prompt that has opened. |
| 500 | `move target out of bounds` | A pointer move outside the viewport. |
| 500 | `unable to capture screen` | An element with no area in view; WebKit gave no snapshot. |
| 500 | `unknown error` | The webview cannot be reached; the window refused a change; the page gave no rectangle for an element. |

Commands never fail with `unexpected alert open`: dialogs are answered when
they open (see [User prompts](/ecosystem/tauri-api/alerts)).

## Not served

Any other route answers `404 unknown command`, `{METHOD} {path} is not
supported`: among them cookies, shadow roots, and opening or closing windows.
Cookies are left out on purpose: on macOS the app's page is
`tauri://localhost`, where WKWebView keeps no cookies at all (`document.cookie`
reads back empty after a write).

## Badges

Every command was written against the W3C specification and is checked by the
plugin's unit tests. On the pages that follow,
<Badge color="green" icon="apple" stroke>Verified on macOS</Badge> marks a
command driven in a real app (the `tauri-profile-manager` example, macOS,
Tauri `3.0.0-alpha.4`), and
<Badge color="orange" icon="flask" stroke>Not run on a real app yet</Badge>
one that only the unit tests check. Nothing has been run on Windows or Linux
yet.

## Command groups

<Columns cols={2}>

<Card title="Status" icon="heart-pulse" href="/ecosystem/tauri-api/status">

`GET /status`: whether the endpoint answers.

</Card>

<Card title="Sessions" icon="play" href="/ecosystem/tauri-api/sessions">

Start and end the one session.

</Card>

<Card title="Timeouts" icon="clock" href="/ecosystem/tauri-api/timeouts">

Script, page-load and implicit-wait timeouts.

</Card>

<Card title="Navigation" icon="compass" href="/ecosystem/tauri-api/navigation">

URL, title, back, forward, refresh and the page source.

</Card>

<Card title="Window" icon="window-maximize" href="/ecosystem/tauri-api/window">

The handle, the rectangle, maximize, minimize and full screen.

</Card>

<Card title="Frames" icon="table-columns" href="/ecosystem/tauri-api/frames">

Enter same-origin frames and leave them.

</Card>

<Card title="Find elements" icon="magnifying-glass" href="/ecosystem/tauri-api/elements">

CSS, XPath, link text and tag name, and the active element.

</Card>

<Card title="Element state" icon="eye" href="/ecosystem/tauri-api/element-state">

Text, name, rectangle, state, attributes, properties and CSS.

</Card>

<Card title="Element interaction" icon="hand-pointer" href="/ecosystem/tauri-api/element-interaction">

Click, clear and type.

</Card>

<Card title="Scripts" icon="code" href="/ecosystem/tauri-api/scripts">

Run JavaScript, sync and async, in the current frame.

</Card>

<Card title="Actions" icon="keyboard" href="/ecosystem/tauri-api/actions">

W3C actions: a keyboard, mouse pointers and pauses.

</Card>

<Card title="User prompts" icon="message" href="/ecosystem/tauri-api/alerts">

Alert, confirm and prompt, answered before they open.

</Card>

<Card title="Screenshots" icon="camera" href="/ecosystem/tauri-api/screenshots">

The page or one element, as PNG, on macOS.

</Card>

</Columns>

<Visibility for="agents">

Summary for AI agents. Base URL `http://127.0.0.1:4445/{token}/` (port from
`SBRS_AUTOMATION_PORT`, token from `SBRS_AUTOMATION_TOKEN` or printed at
start-up). Send no `Origin` header. Bodies are JSON. Every reply is
`{"value": ...}`; an error is `{"value": {"error": "<W3C code>", "message": "...", "stacktrace": ""}}`.
Elements are `{"element-6066-11e4-a52e-4f735466cecf": "<id>"}`. One session at a
time; session routes start with `/session/{sessionId}`.

- `GET /status`
- `POST /session`; `DELETE /session/{sessionId}`
- `GET /timeouts`; `POST /timeouts`
- `POST /url`; `GET /url`; `GET /title`; `POST /back`; `POST /forward`; `POST /refresh`; `GET /source`
- `GET /window`; `GET /window/handles`; `POST /window`; `GET /window/rect`; `POST /window/rect`; `POST /window/maximize`; `POST /window/minimize`; `POST /window/fullscreen`
- `POST /frame`; `POST /frame/parent`
- `POST /element`; `POST /elements`; `POST /element/{elementId}/element`; `POST /element/{elementId}/elements`; `GET /element/active`
- `GET /element/{elementId}/text`, `/name`, `/rect`, `/enabled`, `/selected`, `/displayed`, `/attribute/{name}`, `/property/{name}`, `/css/{name}`
- `POST /element/{elementId}/click`; `POST /element/{elementId}/clear`; `POST /element/{elementId}/value`
- `POST /execute/sync`; `POST /execute/async`
- `POST /actions`; `DELETE /actions`
- `GET /alert/text`; `POST /alert/accept`; `POST /alert/dismiss`; `POST /alert/text`
- `GET /screenshot`; `GET /element/{elementId}/screenshot` (macOS only)

Anything else answers `404 unknown command`.

</Visibility>
