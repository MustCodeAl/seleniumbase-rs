---
title: "Testing Pages with a CAPTCHA"
---

# Testing Pages with a CAPTCHA

A CAPTCHA exists to stop automation, so a test that has to get past a real one
is fighting its own application. The three big vendors publish **test keys** for
this: with them the widget gives a known answer and the vendor's server accepts
(or refuses) the result on purpose. You run the application under test with
those keys, and the browser test clicks the test widget, checks that the token
field was filled, and submits.

This page is about your own application. It is not about getting through real
CAPTCHAs on sites you do not own: the keys below only work on a page whose site
key is one of them, and `solve_captcha` is a click helper, not a solver.
Whether you may use it on a site you do not control is a question for that
site's terms, and it is out of scope here.

## What you will learn

- Which test keys Google reCAPTCHA, hCaptcha and Cloudflare Turnstile document,
  and where.
- How to give the test deployment of your app those keys.
- How to click the widget with `BaseCase` (WebDriver) and with `sb_cdp` (pure
  CDP), read the token, and submit.
- How to test the failure paths, and what to do about reCAPTCHA v3.

## How the pieces fit

1. Your page embeds the vendor's widget with a **site key**.
2. When the widget is satisfied it writes a **token** into a hidden field of your
   form.
3. Your server sends the token and its **secret key** to the vendor's
   verification endpoint (`siteverify`) and acts on the answer.

With the vendor's test keys, step 1 hands out a token without a real risk check
and step 3 accepts it (or refuses it, for the keys made to fail). Only the keys
differ from production, so the code under test is the code you ship.

## Configure the app under test

Keep the keys in the environment, as for any other deploy-specific setting, and
set the test keys only in the test deployment:

```bash
# .env.test: the test deployment only, never production
CAPTCHA_SITE_KEY=6LeIxAcTAAAAAJcZVRqyHh71UMIEGNQ_MXjiZKhI
CAPTCHA_SECRET_KEY=6LeIxAcTAAAAAGG-vFI1TnRWxMZNFuojJ4WifJWe
```

- Use a **site key and secret of the same set**. Cloudflare's test secrets accept
  only the dummy token and production secrets reject it, so a dummy site key
  next to a real secret fails.
- Make the `siteverify` URL a setting too. Then a test can point the server at a
  stub, which is the way to cover reCAPTCHA v3 and any answer a test key cannot
  give. The same setting can switch the check off: pointed at a stub, or at any
  server that always answers `success: true`, it makes your server accept every
  token. Outside the test deployment, have the application refuse to start
  unless the URL is the vendor's own `siteverify` URL (listed below); production
  keys are real, so the test-key check in the next point does not catch a wrong
  URL.
- Test keys protect nothing, and the secrets are public. Have the application
  refuse to start with a known test key when it runs in production.

## The test keys

Every value below was compared character by character with the vendor's page on
2026-10-15 and each page can change, so check it again before you rely on it.

### Google reCAPTCHA v2 (checkbox)

Source: [reCAPTCHA FAQ, "I'd like to run automated tests with reCAPTCHA"](https://developers.google.com/recaptcha/docs/faq).

| | Value |
|---|---|
| Site key | `6LeIxAcTAAAAAJcZVRqyHh71UMIEGNQ_MXjiZKhI` |
| Secret key | `6LeIxAcTAAAAAGG-vFI1TnRWxMZNFuojJ4WifJWe` |

Google says that with these keys the user never gets a challenge and every
verification request passes, and that the widget shows a warning so the keys are
not used for production traffic. The page is marked deprecated and points to
Google Cloud's documentation, but the keys still worked when checked.

- Markup and token field: the script is `https://www.google.com/recaptcha/api.js`,
  the widget is `<div class="g-recaptcha" data-sitekey="...">`, and the token is
  the form field `g-recaptcha-response`
  ([display docs](https://developers.google.com/recaptcha/docs/display)).
- Verification: `POST https://www.google.com/recaptcha/api/siteverify` with
  `secret` and `response` ([verify docs](https://developers.google.com/recaptcha/docs/verify)).
  With the test secret, any `response` string returned `success: true` (hostname
  `testkey.google.com`).
- What a browser showed: the usual "I'm not a robot" checkbox with the warning
  text; one click ticked it with no image challenge and filled a token of about
  1,700 characters into `textarea[name="g-recaptcha-response"]`. That textarea is
  in your page but hidden, so read its value; do not wait for it to be visible.
  The widget's frame has the title `reCAPTCHA` and a `src` starting with
  `https://www.google.com/recaptcha/api2/anchor`.

### reCAPTCHA v3 and Enterprise

Google documents no test keys for v3 or for Enterprise. The FAQ above advises a
separate key for testing environments and warns that v3 scores may be inaccurate
because v3 relies on seeing real traffic.

v3 has no checkbox, so there is nothing to click. What works in practice (this is
advice, not something the vendor documents):

- Use a separate v3 key registered for the test domain, as Google advises.
- Make the score threshold of your server a setting and lower it in the test
  deployment, or point `siteverify` at a stub that answers with the score your
  test needs.
- Drive the action that makes the page ask for a token, then assert on what your
  server does with it (the page after submit), not on a token field.

### hCaptcha

Source: [hCaptcha, integration testing and test keys](https://docs.hcaptcha.com/#integration-testing-test-keys).

| Key set | Site key | Secret key | Response token |
|---|---|---|---|
| Publisher or Pro account | `10000000-ffff-ffff-ffff-000000000001` | `0x0000000000000000000000000000000000000000` | `10000000-aaaa-bbbb-cccc-000000000001` |
| Enterprise, safe end user | `20000000-ffff-ffff-ffff-000000000002` | `0x0000000000000000000000000000000000000000` | `20000000-aaaa-bbbb-cccc-000000000002` |
| Enterprise, bot detected | `30000000-ffff-ffff-ffff-000000000003` | `0x0000000000000000000000000000000000000000` | `30000000-aaaa-bbbb-cccc-000000000003` |

The Publisher or Pro pair never challenges and always yields the same response
token, which verifies as `success: true`. The two Enterprise pairs stand for a
safe user and a detected bot; hCaptcha asks you to send `remoteip` to `siteverify`
so every Enterprise field comes back.

- Markup and token field: the script is `https://js.hcaptcha.com/1/api.js`, the
  widget is `<div class="h-captcha" data-sitekey="...">`, and the token is the
  form field `h-captcha-response`.
- Verification: a form-encoded `POST https://api.hcaptcha.com/siteverify` with
  `secret`, `response`, and optionally `remoteip` and `sitekey`. The Publisher
  or Pro secret and token returned `success: true` (hostname `dummy-key-pass`).
- What a browser showed (Publisher or Pro key): a checkbox, the notice "This
  hCaptcha is for testing only", and no image challenge. One click put the
  documented token into `textarea[name="h-captcha-response"]`; a second textarea,
  `g-recaptcha-response`, got the same token (hCaptcha's reCAPTCHA compatibility
  mode, on by default; the docs do not promise that second field, so read
  `h-captcha-response`).
- Do not load reCAPTCHA and hCaptcha on one page. hCaptcha's compatibility hook
  claims `window.grecaptcha`, and a `.g-recaptcha` div on such a page was drawn as
  an hCaptcha widget with a "sitekey is incorrect" notice. hCaptcha documents the
  script option `recaptchacompat=off` for this
  ([configuration](https://docs.hcaptcha.com/configuration)); it was not tried.

### Cloudflare Turnstile

Source: [Turnstile, test your Turnstile implementation](https://developers.cloudflare.com/turnstile/troubleshooting/testing/).

| Site key | Behaviour | Widget |
|---|---|---|
| `1x00000000000000000000AA` | always passes | visible |
| `2x00000000000000000000AB` | always fails | visible |
| `1x00000000000000000000BB` | always passes | invisible |
| `2x00000000000000000000BB` | always fails | invisible |
| `3x00000000000000000000FF` | forces an interactive challenge | visible |

| Secret key | Behaviour |
|---|---|
| `1x0000000000000000000000000000000AA` | always passes validation |
| `2x0000000000000000000000000000000AA` | always fails validation |
| `3x0000000000000000000000000000000AA` | fails with `timeout-or-duplicate` (as if the token were spent) |

The test site keys produce the dummy token `XXXX.DUMMY.TOKEN.XXXX`. Cloudflare
says a test secret accepts only that token and rejects real ones, a production
secret does the reverse, and the test keys work on any domain including
`localhost`.

- Markup and token field: the script is
  `https://challenges.cloudflare.com/turnstile/v0/api.js`, the widget is
  `<div class="cf-turnstile" data-sitekey="...">`, and the token is the hidden
  form field `cf-turnstile-response`
  ([client-side rendering](https://developers.cloudflare.com/turnstile/get-started/client-side-rendering/)).
- Verification: `POST https://challenges.cloudflare.com/turnstile/v0/siteverify`
  with `secret` and `response`, and optionally `remoteip` and `idempotency_key`.
  A token is valid for 300 seconds and works once
  ([server-side validation](https://developers.cloudflare.com/turnstile/get-started/server-side-validation/)).
- What a browser showed: `1x...AA` and `1x...BB` filled the dummy token with no
  click; both `2x` keys left `cf-turnstile-response` empty; `3x...FF` showed a
  "Verify you are human" checkbox and filled the dummy token after one click.
- The Turnstile frame is not an `iframe` that page script can find (none showed
  up in `document.querySelectorAll("iframe")` while the widget was on screen), so
  there is no frame to switch into. The `.cf-turnstile` container is what a test
  can see.

### What `siteverify` answered

These calls were made on 2026-10-15 with the keys above, to check that the
secrets behave as documented:

| Call | Answer |
|---|---|
| Turnstile, secret `1x...AA`, dummy token | `success: true` |
| Turnstile, secret `2x...AA`, dummy token | `success: false`, `invalid-input-response` |
| Turnstile, secret `3x...AA`, dummy token | `success: false`, `timeout-or-duplicate` |
| hCaptcha, Publisher or Pro secret and token | `success: true` |
| reCAPTCHA, test secret, any `response` | `success: true` |

Your own server tests can use the same pairs: they cover its handling of a
rejected and of an already-used token without opening a browser.

## Click the widget and read the token

The test has the same four steps for every vendor:

1. Wait until the widget has drawn.
2. Click it.
3. Wait until the token field is non-empty. If it stays empty for a few
   seconds, the click probably came before the widget was ready: click again.
4. Submit the form and assert what your application does.

| Vendor | Widget, as the page sees it | Token field |
|---|---|---|
| reCAPTCHA v2 | `iframe[src*="recaptcha/api2/anchor"]` | `textarea[name="g-recaptcha-response"]` |
| hCaptcha | `iframe[src*="hcaptcha.com"][src*="checkbox"]` | `textarea[name="h-captcha-response"]` |
| Turnstile | `.cf-turnstile` | `input[name="cf-turnstile-response"]` |

The first two widget selectors are the ones `Page::solve_captcha` itself uses.

To try the code without your own application, save this HTML as
`recaptcha.html` and serve its folder
(`python3 -m http.server 8765 --bind 127.0.0.1`). For hCaptcha or Turnstile swap
the script, the div and the site key, and save each as a file of its own
(`hcaptcha.html`, `turnstile.html`): one vendor per page. Its form has no server
behind it, so the token is what a test can check:

```html
<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>reCAPTCHA v2 test fixture</title>
<script src="https://www.google.com/recaptcha/api.js" async defer></script>
</head>
<body>
<form id="signup" action="/verify" method="post">
  <input type="text" name="email" id="email">
  <div class="g-recaptcha" data-sitekey="6LeIxAcTAAAAAJcZVRqyHh71UMIEGNQ_MXjiZKhI"></div>
  <button type="submit" id="send">Send</button>
</form>
</body>
</html>
```

### What `solve_captcha` does

`Page::solve_captcha()` (and `BaseCase::solve_captcha()`, which does the same
over chromedriver) looks for the first visible widget among Cloudflare Turnstile,
reCAPTCHA, hCaptcha, Friendly Captcha and DataDome, in that order, scrolls it to
the middle of the window, and presses 28 pixels in from its left edge at half its
height with trusted mouse events (the DataDome slider is pressed 30 pixels in
and dragged to 30 pixels from the right edge). The widgets live in cross-origin
frames whose insides it cannot inspect, so it does not know whether the click
was accepted. It returns `Some(Captcha::...)` for the widget it pressed, `None`
when no supported widget is visible yet, and an error when the script that looks
for the widget fails or the browser refuses the input. The script can fail
while the page is still navigating ("Execution context was destroyed"), so such
an error need not end a test that has just submitted a form or followed a link:
wait for the page to load and call it again. Because `None` is what you get
before the widget has drawn, wait for the widget first, and check the token
afterwards.

With the `gui` feature, `Page::gui_click_captcha()` makes the same press with the
machine's own pointer; test widgets do not need it (see
[Pure CDP](pure_cdp.md#captchas)).

### With the pure CDP engine

```rust,no_run
use std::time::Duration;

use seleniumbase_rs::sb_cdp::{Browser, Captcha, LaunchOptions, Page, State};
use seleniumbase_rs::SeleniumBaseError;

/// Presses `widget` until the hidden field `field` holds a token, and returns
/// the token.
///
/// The widget's frame can be on screen before the checkbox inside it has
/// drawn, and a press that early is lost. `solve_captcha` cannot tell, so each
/// press gets five seconds to produce a token, and the widget is pressed again
/// if none came.
async fn press_until_token(
    page: &Page,
    widget: Captcha,
    field: &str,
) -> Result<String, SeleniumBaseError> {
    // Truthy once the field holds a token; the `?.` keeps it from throwing
    // before the field exists.
    let token = format!("document.querySelector('{field}')?.value");
    let short = page.with_timeout(Duration::from_secs(5));
    for _ in 0..3 {
        assert_eq!(page.solve_captcha().await?, Some(widget));
        match short.wait_for_function(&token).await {
            Ok(()) => return page.evaluate_as(&token).await,
            // No token: the press came too early, so press again.
            Err(SeleniumBaseError::WaitTimeout(_)) => {}
            Err(other) => return Err(other),
        }
    }
    Err(SeleniumBaseError::WaitTimeout(format!(
        "no token in {field} after three presses"
    )))
}

#[tokio::test]
async fn signup_with_the_recaptcha_test_widget() -> Result<(), SeleniumBaseError> {
    let browser = Browser::launch(LaunchOptions::builder().headless(true).build()?).await?;
    let page = browser.default_page().await?;

    page.goto("http://localhost:3000/signup").await?;
    page.locator("#email").fill("ada@example.com").await?;

    // 1. Wait for the widget's frame (it is an element of your page).
    page.locator(r#"iframe[src*="recaptcha/api2/anchor"]"#)
        .wait_for(State::Visible)
        .await?;

    // 2 and 3. Click it until the token lands in the hidden field of your page.
    let field = r#"textarea[name="g-recaptcha-response"]"#;
    let token = press_until_token(&page, Captcha::Recaptcha, field).await?;
    assert!(!token.is_empty());

    // 4. Submit and assert what your application shows.
    page.locator("#send").click().await?;
    page.locator("body")
        .expect()
        .to_contain_text("Thanks")
        .await?;

    browser.close().await?;
    Ok(())
}
```

The same shape works for the other two vendors, with their frame and field from
the table. Give each vendor a page of its own: `solve_captcha` presses the first
widget it finds, and it looks for Turnstile before reCAPTCHA and hCaptcha, so on
a page with Turnstile and hCaptcha it would press Turnstile even when the test
meant hCaptcha. A test key gives a known token, so the test can assert it:

```rust,no_run
# use seleniumbase_rs::sb_cdp::{Captcha, Page, State};
# use seleniumbase_rs::SeleniumBaseError;
# // `press_until_token` is the helper from the listing above.
# async fn press_until_token(_: &Page, _: Captcha, _: &str) -> Result<String, SeleniumBaseError> {
#     unimplemented!()
# }
# async fn demo(page: Page) -> Result<(), SeleniumBaseError> {
// hCaptcha, Publisher or Pro test key, on the hCaptcha fixture.
page.goto("http://127.0.0.1:8765/hcaptcha.html").await?;
page.locator(r#"iframe[src*="hcaptcha.com"][src*="checkbox"]"#)
    .wait_for(State::Visible)
    .await?;
let field = r#"textarea[name="h-captcha-response"]"#;
let token = press_until_token(&page, Captcha::Hcaptcha, field).await?;
assert_eq!(token, "10000000-aaaa-bbbb-cccc-000000000001");

// Turnstile, the key that forces an interactive challenge (3x...FF), on a page
// of its own. Wait for the container to be visible, not only to exist: the
// empty div has no height until the widget starts to draw (unless your CSS
// gives it one), and solve_captcha skips a widget without a size.
page.goto("http://127.0.0.1:8765/turnstile.html").await?;
page.locator(".cf-turnstile").wait_for(State::Visible).await?;
let field = r#"input[name="cf-turnstile-response"]"#;
let token = press_until_token(&page, Captcha::Turnstile, field).await?;
assert_eq!(token, "XXXX.DUMMY.TOKEN.XXXX");
# Ok(())
# }
```

A Turnstile key that always passes (`1x...AA`, `1x...BB`) needs no click: skip
`press_until_token` and wait for the token field with `wait_for_function`.

### With WebDriver (`BaseCase`)

`BaseCase` has no wait for a field's value, so a small helper does it. The field
is hidden, and `get_value` reads the live `value` property of an element whether
or not it is visible:

```rust,no_run
use seleniumbase_rs::{run_browser_test, BaseCase, BrowserConfig, Result, SeleniumBaseError};

/// Waits up to `seconds` for the hidden field `css` to hold a token.
async fn wait_for_token(sb: &mut BaseCase, css: &str, seconds: u32) -> Result<String> {
    for _ in 0..seconds * 4 {
        if let Ok(value) = sb.get_value(css).await {
            if !value.is_empty() {
                return Ok(value);
            }
        }
        sb.sleep(0.25).await;
    }
    Err(SeleniumBaseError::WaitTimeout(format!(
        "no token in {css} after {seconds} seconds"
    )))
}

#[tokio::test]
async fn signup_with_the_recaptcha_test_widget() -> Result<()> {
    run_browser_test(BrowserConfig::default(), |sb| {
        Box::pin(async move {
            sb.open("http://localhost:3000/signup").await?;
            sb.type_text("#email", "ada@example.com").await?;

            // 1. Wait for the widget's frame.
            let frame = r#"iframe[src*="recaptcha/api2/anchor"]"#;
            sb.wait_for_element_visible(frame, 15).await?;

            // 2. Click it: enter the frame (WebDriver can enter a frame from
            //    another origin), click the checkbox, and come back out.
            sb.switch_to_frame(frame).await?;
            sb.click("#recaptcha-anchor").await?;
            sb.switch_to_default_content().await?;

            // 3. Read the token from your page.
            let field = r#"textarea[name="g-recaptcha-response"]"#;
            let token = wait_for_token(sb, field, 15).await?;
            assert!(!token.is_empty());

            // 4. Submit and assert.
            sb.click("#send").await?;
            sb.assert_text_visible("Thanks", "body").await
        })
    })
    .await
}
```

Google does not document the markup inside its frame. `#recaptcha-anchor` is the
checkbox's id as test suites have long used it, but it was not checked here, so
look at it in DevTools before you depend on it. To avoid depending on it at all,
replace step 2 with `solve_captcha`, which presses by position and never enters
the frame:

```rust,no_run
# use seleniumbase_rs::sb_cdp::Captcha;
# use seleniumbase_rs::{BaseCase, SeleniumBaseError};
# async fn demo(sb: &mut BaseCase) -> Result<(), SeleniumBaseError> {
// From the top-level document, after the widget has drawn. Chromium only.
assert_eq!(sb.solve_captcha().await?, Some(Captcha::Recaptcha));
# Ok(())
# }
```

As in the pure CDP helper above, a press can come before the checkbox has
drawn and be lost: give `wait_for_token` a few seconds, and press again when it
times out.

For hCaptcha and Turnstile use `solve_captcha` too: Turnstile has no frame to
enter, and hCaptcha's inner markup was not checked here.

## Failure paths

A test key that fails is how you cover your error handling without a flaky real
challenge:

- **No token.** With Turnstile `2x00000000000000000000AB` the widget never
  produces a token. Assert that your form refuses to submit, or that your server
  rejects the empty token and shows your error message.
- **Rejected token.** Send the dummy token to `siteverify` with the secret
  `2x0000000000000000000000000000000AA` (answer `invalid-input-response`), or with
  `3x0000000000000000000000000000000AA` (answer `timeout-or-duplicate`), to test
  how your server reports a refused and a used token.
- **Bot detected.** hCaptcha's Enterprise "bot detected" pair stands for a
  detected bot, for accounts that have Enterprise (it was not run here).
- **reCAPTCHA** documents no failing key. Stub `siteverify` (see above) to make
  the answer fail.

## In CI

- The widgets load their scripts and frames from the vendors' hosts. A runner
  without network access to them, or a Content Security Policy (CSP) on your
  page that blocks them, shows no widget, and `solve_captcha` returns `None`.
  Take the hosts from each vendor's CSP guidance (below, as read on 2026-10-15),
  not from a list of what one page load fetched: reCAPTCHA's `api.js` adds a
  second script from `www.gstatic.com` to your page, and hCaptcha's asset
  subdomains change over time and by region.

  | Vendor | `script-src` | `frame-src` | Other directives |
  |---|---|---|---|
  | reCAPTCHA ([FAQ, the CSP question](https://developers.google.com/recaptcha/docs/faq)) | `https://www.google.com/recaptcha/`, `https://www.gstatic.com/recaptcha/` | `https://www.google.com/recaptcha/`, `https://recaptcha.google.com/recaptcha/` | `connect-src`: `https://www.google.com/recaptcha/` |
  | hCaptcha ([CSP settings](https://docs.hcaptcha.com/#content-security-policy-settings)) | `https://hcaptcha.com`, `https://*.hcaptcha.com` | the same | `style-src` and `connect-src`: the same |
  | Turnstile ([Content Security Policy](https://developers.cloudflare.com/turnstile/reference/content-security-policy/)) | `https://challenges.cloudflare.com` | the same | none |

  Google and Cloudflare prefer a nonce on the `api.js` script tag to a host
  list; the widget passes the nonce on to the scripts it adds. A CI runner that
  limits outbound traffic needs the same hosts.
- Treat a token as single-use: Turnstile documents that one works once and
  expires after 300 seconds. Start every test from a fresh page load instead of
  reusing a token.
- Run Chrome headless in CI. The three widgets were clicked successfully by
  `solve_captcha` in a headless Chrome (see "What has been checked").

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| `solve_captcha` returns `None` | The widget has not drawn yet, its script was blocked, or it is hidden | Wait for the frame or container first; allow the hosts listed under [In CI](#in-ci) in your CSP and on the CI runner. |
| `solve_captcha` returns `Some(..)` but the token stays empty | The wrong site key (not a test key), or the click came before the widget was ready | Press again after a few seconds, as `press_until_token` does; check the key in the page source; take a screenshot (`save_screenshot`) to see the widget. |
| The widget says the site key is invalid | A production key on a test host, or a key from another vendor | Use the test key of the vendor whose script the page loads. |
| An hCaptcha widget appears where a reCAPTCHA one should | Both scripts on one page | One vendor per page, or `recaptchacompat=off` on the hCaptcha script. |
| `siteverify` refuses the dummy token | A production secret with a test site key | Use the secret of the same test set. |
| `wait_for_function` fails at once with a script error | The field does not exist yet and the expression read `.value` of `null` | Use `?.value`, as above. |

## Security notes

- Test keys give no protection and their secrets are public. Never deploy them,
  and make the application fail at start-up when it finds one in production.
- The same goes for a `siteverify` URL that is not the vendor's: in production
  it would accept whatever the stub accepts. Fail at start-up on that too.
- Keep production keys out of test environments and out of the repository.
- Do not point these tests at a site you do not own or have permission to test.

## What has been checked

On 2026-10-15, on macOS:

- Every key, the markup names and the CSP hosts above against the vendors' pages
  (linked in each section), and the `siteverify` answers in the table above with
  real calls. reCAPTCHA's `api.js`, fetched the same day, adds its second script
  from `https://www.gstatic.com/recaptcha/releases/`.
- A real Chrome, clicked by hand, on a local fixture: the reCAPTCHA v2 test key,
  the hCaptcha Publisher or Pro key and all five Turnstile site keys behaved as
  described, including the tokens.
- This crate's own `Page::solve_captcha`, through `sbrs-mcp --server cdp`
  (0.3.0, pure CDP, headless Chrome) on the same fixture: it attempted
  reCAPTCHA, hCaptcha and Turnstile (`3x...FF`), and each token field was filled
  (the hCaptcha and Turnstile tokens equal the documented ones).
- The four Rust listings on this page type-check against this crate
  (rust-analyzer; tutorials are not compiled by `cargo`).

Not checked: none of the listings was run; `BaseCase::solve_captcha`, the
frame-switching listing and `#recaptcha-anchor` were not tried on a live widget;
the hCaptcha Enterprise key sets were not exercised in a browser;
`recaptchacompat=off` was not tried; and nothing was tried through the crate in
a headed browser.

## See also

- [Pure CDP](pure_cdp.md) for `Browser`, `Page` and `Locator`.
- [Writing Browser Tests](../rust-test-tooling.md) for `run_browser_test`.
- [Waits and Assertions](waits_assertions.md).
