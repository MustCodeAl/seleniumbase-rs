---
title: "Element interaction"
description: "Click, clear and type into elements of the Tauri app's page through the plugin's WebDriver endpoint, simulated in the page."
---

Clicks and typing are simulated in the page with page script and DOM events,
not OS-level input: the mouse and the keyboard are never used. For key-by-key
input with modifiers, use [actions](/ecosystem/tauri-api/actions). Paths follow
`http://127.0.0.1:4445/{token}`; see the
[overview](/ecosystem/tauri-api/overview) for the reply format and the errors.

<RequestExample>

```bash Element click
curl -s -X POST "http://127.0.0.1:4445/$TOKEN/session/$SESSION/element/$ELEMENT/click" -d '{}'
```

```bash Element clear
curl -s -X POST "http://127.0.0.1:4445/$TOKEN/session/$SESSION/element/$ELEMENT/clear" -d '{}'
```

```bash Element send keys
curl -s -X POST "http://127.0.0.1:4445/$TOKEN/session/$SESSION/element/$ELEMENT/value" \
  -d '{"text": "Work"}'
```

```bash Type and press Enter
curl -s -X POST "http://127.0.0.1:4445/$TOKEN/session/$SESSION/element/$ELEMENT/value" \
  -d '{"text": "Work"}'
```

</RequestExample>

<ResponseExample>

```json 200 Element click
{ "value": null }
```

```json 400 element not interactable
{
  "value": {
    "error": "element not interactable",
    "message": "the element is not visible",
    "stacktrace": ""
  }
}
```

```json 400 invalid argument
{
  "value": {
    "error": "invalid argument",
    "message": "`text` is missing",
    "stacktrace": ""
  }
}
```

</ResponseExample>

Every command here takes the same path parameters, answers `null`, and answers
`404 no such element` or `404 stale element reference` for an element the page
does not know or that is gone:

<ParamField path="sessionId" type="string" required>

The current session's id.

</ParamField>

<ParamField path="elementId" type="string" required>

The element's id, from a [find](/ecosystem/tauri-api/elements) or a script.

</ParamField>

## Element click

<Badge color="green">POST</Badge> `/session/{sessionId}/element/{elementId}/click`

<Badge color="green" icon="apple" stroke>Verified on macOS</Badge>

Scrolls the element to the middle of the view at once (also when the page sets
`scroll-behavior: smooth`), checks that it is
[displayed](/ecosystem/tauri-api/element-state#is-element-displayed), focuses
it, and calls its `click()`. The page sees a `click` event, without pointer or
mouse down and up events before it (use [actions](/ecosystem/tauri-api/actions)
for those). Whether another element covers it is not checked.

<ResponseField name="value" type="null" required>

`null`.

</ResponseField>

**Errors**

| Status | `error` | When |
|---|---|---|
| 400 | `element not interactable` | The element is not displayed (`the element is not visible`). |

## Element clear

<Badge color="green">POST</Badge> `/session/{sessionId}/element/{elementId}/clear`

<Badge color="green" icon="apple" stroke>Verified on macOS</Badge>

Sets the element's value to the empty string (its text, for an editable
element) through the native value setter, which React and similar frameworks
watch, then fires `input` and `change`.

<ResponseField name="value" type="null" required>

`null`.

</ResponseField>

## Element send keys

<Badge color="green">POST</Badge> `/session/{sessionId}/element/{elementId}/value`

<Badge color="green" icon="apple" stroke>Verified on macOS</Badge>

Focuses the element and adds the text at the end of its value (its text, for
an editable element) through the native value setter, then fires `input` and
`change` once. The page sees no key events for the text.

The WebDriver Enter key, ``, is taken out of the text: when the text
holds one, the element then gets `keydown`, `keypress` and `keyup` for Enter
and its form is submitted (`requestSubmit`). Other WebDriver key codes are not
read here; they are added as characters.

<ParamField body="text" type="string">

The text to type. Required unless `value` is given.

</ParamField>

<ParamField body="value" type="string[]">

The older form: the text as an array of strings, joined. Read only when
`text` is missing.

</ParamField>

<ResponseField name="value" type="null" required>

`null`.

</ResponseField>

**Errors**

| Status | `error` | When |
|---|---|---|
| 400 | `invalid argument` | Neither `text` nor a `value` array (`` `text` is missing ``). |
