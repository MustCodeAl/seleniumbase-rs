---
title: "Scripts"
description: "Run JavaScript, sync and async, in the current frame of the Tauri app's page through the plugin's WebDriver endpoint."
---

Run JavaScript in the app's window. A script runs in the current frame (the
[frame entered](/ecosystem/tauri-api/frames), or the top page): its `window`
and `document`. Paths follow `http://127.0.0.1:4445/{token}`; see the
[overview](/ecosystem/tauri-api/overview) for the reply format and the errors.

<RequestExample>

```bash Execute script
curl -s -X POST "http://127.0.0.1:4445/$TOKEN/session/$SESSION/execute/sync" \
  -d '{"script": "return document.querySelectorAll(arguments[0]).length", "args": [".profile"]}'
```

```bash With an element
curl -s -X POST "http://127.0.0.1:4445/$TOKEN/session/$SESSION/execute/sync" \
  -d '{"script": "return arguments[0].value", "args": [{"element-6066-11e4-a52e-4f735466cecf": "e1-4fzyo82m"}]}'
```

```bash Execute async script
curl -s -X POST "http://127.0.0.1:4445/$TOKEN/session/$SESSION/execute/async" \
  -d '{"script": "const done = arguments[arguments.length - 1]; setTimeout(() => done(document.title), 100)"}'
```

</RequestExample>

<ResponseExample>

```json 200 Execute script
{ "value": 3 }
```

```json 200 Execute async script
{ "value": "Profiles — SeleniumBase Profile Manager" }
```

```json 500 javascript error
{
  "value": {
    "error": "javascript error",
    "message": "Can't find variable: nosuch",
    "stacktrace": ""
  }
}
```

```json 500 script timeout
{
  "value": {
    "error": "script timeout",
    "message": "the page did not answer `script` in time",
    "stacktrace": ""
  }
}
```

</ResponseExample>

<Badge color="green" icon="apple" stroke>Verified on macOS: sync and async, also in a frame</Badge>

The script is the body of a function made with the current frame's
`Function`, called with that frame's `window` as `this` and the arguments as
`arguments`. It may run for as long as the session's
[script timeout](/ecosystem/tauri-api/timeouts) allows (30 seconds by default,
no limit with `null`), then the command answers `500 script timeout`. The
server answers one request at a time, so a long script holds up the requests
behind it.

## Execute script

<Badge color="green">POST</Badge> `/session/{sessionId}/execute/sync`

The value the script returns is the result. A returned promise is awaited
first.

<ParamField path="sessionId" type="string" required>

The current session's id.

</ParamField>

<ParamField body="script" type="string" required>

The function body.

</ParamField>

<ParamField body="args" type="any[]" default="[]">

The arguments. An element reference becomes the element, in arrays and
objects too.

</ParamField>

<ResponseField name="value" type="any" required>

The result, serialized as under [Results](#results).

</ResponseField>

## Execute async script

<Badge color="green">POST</Badge> `/session/{sessionId}/execute/async`

The script gets one more argument, last: a callback. The value it is called
with is the result.

<ParamField path="sessionId" type="string" required>

The current session's id.

</ParamField>

<ParamField body="script" type="string" required>

The function body.

</ParamField>

<ParamField body="args" type="any[]" default="[]">

The arguments, before the callback. An element reference becomes the element,
in arrays and objects too.

</ParamField>

<ResponseField name="value" type="any" required>

The value given to the callback, serialized as under [Results](#results).

</ResponseField>

## Results

A result goes back as JSON:

- an element becomes its [reference](/ecosystem/tauri-api/overview#element-references);
- an array, a `NodeList` or an `HTMLCollection` becomes an array;
- another object is copied key by key (its own enumerable keys);
- `undefined` becomes `null`.

## Errors

| Status | `error` | When |
|---|---|---|
| 400 | `invalid argument` | `script` is missing or not a string (`` `script` is missing ``). |
| 404 | `no such element`, `stale element reference` | An element in `args` is unknown or gone. |
| 404 | `no such frame` | The frame entered is gone or no longer same-origin. |
| 500 | `javascript error` | The script threw, or its promise was rejected; the message is the error's. |
| 500 | `script timeout` | The script did not finish within the script timeout. |
