---
title: "Sessions"
description: "Start and end the WebDriver session of the Tauri plugin: POST /session and DELETE /session/{sessionId}."
---

The endpoint holds one session at a time. Paths follow
`http://127.0.0.1:4445/{token}`; see the [overview](/ecosystem/tauri-api/overview)
for the reply format and the errors.

<RequestExample>

```bash New session
curl -s -X POST "http://127.0.0.1:4445/$TOKEN/session" \
  -d '{"capabilities": {"alwaysMatch": {}}}'
```

```bash Keep the session id
SESSION=$(curl -s -X POST "http://127.0.0.1:4445/$TOKEN/session" -d '{}' | jq -r .value.sessionId)
```

```bash Delete session
curl -s -X DELETE "http://127.0.0.1:4445/$TOKEN/session/$SESSION"
```

</RequestExample>

<ResponseExample>

```json 200 New session
{
  "value": {
    "sessionId": "sbrs-automation-1",
    "capabilities": {
      "browserName": "tauri",
      "platformName": "macos",
      "acceptInsecureCerts": false,
      "setWindowRect": true,
      "timeouts": { "script": 30000, "pageLoad": 60000, "implicit": 0 }
    }
  }
}
```

```json 200 Delete session
{ "value": null }
```

```json 404 invalid session id
{
  "value": {
    "error": "invalid session id",
    "message": "no session `sbrs-automation-1`",
    "stacktrace": ""
  }
}
```

```json 500 session not created
{
  "value": {
    "error": "session not created",
    "message": "the app's window did not load its page in time",
    "stacktrace": ""
  }
}
```

</ResponseExample>

## New session

<Badge color="green">POST</Badge> `/session`

<Badge color="green" icon="apple" stroke>Verified on macOS</Badge>

Starts a session once the app's window has loaded its page. The plugin serves
before the app has opened its window, so the command asks the page for its
`document.readyState` every 50 ms until it is `complete`, for up to 60 seconds.
Then it resets the page script: a session starts in the top page, with no
dialog recorded, no dialog answer set and no key or button held from the last
session. The [timeouts](/ecosystem/tauri-api/timeouts) go back to their
defaults.

A new session replaces the current one: one session at a time, and the old id
then answers `404 invalid session id`.

**Body**

<ParamField body="capabilities" type="object">

Read and ignored. The body must be JSON (or empty), but no capability is
matched: the answer always carries the capabilities below.

</ParamField>

**Response**

<ResponseField name="value" type="object" required>

<Expandable title="properties" defaultOpen>

<ResponseField name="sessionId" type="string" required>

`sbrs-automation-` and the number of the session since the app started,
from 1.

</ResponseField>

<ResponseField name="capabilities" type="object" required>

<Expandable title="properties">

<ResponseField name="browserName" type="string" required>

`tauri`.

</ResponseField>

<ResponseField name="platformName" type="string" required>

The system the app runs on, as Rust names it: `macos`, `windows` or `linux`.

</ResponseField>

<ResponseField name="acceptInsecureCerts" type="boolean" required>

`false`.

</ResponseField>

<ResponseField name="setWindowRect" type="boolean" required>

`true`: the window can be moved and resized (see
[Set window rect](/ecosystem/tauri-api/window#set-window-rect)).

</ResponseField>

<ResponseField name="timeouts" type="object" required>

The session's timeouts in milliseconds, as
[Get timeouts](/ecosystem/tauri-api/timeouts#get-timeouts) reports them.

<Expandable title="properties">

<ResponseField name="script" type="integer" required>

`30000`.

</ResponseField>

<ResponseField name="pageLoad" type="integer" required>

`60000`.

</ResponseField>

<ResponseField name="implicit" type="integer" required>

`0`.

</ResponseField>

</Expandable>

</ResponseField>

</Expandable>

</ResponseField>

</Expandable>

</ResponseField>

**Errors**

| Status | `error` | When |
|---|---|---|
| 500 | `session not created` | The window did not load its page in 60 seconds (`the app's window did not load its page in time`), or the page could not be reset (`the page could not be reset: ...`). |

## Delete session

<Badge color="red">DELETE</Badge> `/session/{sessionId}`

<Badge color="green" icon="apple" stroke>Verified on macOS</Badge>

Ends the session: the endpoint forgets its id. The app and its window stay
open, and the page is not reset until the next new session.

**Path**

<ParamField path="sessionId" type="string" required>

The id that [New session](#new-session) returned.

</ParamField>

**Response**

<ResponseField name="value" type="null" required>

Always `null`.

</ResponseField>

**Errors**

| Status | `error` | When |
|---|---|---|
| 404 | `invalid session id` | No session is open, or the id is not the current one (`` no session `...` ``). |

<Note>

Every route under `/session/{sessionId}` checks the id first and answers
`404 invalid session id` for any other.

</Note>
