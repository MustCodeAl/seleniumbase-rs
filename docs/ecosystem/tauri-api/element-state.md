---
title: "Element state"
description: "Read an element's text, tag name, rectangle, enabled, selected and displayed state, attributes, properties and CSS through the Tauri plugin's WebDriver endpoint."
---

Each command reads one thing of an element, in the page, without changing it.
Paths follow `http://127.0.0.1:4445/{token}`; see the
[overview](/ecosystem/tauri-api/overview) for the reply format and the errors.

<RequestExample>

```bash Get element text
curl -s "http://127.0.0.1:4445/$TOKEN/session/$SESSION/element/$ELEMENT/text"
```

```bash Get element rect
curl -s "http://127.0.0.1:4445/$TOKEN/session/$SESSION/element/$ELEMENT/rect"
```

```bash Is element displayed
curl -s "http://127.0.0.1:4445/$TOKEN/session/$SESSION/element/$ELEMENT/displayed"
```

```bash Get element attribute
curl -s "http://127.0.0.1:4445/$TOKEN/session/$SESSION/element/$ELEMENT/attribute/placeholder"
```

```bash Get element CSS value
curl -s "http://127.0.0.1:4445/$TOKEN/session/$SESSION/element/$ELEMENT/css/display"
```

</RequestExample>

<ResponseExample>

```json 200 Get element text
{ "value": "Saved" }
```

```json 200 Get element rect
{
  "value": { "x": 24, "y": 312.5, "width": 120, "height": 32 }
}
```

```json 200 Is element displayed
{ "value": true }
```

```json 200 Get element attribute
{ "value": null }
```

```json 404 stale element reference
{
  "value": {
    "error": "stale element reference",
    "message": "element e1-4fzyo82m is no longer in the page",
    "stacktrace": ""
  }
}
```

</ResponseExample>

Every command here takes the same path parameters and fails the same way:

<ParamField path="sessionId" type="string" required>

The current session's id.

</ParamField>

<ParamField path="elementId" type="string" required>

The element's id, from a [find](/ecosystem/tauri-api/elements) or a script.

</ParamField>

| Status | `error` | When |
|---|---|---|
| 404 | `no such element` | The page does not know the id (`no element ... in this page`), also after the window loaded another document. |
| 404 | `stale element reference` | The element is no longer in the page. |

## Get element text

<Badge color="blue">GET</Badge> `/session/{sessionId}/element/{elementId}/text`

<Badge color="green" icon="apple" stroke>Verified on macOS</Badge>

<ResponseField name="value" type="string" required>

The element's `innerText` (its `textContent` where it has none).

</ResponseField>

## Get element tag name

<Badge color="blue">GET</Badge> `/session/{sessionId}/element/{elementId}/name`

<ResponseField name="value" type="string" required>

The tag name in lower case, such as `button`.

</ResponseField>

## Get element rect

<Badge color="blue">GET</Badge> `/session/{sessionId}/element/{elementId}/rect`

<ResponseField name="value" type="object" required>

The element's box in CSS pixels, from the top left corner of its document (its
frame's, in a frame): `getBoundingClientRect()` plus the scroll offset. The
numbers are not rounded.

<Expandable title="properties">

<ResponseField name="x" type="number" required>

The left edge.

</ResponseField>

<ResponseField name="y" type="number" required>

The top edge.

</ResponseField>

<ResponseField name="width" type="number" required>

The width.

</ResponseField>

<ResponseField name="height" type="number" required>

The height.

</ResponseField>

</Expandable>

</ResponseField>

## Is element enabled

<Badge color="blue">GET</Badge> `/session/{sessionId}/element/{elementId}/enabled`

<Badge color="green" icon="apple" stroke>Verified on macOS</Badge>

<ResponseField name="value" type="boolean" required>

`false` when the element's `disabled` property is true; `true` otherwise, also
for elements that have no such property.

</ResponseField>

## Is element selected

<Badge color="blue">GET</Badge> `/session/{sessionId}/element/{elementId}/selected`

<ResponseField name="value" type="boolean" required>

`true` when the element is `checked` (a checkbox or a radio button) or
`selected` (an `option`).

</ResponseField>

## Is element displayed

<Badge color="blue">GET</Badge> `/session/{sessionId}/element/{elementId}/displayed`

<Badge color="green" icon="apple" stroke>Verified on macOS</Badge>

<ResponseField name="value" type="boolean" required>

`false` when the element's computed style has `display: none`,
`visibility: hidden` or `opacity: 0`, or its box has no width or no height;
`true` otherwise. [Click](/ecosystem/tauri-api/element-interaction#element-click)
uses the same test.

</ResponseField>

## Get element attribute

<Badge color="blue">GET</Badge> `/session/{sessionId}/element/{elementId}/attribute/{name}`

<ParamField path="name" type="string" required>

The attribute's name.

</ParamField>

<ResponseField name="value" type="string | null" required>

`getAttribute(name)`: the attribute's value as written in the HTML, or `null`
when the element does not have it.

</ResponseField>

## Get element property

<Badge color="blue">GET</Badge> `/session/{sessionId}/element/{elementId}/property/{name}`

<ParamField path="name" type="string" required>

The JavaScript property's name, such as `value` or `checked`.

</ParamField>

<ResponseField name="value" type="any" required>

The property's current value, serialized like a
[script's result](/ecosystem/tauri-api/scripts#results): elements become
references, `undefined` becomes `null`.

</ResponseField>

## Get element CSS value

<Badge color="blue">GET</Badge> `/session/{sessionId}/element/{elementId}/css/{name}`

<ParamField path="name" type="string" required>

The CSS property's name, such as `display` or `background-color`.

</ParamField>

<ResponseField name="value" type="string" required>

`getComputedStyle(element).getPropertyValue(name)`; the empty string for an
unknown property.

</ResponseField>
