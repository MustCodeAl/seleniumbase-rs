---
title: "Actions"
description: "W3C actions on the Tauri plugin's WebDriver endpoint: one keyboard, mouse pointers and pauses, simulated with DOM events in the page."
---

Actions are simulated in the page with DOM events at the pointer's place in
the current frame, not OS-level input: the mouse and the keyboard are never
used. Paths follow `http://127.0.0.1:4445/{token}`; see the
[overview](/ecosystem/tauri-api/overview) for the reply format and the errors.

<RequestExample>

```bash Double click
curl -s -X POST "http://127.0.0.1:4445/$TOKEN/session/$SESSION/actions" -d '{
  "actions": [{
    "id": "mouse", "type": "pointer", "parameters": {"pointerType": "mouse"},
    "actions": [
      {"type": "pointerMove", "origin": {"element-6066-11e4-a52e-4f735466cecf": "e1-4fzyo82m"}, "x": 0, "y": 0},
      {"type": "pointerDown", "button": 0}, {"type": "pointerUp", "button": 0},
      {"type": "pointerDown", "button": 0}, {"type": "pointerUp", "button": 0}
    ]
  }]
}'
```

```bash Press Enter
curl -s -X POST "http://127.0.0.1:4445/$TOKEN/session/$SESSION/actions" -d '{
  "actions": [{
    "id": "keyboard", "type": "key",
    "actions": [{"type": "keyDown", "value": "\uE007"}, {"type": "keyUp", "value": "\uE007"}]
  }]
}'
```

```bash Release actions
curl -s -X DELETE "http://127.0.0.1:4445/$TOKEN/session/$SESSION/actions"
```

</RequestExample>

<ResponseExample>

```json 200 Perform actions
{ "value": null }
```

```json 500 unsupported operation
{
  "value": {
    "error": "unsupported operation",
    "message": "wheel input (scrolling) is not simulated",
    "stacktrace": ""
  }
}
```

```json 500 move target out of bounds
{
  "value": {
    "error": "move target out of bounds",
    "message": "(1200, 40) is outside the viewport, 900 x 672",
    "stacktrace": ""
  }
}
```

```json 400 invalid argument
{
  "value": {
    "error": "invalid argument",
    "message": "`button` must be a mouse button from 0 to 4",
    "stacktrace": ""
  }
}
```

</ResponseExample>

## Perform actions

<Badge color="green">POST</Badge> `/session/{sessionId}/actions`

<Badge color="green" icon="apple" stroke>Verified on macOS: double click, hover, Space, Enter</Badge> <Badge color="orange" icon="flask" stroke>Not run on a real app yet: right click, drag, Backspace</Badge>

Runs W3C actions <Tooltip tip="A tick is the i-th action of every input source, run together.">tick by tick</Tooltip>:
one keyboard (`keyDown`, `keyUp`), mouse pointers (`pointerMove` from the
viewport, the pointer or an element, `pointerDown`, `pointerUp` with buttons 0
to 4) and pauses. The whole body is checked before anything runs, so a body
with one wrong or unsupported action runs nothing. Each tick's input is
dispatched at once; the tick then lasts as long as its longest pause or
pointer move (a move does not glide: the pointer jumps, then the time passes).
Other actions may carry a `duration`, which is ignored.

Right clicks, drags and Backspace are checked only by the tests of
`src/bridge.js` in a fake page under Node.js (`tests/bridge.test.mjs`), not in a
webview.

<ParamField path="sessionId" type="string" required>

The current session's id.

</ParamField>

<ParamField body="actions" type="object[]" required>

The input sources.

<Expandable title="input source">

<ParamField body="id" type="string" required>

The source's name. Each pointer source is a mouse of its own, with its own
place and buttons.

</ParamField>

<ParamField body="type" type="string" required>

`none` (pauses only), `key` or `pointer`. `wheel` answers
`500 unsupported operation`; any other type `400 invalid argument`.

</ParamField>

<ParamField body="parameters.pointerType" type="string" default="mouse">

For a `pointer` source: `mouse`. `pen` and `touch` answer
`500 unsupported operation`.

</ParamField>

<ParamField body="actions" type="object[]" required>

The source's actions, one per tick.

<Expandable title="action">

<ParamField body="type" type="string" required>

`pause` for any source; `keyDown` or `keyUp` for a `key` source;
`pointerMove`, `pointerDown` or `pointerUp` for a `pointer` source.
`pointerCancel` answers `500 unsupported operation`; any other `400 invalid
argument`.

</ParamField>

<ParamField body="duration" type="integer">

For `pause` and `pointerMove`: how long the tick lasts at least, in
milliseconds, from 0 to 2^53 - 1.

</ParamField>

<ParamField body="value" type="string">

For `keyDown` and `keyUp`, required: a key. A WebDriver key code, such as
`\uE007`, names a key ([Keys](#keys)); any other value is the text it types.

</ParamField>

<ParamField body="button" type="integer">

For `pointerDown` and `pointerUp`, required: 0 left, 1 middle, 2 right, 3 back,
4 forward.

</ParamField>

<ParamField body="origin" type="string | object" default="viewport">

For `pointerMove`: `viewport` (from its top left corner), `pointer` (from where
the pointer is) or an element reference (from the centre of the element, which
must be in the current frame).

</ParamField>

<ParamField body="x" type="number" default="0">

For `pointerMove`: the offset from the origin, in CSS pixels.

</ParamField>

<ParamField body="y" type="number" default="0">

For `pointerMove`: the offset from the origin, in CSS pixels.

</ParamField>

</Expandable>

</ParamField>

</Expandable>

</ParamField>

<ResponseField name="value" type="null" required>

`null`, once the last tick is over.

</ResponseField>

### What the page sees

<AccordionGroup>

<Accordion title="Pointer moves" icon="arrow-pointer" defaultOpen>

A move onto an element out of view scrolls it into view first, at once even
when the page sets `scroll-behavior: smooth` (as clicks and element
screenshots do). A move that ends outside the viewport answers
`500 move target out of bounds`. When the element under the pointer changes,
the old one gets `pointerout`, `pointerleave`, `mouseout` and `mouseleave`, the
new one `pointerover`, `pointerenter`, `mouseover` and `mouseenter`; then the
element under the pointer gets `pointermove` and `mousemove`.

</Accordion>

<Accordion title="Buttons" icon="computer-mouse">

A press sends `pointerdown` and `mousedown`, and focuses what it lands on (or
what holds that). A release sends `pointerup` and `mouseup`, then `click` with
its count for the left button, and `dblclick` for two presses on the same
element within 500 ms; the other buttons get `auxclick`, and the right one
`contextmenu` too. A cancelled `pointerdown` holds back the `mousedown` and the
focus.

</Accordion>

<Accordion title="Keys" icon="keyboard">

Key events go to the focused element of the current frame, with `shiftKey`,
`ctrlKey`, `altKey` and `metaKey` set while those keys are held. Printable keys
type (through `execCommand`, so the page sees `beforeinput` and `input`), not
while Control or Meta is held; Backspace and Delete delete; Enter submits a
form field's form, adds a line to a `textarea` or a paragraph to an editable
element, and clicks a button or a link. A cancelled `keydown` types nothing.

</Accordion>

<Accordion title="What it cannot do" icon="triangle-exclamation">

Because the events are not the OS's, CSS `:hover` does not apply and HTML drag
and drop (`draggable`) does not start. Wheel input, pen and touch pointers and
`pointerCancel` answer `unsupported operation`.

</Accordion>

</AccordionGroup>

### Keys

A WebDriver key code is turned into the key name W3C gives it; a few:

| Code | Key | Code | Key |
|---|---|---|---|
| `\uE003` | Backspace | `\uE00C` | Escape |
| `\uE004` | Tab | `\uE00D` | Space (`" "`) |
| `\uE006`, `\uE007` | Enter | `\uE017` | Delete |
| `\uE008` | Shift | `\uE03D` | Meta |

The arrows, Home, End, Page Up and Down, Insert, the digits of the number pad,
F1 to F12 and the right-hand modifier codes are mapped the same way.

**Errors**

| Status | `error` | When |
|---|---|---|
| 400 | `invalid argument` | `actions` is not an array; a source without `id`, `type` or `actions`; an unknown source or action type; a key action without a key; a button outside 0 to 4; a wrong `origin`, `x`, `y` or `duration`. |
| 404 | `no such element`, `stale element reference` | A move's origin element is unknown, gone, or not in the current frame. |
| 404 | `no such frame` | The frame entered is gone or no longer same-origin. |
| 500 | `unsupported operation` | Wheel input, a pen or touch pointer, `pointerCancel`. |
| 500 | `move target out of bounds` | A move that ends outside the viewport. |

## Release actions

<Badge color="red">DELETE</Badge> `/session/{sessionId}/actions`

<Badge color="orange" icon="flask" stroke>Not run on a real app yet</Badge>

Lets go of the keys and buttons still held, the last pressed first, as a
`keyUp` or a `pointerUp` would (with the events those send), then forgets the
input state (the pointers' places too). If the frame entered is gone, it only forgets.

<ParamField path="sessionId" type="string" required>

The current session's id.

</ParamField>

<ResponseField name="value" type="null" required>

`null`.

</ResponseField>
