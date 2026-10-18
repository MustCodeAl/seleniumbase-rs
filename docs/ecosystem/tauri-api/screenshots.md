---
title: "Screenshots"
description: "Screenshots of the Tauri app's page or of one element, as base64 PNG, through the plugin's WebDriver endpoint (macOS only for now)."
---

<Badge color="gray" icon="apple">macOS only</Badge> <Badge color="gray" icon="windows" stroke>Windows: unsupported operation</Badge> <Badge color="gray" icon="linux" stroke>Linux: unsupported operation</Badge>

A screenshot is a PNG, base64-encoded in `value`. Paths follow
`http://127.0.0.1:4445/{token}`; see the
[overview](/ecosystem/tauri-api/overview) for the reply format and the errors.

<RequestExample>

```bash Take screenshot
curl -s "http://127.0.0.1:4445/$TOKEN/session/$SESSION/screenshot" \
  | jq -r .value | base64 --decode > page.png
```

```bash Take element screenshot
curl -s "http://127.0.0.1:4445/$TOKEN/session/$SESSION/element/$ELEMENT/screenshot" \
  | jq -r .value | base64 --decode > element.png
```

</RequestExample>

<ResponseExample>

```json 200 Take screenshot
{ "value": "iVBORw0KGgoAAAANSUhEUgAA..." }
```

```json 500 unsupported operation
{
  "value": {
    "error": "unsupported operation",
    "message": "screenshots are taken on macOS only so far (WebView2 and WebKitGTK are not done)",
    "stacktrace": ""
  }
}
```

```json 500 unable to capture screen
{
  "value": {
    "error": "unable to capture screen",
    "message": "the element has no area in view",
    "stacktrace": ""
  }
}
```

</ResponseExample>

## Platforms

<View title="macOS" icon="apple">

The window's `WKWebView` takes the snapshot itself
(`takeSnapshotWithConfiguration`), at the screen's scale (so 2x on a Retina
display), and the plugin returns it as PNG. WebKit draws the page out of
process, so the snapshot comes from the web view, on the main thread; it has
30 seconds (`500 timeout`, `the web view did not draw a snapshot in time`).

</View>

<View title="Windows and Linux" icon="windows">

Windows (WebView2) and Linux (WebKitGTK) answer `500 unsupported operation`
for now. An element screenshot still scrolls the element into view first.

</View>

## Take screenshot

<Badge color="blue">GET</Badge> `/session/{sessionId}/screenshot`

<Badge color="green" icon="apple" stroke>Verified on macOS</Badge>

The whole web view. On a Retina display a 900 x 700 window gave a 1800 x 1400
PNG.

<ParamField path="sessionId" type="string" required>

The current session's id.

</ParamField>

<ResponseField name="value" type="string" required>

The PNG, base64-encoded.

</ResponseField>

## Take element screenshot

<Badge color="blue">GET</Badge> `/session/{sessionId}/element/{elementId}/screenshot`

<Badge color="green" icon="apple" stroke>Verified on macOS</Badge> <Badge color="orange" icon="flask" stroke>Not run on a real app yet: a zoomed page</Badge>

The part of the view the element covers, after it is scrolled into view: the
page scrolls it to the middle of the view at once, even when the page sets
`scroll-behavior: smooth`, and cuts its box to the viewports it is in (its
frames' and the window's). This also holds when the page is zoomed, as Tauri's
`set_zoom` and the zoom hotkeys do: the area is scaled by the web view's
`pageZoom`, which only a unit test of that conversion checks. On macOS a real
app took the screenshot of a paragraph far below the view on a page with
`scroll-behavior: smooth`.

<ParamField path="sessionId" type="string" required>

The current session's id.

</ParamField>

<ParamField path="elementId" type="string" required>

The element's id.

</ParamField>

<ResponseField name="value" type="string" required>

The PNG, base64-encoded.

</ResponseField>

## Errors

| Status | `error` | When |
|---|---|---|
| 404 | `no such element`, `stale element reference` | The element is unknown or gone. |
| 500 | `unsupported operation` | Windows and Linux. |
| 500 | `unable to capture screen` | The element has no area in view; the window holds no `WKWebView`; WebKit gave no snapshot or the snapshot cannot be encoded. |
| 500 | `timeout` | WebKit did not draw the snapshot within 30 seconds. |
