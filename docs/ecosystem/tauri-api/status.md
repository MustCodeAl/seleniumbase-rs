---
title: "Status"
description: "GET /status of the Tauri plugin's WebDriver endpoint: whether the endpoint answers, before any session."
---

Whether the endpoint is up. Paths follow `http://127.0.0.1:4445/{token}`; see
the [overview](/ecosystem/tauri-api/overview) for the token, the reply format
and the errors.

<RequestExample>

```bash Status
curl -s "http://127.0.0.1:4445/$TOKEN/status"
```

```bash Wait for the endpoint
until curl -sf "http://127.0.0.1:4445/$TOKEN/status" > /dev/null; do sleep 0.2; done
```

</RequestExample>

<ResponseExample>

```json 200 Status
{
  "value": {
    "ready": true,
    "message": "tauri-plugin-sbrs-automation is ready"
  }
}
```

```json 404 Wrong token
{
  "value": {
    "error": "unknown command",
    "message": "the URL does not start with the token",
    "stacktrace": ""
  }
}
```

</ResponseExample>

## Status

<Badge color="blue">GET</Badge> `/status`

The server answers this one itself, without the page and without a session, as
soon as it listens: also before the app's window has loaded its page. That is
the moment to start a session, which then waits for the page (see
[New session](/ecosystem/tauri-api/sessions#new-session)). The end-to-end test
of the `tauri-profile-manager` example waits for this answer before it starts
`BaseCase`.

**Response**

<ResponseField name="value" type="object" required>

<Expandable title="properties" defaultOpen>

<ResponseField name="ready" type="boolean" required>

Always `true`.

</ResponseField>

<ResponseField name="message" type="string" required>

`tauri-plugin-sbrs-automation is ready`.

</ResponseField>

</Expandable>

</ResponseField>

**Errors**

CDP configuration is reported separately by [GET /cdp](/ecosystem/tauri-api/cdp).
This status response does not indicate whether a Chromium debugger is ready.

| Status | `error` | When |
|---|---|---|
| 404 | `unknown command` | The path does not start with the token. |
| 403 | `unknown error` | The request has an `Origin` header. |

<Tip>

No answer at all (connection refused) means the endpoint is off or not
listening: the plugin is off in release builds unless `SBRS_AUTOMATION` turns
it on, and an app whose port is taken runs without the endpoint and says so on
stderr.

</Tip>
