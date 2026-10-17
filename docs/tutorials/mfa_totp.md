---
title: "MFA / TOTP Guide"
---

# MFA / TOTP Guide

Multi-factor authentication (MFA) flows that use time-based one-time passwords
(TOTP) can be automated with `seleniumbase-rs`. The crate generates codes locally
from a base32-encoded shared secret, so you can complete MFA login steps without
manual intervention.

## What you will learn

- How to generate a TOTP code.
- How to use the code in a login flow.
- Security best practices for handling secrets.

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

`enter_totp_code(css, secret).await?` (alias `enter_mfa_code`) generates the code
and types it into the field in one call:

```rust,no_run
# use seleniumbase_rs::BaseCase;
# async fn demo(sb: &mut BaseCase) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
sb.enter_totp_code("#otp", "JBSWY3DPEHPK3PXP").await?;
# Ok(())
# }
```

## Google Authenticator compatibility

Codes are compatible with Google Authenticator, Authy, Microsoft Authenticator,
and other standard TOTP apps (RFC 6238 with HMAC-SHA1). The time step is 30
seconds and the output length is 6 digits.

<Info>

Neither can be changed, so a service that uses another length, step or hash will
not accept these codes.

</Info>

## Security notes

<Warning>

- Keep MFA secrets out of source code; load them from environment variables or a
  secrets manager. If you keep one in a file, `sbrs encrypt` and `sbrs decrypt`
  (AES-256-GCM, passphrase from `SB_ENCRYPTION_KEY`) can protect it at rest.
- Codes are generated locally; no network request is required.
- Treat TOTP secrets with the same care as passwords. Rotate them if exposed.

</Warning>

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| Code rejected | System clock skew | Synchronize the test machine's clock with NTP. |
| `invalid base32 character` error | The secret is not base32 (letters `A`-`Z` and digits `2`-`7`) | Re-copy the secret from the enrollment screen. |
| Code expires before use | Generated too early | Generate the code immediately before typing it. |
