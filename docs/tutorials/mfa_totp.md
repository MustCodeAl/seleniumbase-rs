# MFA / TOTP Guide

SeleniumBase for Rust can generate time-based one-time passwords (TOTP) for multi-factor authentication flows.

## Generate a TOTP code

```rust
let code = sb.get_totp_code("JBSWY3DPEHPK3PXP").await?;
println!("TOTP code: {code}");
```

The secret is the base32-encoded shared secret provided by the service when enabling MFA.

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

Codes are compatible with Google Authenticator, Authy, and other standard TOTP apps. The default time step is 30 seconds.

## Notes

- Keep MFA secrets out of source code; load them from environment variables or a secrets manager.
- Codes are generated locally; no network request is required.
