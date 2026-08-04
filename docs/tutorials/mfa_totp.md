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

```rust
let code = sb.get_totp_code("JBSWY3DPEHPK3PXP").await?;
println!("TOTP code: {code}");
```

The secret is the base32-encoded shared secret provided by the service when
enabling MFA. It is often shown as a QR code or a plain string during enrollment.

## Use in a login flow

```rust
sb.type_text("#username", "demo_user").await?;
sb.type_text("#password", "secret_pass").await?;
sb.click("#login").await?;

let code = sb.get_totp_code("JBSWY3DPEHPK3PXP").await?;
sb.type_text("#otp", &code).await?;
sb.click("#verify-otp").await?;
```

## Google Authenticator compatibility

Codes are compatible with Google Authenticator, Authy, Microsoft Authenticator,
and other standard TOTP apps. The default time step is 30 seconds and the output
length is 6 digits.

## Security notes

- Keep MFA secrets out of source code; load them from environment variables or a
  secrets manager.
- Codes are generated locally; no network request is required.
- Treat TOTP secrets with the same care as passwords. Rotate them if exposed.

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| Code rejected | System clock skew | Synchronize the test machine's clock with NTP. |
| Invalid secret format | Secret is not base32 | Re-copy the secret without spaces or convert it. |
| Code expires before use | Generated too early | Generate the code immediately before typing it. |
