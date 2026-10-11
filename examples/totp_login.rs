//! Sign in with a time-based one-time password (TOTP), the code an
//! authenticator app shows.
//!
//! `get_totp_code(secret)` turns a base32 secret into the current six-digit
//! code (RFC 6238: HMAC-SHA1, 30-second steps), and `enter_totp_code(css,
//! secret)` types it into a field. The example serves a login page whose own
//! JavaScript checks the code with the browser's WebCrypto, independently of the
//! Rust implementation, so an accepted code proves the code is right. A wrong
//! code must be rejected.
//!
//! The secret comes from the `TOTP_SECRET` environment variable. When it is
//! not set the example uses the public test key from RFC 6238's reference
//! material, which protects nothing. In a real test, load your secret from the
//! environment or a secrets manager, never from source code.
//!
//! Run it with:
//!
//! ```bash
//! cargo run --example totp_login
//! TOTP_SECRET=<your base32 secret> cargo run --example totp_login
//! ```
//!
//! It needs Chrome and a matching chromedriver on your `PATH` (`sbase doctor`
//! checks both) and does not need the network: the page is served from
//! `127.0.0.1`.

mod local_site;

use local_site::{ensure, Site};
use seleniumbase_rs::{run_browser_test, BaseCase, BrowserConfig, Result, SeleniumBaseError};

/// The base32 form of the ASCII key `12345678901234567890`, the shared secret
/// in the RFC 4226 and RFC 6238 test vectors. It is public.
const PUBLIC_TEST_SECRET: &str = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";

/// The login page. It accepts the code for the current 30-second step and the
/// one on either side, the usual allowance for clock drift. `__SECRET__` is
/// replaced with the secret before the page is served.
const LOGIN_PAGE: &str = r#"<!doctype html>
<html lang="en">
<head><meta charset="utf-8"><title>Two-factor sign-in</title></head>
<body>
  <h1>Two-factor sign-in</h1>
  <form id="login">
    <label for="username">Username</label>
    <input id="username" autocomplete="off">
    <label for="code">Verification code</label>
    <input id="code" autocomplete="off" inputmode="numeric">
    <button id="verify" type="submit">Verify</button>
  </form>
  <p id="status" aria-live="polite"></p>
  <script>
    const SECRET = '__SECRET__';

    function base32Decode(text) {
      const alphabet = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ234567';
      let bits = '';
      for (const c of text.replace(/[ =-]/g, '').toUpperCase()) {
        bits += alphabet.indexOf(c).toString(2).padStart(5, '0');
      }
      const bytes = [];
      for (let i = 0; i + 8 <= bits.length; i += 8) bytes.push(parseInt(bits.slice(i, i + 8), 2));
      return new Uint8Array(bytes);
    }

    async function codeForStep(step) {
      const key = await crypto.subtle.importKey(
        'raw', base32Decode(SECRET), { name: 'HMAC', hash: 'SHA-1' }, false, ['sign']);
      const counter = new ArrayBuffer(8);
      new DataView(counter).setUint32(4, step);
      const mac = new Uint8Array(await crypto.subtle.sign('HMAC', key, counter));
      const offset = mac[mac.length - 1] & 15;
      const value = ((mac[offset] & 0x7f) << 24) | (mac[offset + 1] << 16)
                  | (mac[offset + 2] << 8) | mac[offset + 3];
      return String(value % 1000000).padStart(6, '0');
    }

    document.getElementById('login').addEventListener('submit', async (event) => {
      event.preventDefault();
      const entered = document.getElementById('code').value.trim();
      const step = Math.floor(Date.now() / 30000);
      let ok = false;
      for (const candidate of [step - 1, step, step + 1]) {
        if (entered === await codeForStep(candidate)) ok = true;
      }
      document.getElementById('status').textContent =
        ok ? 'Verified: welcome ' + document.getElementById('username').value : 'Invalid code';
    });
  </script>
</body>
</html>
"#;

#[tokio::main]
async fn main() -> Result<()> {
    let secret = std::env::var("TOTP_SECRET").unwrap_or_else(|_| PUBLIC_TEST_SECRET.to_owned());
    // The secret is placed in the page's script, so accept only base32 text.
    if !secret
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '-' | '='))
    {
        return Err(SeleniumBaseError::InvalidConfig(
            "TOTP_SECRET must be base32 text".to_owned(),
        ));
    }

    let page = LOGIN_PAGE.replace("__SECRET__", &secret);
    let url = Site::serve(&[("/", page.as_str())])?.url("/");
    run_browser_test(BrowserConfig::default(), |sb| {
        Box::pin(sign_in(sb, url, secret))
    })
    .await
}

async fn sign_in(sb: &mut BaseCase, url: String, secret: String) -> Result<()> {
    sb.open(&url).await?;
    sb.type_text("#username", "alice").await?;

    let code = sb.get_totp_code(&secret)?;
    println!("Generated TOTP: {code}");
    ensure(
        code.len() == 6 && code.chars().all(|c| c.is_ascii_digit()),
        "the code is six digits",
    )?;

    // A wrong code is rejected.
    let wrong = format!("{:06}", (code.parse::<u32>().unwrap_or(0) + 1) % 1_000_000);
    sb.type_text("#code", &wrong).await?;
    sb.click("#verify").await?;
    wait_for_status(sb, "Invalid code").await?;

    // The right code is accepted. `enter_totp_code` generates and types it.
    sb.enter_totp_code("#code", &secret).await?;
    sb.click("#verify").await?;
    wait_for_status(sb, "Verified: welcome alice").await?;
    println!("The one-time password was accepted");
    Ok(())
}

/// The page checks the code asynchronously, so wait for its verdict.
async fn wait_for_status(sb: &mut BaseCase, expected: &str) -> Result<()> {
    sb.wait_for_text("#status", expected, 5).await
}
