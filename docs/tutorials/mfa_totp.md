---
title: "MFA / TOTP Guide"
sidebarTitle: "MFA and TOTP"
---

Multi-factor authentication (MFA) flows that use time-based one-time passwords
(<Tooltip tip="Time-based one-time password: a short code worked out from a shared secret and the current time.">TOTP</Tooltip>) can be automated with `seleniumbase-rs`. The crate generates codes locally
from a base32-encoded shared secret, so you can complete MFA login steps without
manual intervention.

<Info>

**What you will learn**

- How to generate a TOTP code.
- How to use the code in a login flow.
- Security best practices for handling secrets.

</Info>

<Card title="Quick reference" icon="key">

`get_totp_code(secret)` returns the code. Its aliases are `get_mfa_code` and `get_google_auth_code`.

`enter_totp_code(css, secret)` types the code into a field and presses Enter. Its alias is `enter_mfa_code`.

RFC 6238 with HMAC-SHA1, a 30 second step and 6 digits.

</Card>

<Visibility for="agents">

Summary for AI exports: `sb.get_totp_code("BASE32SECRET")?` is a plain method, not an `async` one; it reads the clock and returns a 6 digit code (RFC 6238, HMAC-SHA1, 30 second step) with no browser call. `sb.enter_totp_code(css, secret).await?` types the code into the field and presses Enter. `get_mfa_code` and `get_google_auth_code` are aliases of `get_totp_code`, and `enter_mfa_code` of `enter_totp_code`. Load secrets from environment variables or a secrets manager, never from source code.

</Visibility>

## Generate a TOTP code

`get_totp_code` is a plain method, not an `async` one: it reads the clock and
computes the code, with no browser call.

```rust,no_run
# use seleniumbase_rs::BaseCase;
# fn demo(sb: &BaseCase) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
let code = sb.get_totp_code("JBSWY3DPEHPK3PXP")?;
println!("TOTP code: {code}");
# Ok(())
# }
```

The secret is the base32-encoded shared secret provided by the service when
enabling MFA. It is often shown as a QR code or a plain string during enrollment.
Spaces and dashes in the secret are ignored, and case does not matter.
`get_mfa_code` and `get_google_auth_code` are aliases of `get_totp_code`.

## Use in a login flow

<Tabs>

<Tab title="Step by step" icon="list-ol">

```rust,no_run
# use seleniumbase_rs::BaseCase;
# async fn demo(sb: &mut BaseCase) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
sb.type_text("#username", "demo_user").await?;
sb.type_text("#password", "secret_pass").await?;
sb.click("#login").await?;

let code = sb.get_totp_code("JBSWY3DPEHPK3PXP")?;
sb.type_text("#otp", &code).await?;
sb.click("#verify-otp").await?;
# Ok(())
# }
```

</Tab>

<Tab title="One call" icon="bolt">

`enter_totp_code(css, secret).await?` (alias `enter_mfa_code`) generates the code,
types it into the field and presses Enter, as Python's `enter_mfa_code` does, in
one call:

```rust,no_run
# use seleniumbase_rs::BaseCase;
# async fn demo(sb: &mut BaseCase) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
sb.enter_totp_code("#otp", "JBSWY3DPEHPK3PXP").await?;
# Ok(())
# }
```

</Tab>

</Tabs>

## Google Authenticator compatibility

Codes are compatible with Google Authenticator, Authy, Microsoft Authenticator,
and other standard TOTP apps (RFC 6238 with HMAC-SHA1). The time step is 30
seconds and the output length is 6 digits.

<Warning>

Neither can be changed, so a service that uses another length, step or hash will
not accept these codes.

</Warning>

## Security notes

<Warning>

- Keep MFA secrets out of source code; load them from environment variables or a
  secrets manager. If you keep one in a file, `sbrs encrypt` and `sbrs decrypt`
  (AES-256-GCM, passphrase from `SB_ENCRYPTION_KEY`) can protect it at rest.
- Treat TOTP secrets with the same care as passwords. Rotate them if exposed.

</Warning>

<Note>

Codes are generated locally; no network request is required.

</Note>

## Troubleshooting

<AccordionGroup>

<Accordion title="Code rejected" icon="clock">

**Likely cause:** System clock skew.

**Fix:** Synchronize the test machine's clock with NTP.

</Accordion>

<Accordion title="invalid base32 character error" icon="key">

**Likely cause:** The secret is not base32 (letters `A`-`Z` and digits `2`-`7`).

**Fix:** Re-copy the secret from the enrollment screen.

</Accordion>

<Accordion title="Code expires before use" icon="hourglass-half">

**Likely cause:** Generated too early.

**Fix:** Generate the code immediately before typing it.

</Accordion>

</AccordionGroup>
