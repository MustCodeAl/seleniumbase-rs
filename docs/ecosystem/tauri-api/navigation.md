---
title: "Navigation"
description: "Navigate the Tauri app's window and read its URL, title and page source through the plugin's WebDriver endpoint."
---

Load pages in the app's window, go back and forward, and read the page. Paths
follow `http://127.0.0.1:4445/{token}`; see the
[overview](/ecosystem/tauri-api/overview) for the reply format and the errors.

<RequestExample>

```bash Navigate to
curl -s -X POST "http://127.0.0.1:4445/$TOKEN/session/$SESSION/url" \
  -d '{"url": "tauri://localhost/"}'
```

```bash Get current URL
curl -s "http://127.0.0.1:4445/$TOKEN/session/$SESSION/url"
```

```bash Get title
curl -s "http://127.0.0.1:4445/$TOKEN/session/$SESSION/title"
```

```bash Back
curl -s -X POST "http://127.0.0.1:4445/$TOKEN/session/$SESSION/back" -d '{}'
```

```bash Get page source
curl -s "http://127.0.0.1:4445/$TOKEN/session/$SESSION/source"
```

</RequestExample>

<ResponseExample>

```json 200 Navigate to
{ "value": null }
```

```json 200 Get current URL
{ "value": "tauri://localhost/" }
```

```json 200 Get title
{ "value": "Profiles — SeleniumBase Profile Manager" }
```

```json 200 Get page source
{ "value": "<html><head>...</head><body>...</body></html>" }
```

```json 500 timeout
{
  "value": {
    "error": "timeout",
    "message": "the page did not finish loading",
    "stacktrace": ""
  }
}
```

</ResponseExample>

A navigation waits for the new page: the page script answers that it is
leaving until the old page is gone, so the old page's `complete` is never taken
for the new one, and the command answers once the new document's
`readyState` is `complete`, or `500 timeout` when the session's `pageLoad`
timeout (60 seconds by default) is over. Every navigation leaves any frame
entered for the top page. The dialogs recorded, the dialog answer set and the
element ids are forgotten when the window loads another document.

## Navigate to

<Badge color="green">POST</Badge> `/session/{sessionId}/url`

Sets the top page's `location.href`. A URL that differs from the current one
only in its hash does not unload the page; the command then answers as soon as
the page is complete.

**Path**

<ParamField path="sessionId" type="string" required>

The current session's id.

</ParamField>

**Body**

<ParamField body="url" type="string" required>

The URL to load, absolute or relative to the current page.

</ParamField>

**Response**

<ResponseField name="value" type="null" required>

`null`, once the page has loaded.

</ResponseField>

**Errors**

| Status | `error` | When |
|---|---|---|
| 400 | `invalid argument` | `url` is missing or not a string (`` `url` is missing ``). |
| 500 | `timeout` | The page did not load within `pageLoad`. |

## Get current URL

<Badge color="blue">GET</Badge> `/session/{sessionId}/url`

The top page's `location.href`, also while a frame is entered.

**Path**

<ParamField path="sessionId" type="string" required>

The current session's id.

</ParamField>

**Response**

<ResponseField name="value" type="string" required>

The URL.

</ResponseField>

## Get title

<Badge color="blue">GET</Badge> `/session/{sessionId}/title`

<Badge color="green" icon="apple" stroke>Verified on macOS</Badge>

The top page's `document.title`, also while a frame is entered.

**Path**

<ParamField path="sessionId" type="string" required>

The current session's id.

</ParamField>

**Response**

<ResponseField name="value" type="string" required>

The title.

</ResponseField>

## Back, forward and refresh

<Badge color="green">POST</Badge> `/session/{sessionId}/back`

<Badge color="green">POST</Badge> `/session/{sessionId}/forward`

<Badge color="green">POST</Badge> `/session/{sessionId}/refresh`

Call `history.back()`, `history.forward()` or `location.reload()` in the top
page, then wait for the page like [Navigate to](#navigate-to). A history entry
of the same document (a hash change, `pushState`) fires `hashchange` or
`popstate`, and the command answers once the page is complete.

<Warning>

With no entry to go back or forward to, the page does not unload, so the
command answers `500 timeout` only when the `pageLoad` timeout is over.

</Warning>

**Path**

<ParamField path="sessionId" type="string" required>

The current session's id.

</ParamField>

**Response**

<ResponseField name="value" type="null" required>

`null`, once the page has loaded.

</ResponseField>

**Errors**

| Status | `error` | When |
|---|---|---|
| 500 | `timeout` | The page did not load within `pageLoad`. |

## Get page source

<Badge color="blue">GET</Badge> `/session/{sessionId}/source`

The `outerHTML` of the current frame's document element: of the
[frame entered](/ecosystem/tauri-api/frames), or of the top page.

**Path**

<ParamField path="sessionId" type="string" required>

The current session's id.

</ParamField>

**Response**

<ResponseField name="value" type="string" required>

The HTML, from `<html>` to `</html>`, without a doctype.

</ResponseField>

**Errors**

| Status | `error` | When |
|---|---|---|
| 404 | `no such frame` | The frame entered is gone or no longer same-origin. |
