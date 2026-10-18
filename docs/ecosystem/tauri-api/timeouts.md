---
title: "Timeouts"
description: "Read and set the session timeouts of the Tauri plugin's WebDriver endpoint: script, pageLoad and implicit."
---

A session's timeouts, in milliseconds. Paths follow
`http://127.0.0.1:4445/{token}`; see the [overview](/ecosystem/tauri-api/overview)
for the reply format and the errors.

<RequestExample>

```bash Get timeouts
curl -s "http://127.0.0.1:4445/$TOKEN/session/$SESSION/timeouts"
```

```bash Set timeouts
curl -s -X POST "http://127.0.0.1:4445/$TOKEN/session/$SESSION/timeouts" \
  -d '{"script": null, "pageLoad": 7000, "implicit": 250}'
```

</RequestExample>

<ResponseExample>

```json 200 Get timeouts
{
  "value": { "script": 30000, "pageLoad": 60000, "implicit": 0 }
}
```

```json 200 Set timeouts
{ "value": null }
```

```json 400 invalid argument
{
  "value": {
    "error": "invalid argument",
    "message": "`implicit` must be a whole number of milliseconds from 0 to 9007199254740991",
    "stacktrace": ""
  }
}
```

</ResponseExample>

| Timeout | Default | What it limits |
|---|---|---|
| `script` | `30000` | How long [a script](/ecosystem/tauri-api/scripts) may run before it answers `500 script timeout`. `null` sets no limit. |
| `pageLoad` | `60000` | How long [navigation](/ecosystem/tauri-api/navigation) (`POST /url`, `/back`, `/forward`, `/refresh`) waits for the new page before it answers `500 timeout`. |
| `implicit` | `0` | How long [finding elements](/ecosystem/tauri-api/elements) looks again, every 50 ms, while nothing matches. |

A new session starts with the defaults. A new session always waits up to 60
seconds for the window's page, whatever `pageLoad` the last session set. Other
page operations have a fixed 30 seconds.

## Get timeouts

<Badge color="blue">GET</Badge> `/session/{sessionId}/timeouts`

**Path**

<ParamField path="sessionId" type="string" required>

The current session's id.

</ParamField>

**Response**

<ResponseField name="value" type="object" required>

<Expandable title="properties" defaultOpen>

<ResponseField name="script" type="integer | null" required>

Milliseconds, or `null` for no limit.

</ResponseField>

<ResponseField name="pageLoad" type="integer" required>

Milliseconds.

</ResponseField>

<ResponseField name="implicit" type="integer" required>

Milliseconds.

</ResponseField>

</Expandable>

</ResponseField>

## Set timeouts

<Badge color="green">POST</Badge> `/session/{sessionId}/timeouts`

Sets the timeouts the body names. A key left out keeps its value, and other
keys are ignored. A body with one wrong value changes nothing.

**Path**

<ParamField path="sessionId" type="string" required>

The current session's id.

</ParamField>

**Body**

<ParamField body="script" type="integer | null">

A whole number of milliseconds from 0 to 2^53 - 1 (9007199254740991), or
`null` for no limit.

</ParamField>

<ParamField body="pageLoad" type="integer">

A whole number of milliseconds from 0 to 2^53 - 1. Not `null`.

</ParamField>

<ParamField body="implicit" type="integer">

A whole number of milliseconds from 0 to 2^53 - 1. Not `null`.

</ParamField>

**Response**

<ResponseField name="value" type="null" required>

Always `null`.

</ResponseField>

**Errors**

| Status | `error` | When |
|---|---|---|
| 400 | `invalid argument` | A value that is negative, a fraction, too large, a string, or `null` for `pageLoad` or `implicit`. |
