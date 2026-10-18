---
title: "Find elements"
description: "Find elements in the Tauri app's page by CSS selector, XPath, link text or tag name, and get the active element, through the plugin's WebDriver endpoint."
---

Finding returns [element references](/ecosystem/tauri-api/overview#element-references)
for the other element commands. Paths follow `http://127.0.0.1:4445/{token}`;
see the [overview](/ecosystem/tauri-api/overview) for the reply format and the
errors.

<RequestExample>

```bash Find an element
curl -s -X POST "http://127.0.0.1:4445/$TOKEN/session/$SESSION/element" \
  -d '{"using": "css selector", "value": "#save"}'
```

```bash Find elements
curl -s -X POST "http://127.0.0.1:4445/$TOKEN/session/$SESSION/elements" \
  -d '{"using": "xpath", "value": "//li[@class=\"profile\"]"}'
```

```bash Find from an element
curl -s -X POST "http://127.0.0.1:4445/$TOKEN/session/$SESSION/element/$ELEMENT/element" \
  -d '{"using": "tag name", "value": "button"}'
```

```bash Get active element
curl -s "http://127.0.0.1:4445/$TOKEN/session/$SESSION/element/active"
```

</RequestExample>

<ResponseExample>

```json 200 Find an element
{
  "value": { "element-6066-11e4-a52e-4f735466cecf": "e1-4fzyo82m" }
}
```

```json 200 Find elements
{
  "value": [
    { "element-6066-11e4-a52e-4f735466cecf": "e2-k1q8s0dz" },
    { "element-6066-11e4-a52e-4f735466cecf": "e3-9vtx2m4a" }
  ]
}
```

```json 404 no such element
{
  "value": {
    "error": "no such element",
    "message": "no element matches css selector `#missing`",
    "stacktrace": ""
  }
}
```

```json 400 invalid selector
{
  "value": {
    "error": "invalid selector",
    "message": "'#' is not a valid selector.",
    "stacktrace": ""
  }
}
```

</ResponseExample>

## Locator strategies

| `using` | Finds |
|---|---|
| `css selector` | `querySelector` and `querySelectorAll`. |
| `xpath` | The element nodes of `document.evaluate`, in document order, evaluated from the root. |
| `link text` | `a` elements whose `innerText`, trimmed, equals `value`. |
| `partial link text` | `a` elements whose `innerText` contains `value`. |
| `tag name` | `getElementsByTagName`. |

<Badge color="green" icon="apple" stroke>Verified on macOS: css selector</Badge> <Badge color="orange" icon="flask" stroke>Not run on a real app yet: xpath, link text, tag name</Badge>

Any other `using` answers `400 invalid argument` (`` unknown locator strategy `...` ``),
and a selector or XPath the page cannot parse `400 invalid selector`, with the
page's own message.

Under the session's [implicit wait](/ecosystem/tauri-api/timeouts), finding
looks again every 50 ms while nothing matches, until the wait is over (the
default, 0, looks once).

## Find element

<Badge color="green">POST</Badge> `/session/{sessionId}/element`

<Badge color="green" icon="apple" stroke>Verified on macOS</Badge>

The first match in the current frame's document (see
[Frames](/ecosystem/tauri-api/frames)).

<ParamField path="sessionId" type="string" required>

The current session's id.

</ParamField>

<ParamField body="using" type="string" required>

The [locator strategy](#locator-strategies).

</ParamField>

<ParamField body="value" type="string" required>

The selector, XPath, link text or tag name.

</ParamField>

<ResponseField name="value" type="object" required>

The element's reference.

<Expandable title="properties">

<ResponseField name="element-6066-11e4-a52e-4f735466cecf" type="string" required>

The element's id, such as `e1-4fzyo82m`. The same element always gets the same
id.

</ResponseField>

</Expandable>

</ResponseField>

**Errors**

| Status | `error` | When |
|---|---|---|
| 400 | `invalid argument` | `using` or `value` is missing or not a string; an unknown strategy. |
| 400 | `invalid selector` | The page cannot parse the selector or the XPath. |
| 404 | `no such element` | Nothing matched before the implicit wait was over. |
| 404 | `no such frame` | The frame entered is gone or no longer same-origin. |

## Find elements

<Badge color="green">POST</Badge> `/session/{sessionId}/elements`

<Badge color="green" icon="apple" stroke>Verified on macOS</Badge>

Every match in the current frame's document, in document order. Nothing
matched is an empty array, not an error.

<ParamField path="sessionId" type="string" required>

The current session's id.

</ParamField>

<ParamField body="using" type="string" required>

The [locator strategy](#locator-strategies).

</ParamField>

<ParamField body="value" type="string" required>

The selector, XPath, link text or tag name.

</ParamField>

<ResponseField name="value" type="object[]" required>

The elements' references, possibly none.

</ResponseField>

## Find element from element

<Badge color="green">POST</Badge> `/session/{sessionId}/element/{elementId}/element`

The first match inside an element. An XPath is evaluated with the element as
its context node, so `.//` looks inside it and `//` in the whole document.

<ParamField path="sessionId" type="string" required>

The current session's id.

</ParamField>

<ParamField path="elementId" type="string" required>

The id of the element to search in.

</ParamField>

<ParamField body="using" type="string" required>

The [locator strategy](#locator-strategies).

</ParamField>

<ParamField body="value" type="string" required>

The selector, XPath, link text or tag name.

</ParamField>

<ResponseField name="value" type="object" required>

The element's reference.

</ResponseField>

**Errors**

As [Find element](#find-element), and `404 no such element` or
`404 stale element reference` when `elementId` is unknown or gone.

## Find elements from element

<Badge color="green">POST</Badge> `/session/{sessionId}/element/{elementId}/elements`

Every match inside an element; an empty array when none.

<ParamField path="sessionId" type="string" required>

The current session's id.

</ParamField>

<ParamField path="elementId" type="string" required>

The id of the element to search in.

</ParamField>

<ParamField body="using" type="string" required>

The [locator strategy](#locator-strategies).

</ParamField>

<ParamField body="value" type="string" required>

The selector, XPath, link text or tag name.

</ParamField>

<ResponseField name="value" type="object[]" required>

The elements' references, possibly none.

</ResponseField>

## Get active element

<Badge color="blue">GET</Badge> `/session/{sessionId}/element/active`

The current frame's focused element (`document.activeElement`), or its body
when nothing has the focus.

<ParamField path="sessionId" type="string" required>

The current session's id.

</ParamField>

<ResponseField name="value" type="object" required>

The element's reference.

</ResponseField>
