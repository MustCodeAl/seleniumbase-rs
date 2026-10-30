---
title: "Alerts and MFA"
sidebarTitle: "Alerts and MFA"
description: "Handle JavaScript dialogs and generate TOTP codes."
---

<Note>

Methods are listed as `name(arguments)`, called on a `BaseCase` as `sb.name(...).await?`. The argument names, the selector syntax and the return types are explained in [Reading the API reference](/tutorials/api_reference#reading-this-page).

</Note>

## Alerts

| Method | Description |
|--------|-------------|
| `accept_alert()` | Accepts the current alert. |
| `dismiss_alert()` | Dismisses the current alert. |
| `switch_to_alert()` | Waits for an alert, confirm box or prompt and returns an `Alert` with `text()`, `accept()`, `dismiss()` and `send_keys(text)`. Builder method: `.timeout(duration)`. |
| `wait_for_and_switch_to_alert(timeout_secs)` | The same with a timeout in seconds. |
| `is_alert_present()` | Returns true if an alert is present. |
| `get_alert_text()` | Returns the alert text. |
| `type_alert_text(text)` | Types into a prompt alert. |

## MFA / TOTP

| Method | Description |
|--------|-------------|
| `get_totp_code(secret)` | Generates a TOTP code from a base32 secret. |
| `get_mfa_code(secret)` | Alias for `get_totp_code`. |
| `enter_mfa_code(selector, secret)` | Generates a TOTP code, types it into an input and presses Enter. |
