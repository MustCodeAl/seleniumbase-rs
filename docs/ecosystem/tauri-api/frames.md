---
title: "Frames"
description: "Enter same-origin frames of the Tauri app's page and leave them: POST /frame and POST /frame/parent."
---

Only a frame of the same origin as the app's page can be entered (an
`<iframe srcdoc>` or one of the app's own pages): the plugin works through page
script, which cannot reach into another origin, so a cross-origin or sandboxed
frame answers `404 no such frame`. Paths follow
`http://127.0.0.1:4445/{token}`; see the
[overview](/ecosystem/tauri-api/overview) for the reply format and the errors.

<RequestExample>

```bash Enter a frame element
FRAME=$(curl -s -X POST "http://127.0.0.1:4445/$TOKEN/session/$SESSION/element" \
  -d '{"using": "css selector", "value": "iframe"}' | jq -c .value)
curl -s -X POST "http://127.0.0.1:4445/$TOKEN/session/$SESSION/frame" \
  -d "{\"id\": $FRAME}"
```

```bash Enter frame 0
curl -s -X POST "http://127.0.0.1:4445/$TOKEN/session/$SESSION/frame" -d '{"id": 0}'
```

```bash Back to the top page
curl -s -X POST "http://127.0.0.1:4445/$TOKEN/session/$SESSION/frame" -d '{"id": null}'
```

```bash Parent frame
curl -s -X POST "http://127.0.0.1:4445/$TOKEN/session/$SESSION/frame/parent" -d '{}'
```

</RequestExample>

<ResponseExample>

```json 200 Switch to frame
{ "value": null }
```

```json 404 no such frame
{
  "value": {
    "error": "no such frame",
    "message": "the frame is cross-origin: only a frame of the same origin as the app's page can be entered, since the page script cannot reach into another origin",
    "stacktrace": ""
  }
}
```

```json 400 invalid argument
{
  "value": {
    "error": "invalid argument",
    "message": "`id` must be null, a frame index or a frame element",
    "stacktrace": ""
  }
}
```

</ResponseExample>

Once a frame is entered, these act in it: [finding](/ecosystem/tauri-api/elements)
(from the document), the active element, the
[page source](/ecosystem/tauri-api/navigation#get-page-source),
[scripts](/ecosystem/tauri-api/scripts) and [actions](/ecosystem/tauri-api/actions).
The current URL and the title stay the top page's, and an element command acts
on its element wherever it is. [Navigating](/ecosystem/tauri-api/navigation) and
a [new session](/ecosystem/tauri-api/sessions#new-session) return to the top
page.

A frame entered that is then removed, or that loads a page of another origin,
answers `404 no such frame` (`the frame entered is gone or is no longer
same-origin; switch to its parent or the top page`) for each command that acts
in it.

<Info>

The plugin replaces `alert`, `confirm` and `prompt` in a frame it enters, and
in every same-origin frame when the next command runs after the frame has
loaded; a dialog the frame opens before that is the real one. See
[User prompts](/ecosystem/tauri-api/alerts).

</Info>

## Switch to frame

<Badge color="green">POST</Badge> `/session/{sessionId}/frame`

<Badge color="green" icon="apple" stroke>Verified on macOS: a frame element</Badge> <Badge color="orange" icon="flask" stroke>Not run on a real app yet: an index</Badge>

Enters a frame of the current frame (or of the top page), or goes back to the
top page. On macOS, an `<iframe srcdoc>` was entered by its element (finding, a
script, a click and its alert inside), and a sandboxed frame was refused.

<ParamField path="sessionId" type="string" required>

The current session's id.

</ParamField>

<ParamField body="id" type="null | integer | object" required>

One of:

- `null`: the top page;
- an index, 0 to 65535, into the current page's frames (`window.frames`),
  which must be an `iframe` or `frame` element of the current document;
- a frame element's [reference](/ecosystem/tauri-api/overview#element-references):
  an `iframe` or `frame` element in the current frame's document.

</ParamField>

<ResponseField name="value" type="null" required>

`null`.

</ResponseField>

**Errors**

| Status | `error` | When |
|---|---|---|
| 400 | `invalid argument` | `id` is missing, negative, above 65535, a fraction, a string, or an object that is not an element reference. |
| 404 | `no such frame` | The frame is cross-origin or sandboxed; the element is not a frame or not in the current frame; there is no frame at the index. |
| 404 | `no such element`, `stale element reference` | The frame element's id is unknown, or the element is gone. |

## Switch to parent frame

<Badge color="green">POST</Badge> `/session/{sessionId}/frame/parent`

<Badge color="orange" icon="flask" stroke>Not run on a real app yet</Badge>

Leaves the frame entered last for its parent. In the top page it does nothing.

<ParamField path="sessionId" type="string" required>

The current session's id.

</ParamField>

<ResponseField name="value" type="null" required>

`null`.

</ResponseField>
