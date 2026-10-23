---
title: "How logging in works"
sidebarTitle: "Authentication"
description: "How websites know who you are: sessions and tokens, cookie logins, HTTP Basic auth, OAuth 2.0 and OpenID Connect redirects, JWTs, one-time codes (TOTP) and passkeys; how to log in fast in tests with saved sessions; and the TOTP, saved-cookie and session-store helpers of seleniumbase-rs."
---

<Info>

**In short:** HTTP has no memory, so after you prove who you are once (a
password, a code, a fingerprint), the site hands your browser a **token** (a
cookie, or a value your page keeps) and your browser sends it back with every
request. Everything about logging in is a variation on that: how you prove
yourself, what the token is, and where it lives. A test that understands this
can skip the slow part.

</Info>

**Authentication** answers "who are you?". **Authorization** answers "what may you
do?". They are different checks, and mixing them up is how
[broken access control](/concepts/web-security) happens. This page is about the
first.

## Sessions and tokens

After a login the server needs a way to recognise you on the next request. There
are two main designs.

| | Server-side session | Self-contained token (such as a JWT) |
|---|---|---|
| What the browser holds | A random **session ID**, usually in a cookie | A signed token with the user's details inside |
| Where the facts are | In a store on the server | Inside the token itself |
| Signing out | Delete the session on the server; it is dead at once | Hard: the token is valid until it expires, unless the server keeps a block list |
| Scaling | The server needs a shared session store | Any server can check the signature |
| Typical home in the browser | A cookie with `HttpOnly` | A cookie, or `localStorage` and an `Authorization` header |

Where the token lives matters. A cookie marked `HttpOnly` cannot be read by page
JavaScript, so a cross-site scripting bug cannot steal it. A token in
`localStorage` can be read by any script on the page. The same page shows how
cookies and storage behave: see [sessions, cookies and storage](/concepts/sessions-and-state).

## The cookie login

The oldest and still most common login: a form, a `POST`, a cookie.

```mermaid
sequenceDiagram
    participant B as Browser
    participant S as Server
    B->>S: POST /login with the username and password
    S-->>B: 302 redirect and Set-Cookie sid=abc123 with HttpOnly, Secure, SameSite
    B->>S: GET /account with Cookie sid=abc123
    S-->>B: 200 your account page
    Note over B,S: Every later request to this site carries the cookie
```

*A cookie login. The cookie is the proof of the login, so anyone who has it is
you.*

That last point is the foundation of fast test logins below: **the cookie is the
login**. Copy the cookie into another browser and that browser is logged in.

## HTTP Basic authentication

**Basic** authentication is built into HTTP. The server answers `401` with a
header `WWW-Authenticate: Basic realm="..."`, the browser pops up its own
username and password box, and from then on it sends

```http
Authorization: Basic dXNlcjpwYXNzd29yZA==
```

which is the Base64 of `user:password`. That is an encoding, not encryption
(see [encryption](/concepts/encryption)), so Basic is safe only over https.

For automation the awkward part is the **native dialog**: it is not part of the
page, so no selector can reach it. The ways around it are to put the
credentials in the address (`https://user:pass@host/`, accepted for the first
navigation by most browsers), to send the `Authorization` header yourself by
intercepting requests ([network](/concepts/network)), or to answer the
`Fetch.authRequired` event of the [DevTools Protocol](/concepts/cdp). Bearer
tokens use the same header with a different word: `Authorization: Bearer <token>`.

## OAuth 2.0 and OpenID Connect

"Sign in with Google" uses two standards. **OAuth 2.0** lets an app get limited
access to your account *without ever seeing your password*. **OpenID Connect**
(OIDC) adds an **ID token** on top, so the app also learns who you are.

The usual flow is the *authorization code flow* with **PKCE**:

```mermaid
sequenceDiagram
    participant U as Browser
    participant A as App
    participant I as Identity provider
    U->>A: Click "Sign in"
    A-->>U: Redirect to the provider with client_id, scope, state and a PKCE challenge
    U->>I: Open the provider's login page
    Note over U,I: The user signs in here, with MFA if needed
    I-->>U: Redirect back to the app with a one-time code and the state
    U->>A: Follow the redirect
    A->>I: Exchange the code and the PKCE verifier for tokens
    I-->>A: Access token, ID token and refresh token
    A-->>U: Set the app's own session cookie
```

*The browser is sent away to the provider and back. The password is typed on the
provider's page only.*

Three terms help: the **state** value stops a forged return; **PKCE** stops
someone who steals the code from using it; and the **scope** names what access
is asked for.

**Tests and OAuth.** The redirect chain crosses several origins, so a test must
wait for the browser to come back to the app rather than act at once. Do not
automate a login on a real public provider: they detect bots, ask for MFA and
may lock the account. Use a provider test tenant, a fake identity provider that
your suite starts, or skip the redirect altogether with the fast login below.

## JWTs

A **JSON Web Token** is a compact, signed note. It is three Base64URL parts
joined by dots: a header, a payload and a signature.

```text
eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ1c2VyLTQyIiwiZXhwIjoxNzAwMDAwMDAwfQ.sig...
 header   -> {"alg":"HS256"}
 payload  -> {"sub":"user-42","exp":1700000000}
 signature-> HMAC (or a private-key signature) of the first two parts
```

The payload holds **claims**: `sub` (who), `iss` (who issued it), `aud` (who it is
for), `iat` and `exp` (issued and expiry times, in seconds since 1970).

- It is **signed, not encrypted.** Anyone can read the payload by decoding it, so
  never put a secret in a JWT.
- The server must check the signature, the expiry and the audience, and must
  refuse the `none` algorithm. Skipping any of these is a classic hole.
- Access tokens are short (minutes) and are renewed with a longer-lived
  **refresh token**.

## Multi-factor authentication and TOTP

**MFA** asks for more than one kind of proof: something you **know** (a
password), something you **have** (a phone, a key) and something you **are** (a
fingerprint).

The common "have" is a **TOTP** (time-based one-time password), the six digits in
an authenticator app, defined in RFC 6238. When you enrol, the site shows a QR
code that holds a **shared secret** (a Base32 text). From then on both sides
compute the same value:

```text
code = last six digits of  HMAC(secret, floor(unix_time / 30))
```

A new code appears every 30 seconds and the server accepts the current one
(often the neighbouring ones too, for clock drift). Nothing travels between
phone and server; only the shared secret and the clock matter.

For a test account this is good news: **a TOTP is just a calculation.** Keep the
Base32 secret of the test account in your secret store and compute the code when
the login asks for it. Two practical rules: the machine's clock must be right,
and if the current window has only a second or two left, wait for the next one,
or the code will have expired by the time it is submitted. Codes sent by SMS or
email need a test inbox or number; push approvals ("tap yes on your phone")
cannot be automated at all, so ask for a test account that uses TOTP.

## Passkeys

A **passkey** replaces the password with a key pair, using the **WebAuthn**
standard. The device creates the pair when you register; the private key never
leaves it (and is unlocked by a fingerprint, face or PIN), and the site stores only
the public key. To log in, the site sends a random **challenge**, the device
signs it, the site checks the signature. There is no password to steal or
phish, and the key works only for the site it was made for.

For tests the unlock step is a native system dialog, which automation cannot
click. The DevTools Protocol has a `WebAuthn` domain with **virtual
authenticators**: a fake device inside the browser that approves requests on
its own.

## Logging in fast in tests

A UI login can take seconds and may involve a redirect chain, MFA and rate
limits. Doing it before every test is slow and fragile. Because the cookie is
the login, you can do it once and reuse it.

<Steps>

<Step title="Log in once" icon="right-to-bracket">

Either through the real page, or by calling the login API directly with an HTTP
client and keeping the `Set-Cookie` values.

</Step>

<Step title="Keep the proof" icon="floppy-disk">

Save the cookies (and the `localStorage` entries, for token-in-storage apps) to
a file, or to a store that other workers share.

</Step>

<Step title="Inject it into each test" icon="syringe">

Open any page of the right site, put the cookies into the browser, and reload.
The test starts already signed in, on the page it is about.

</Step>

</Steps>

| Approach | Speed | Realism | Watch out for |
|---|---|---|---|
| Log in through the UI in each test | Slowest | Highest | MFA, rate limits, redirect chains |
| Log in through the API, inject the cookie | Fast | Good | The API login must create the same cookie as the page |
| Save a UI login once, reuse it | Fast | High | Expired sessions: save again when a cookie runs out |
| Pre-made test session in the app (a test-only endpoint) | Fastest | Lower | Never ship that endpoint to production |

<Warning>

Keep at least one test that logs in through the real page. Fast logins prove the
rest of the app works for a signed-in user; they prove nothing about the login.
Also treat a saved cookie file like a password: it **is** a password until it
expires. Keep it out of Git and out of CI logs.

</Warning>

## How seleniumbase-rs uses it

<Tabs>

<Tab title="MFA and TOTP" icon="mobile-screen">

`BaseCase` computes the code for you:

- `get_totp_code(secret)` returns the current six-digit code for a Base32 secret.
  `get_mfa_code` and `get_google_auth_code` are aliases of it. The code
  generator is checked against the RFC 6238 test vectors, ignores spaces, dashes and
  letter case in the secret, and refuses text that is not Base32.
- `enter_totp_code(css, secret)` generates the code, types it into the element
  that `css` selects and presses Enter. `enter_mfa_code` is an alias. This matches
  Python's `enter_mfa_code(selector, totp_key)`.

```rust,ignore src/main.rs
// The Base32 secret comes from your secret store, not from the source file.
let secret = std::env::var("TEST_USER_TOTP_SECRET")?;
// ... after the username and password step, on the page that asks for the code:
sb.enter_totp_code("#otp", &secret).await?;
```

</Tab>

<Tab title="Saved cookies" icon="cookie">

- `save_cookies(name)` writes the page's cookies as JSON to
  `saved_cookies/<name>.txt` in the working folder and returns the path. Only the
  last part of the name is used, so a name cannot leave the folder.
  `save_cookies_to_path(path)` writes to a file of your choice.
- `load_cookies(name)` adds them back. Open a page of the right site first,
  because the browser refuses a cookie for another site. Cookies of another
  domain are given the current page's domain, and their expiry is dropped, so they
  last for the browser session; `load_cookies_with(name, CookieExpiry)` changes
  that. `load_cookies_from_path(path)` reads from a file.
- `get_saved_cookies(name)` reads the file without a browser,
  `delete_saved_cookies(name)` removes it, and `add_cookies(map)` adds cookies
  from a map of names to values.

```rust,ignore src/main.rs
// First run, after a real login through the page:
sb.save_cookies("app_login").await?;        // saved_cookies/app_login.txt

// Later runs, after opening any page of the same site:
sb.load_cookies("app_login").await?;
// ... then reload the page: the session is signed in.
```

</Tab>

<Tab title="Shared sessions (pure CDP)" icon="users">

The pure CDP engine ([DevTools Protocol](/concepts/cdp)) keeps logins in memory.
A `Session` is a set of cookies and, optionally, the local storage of one origin
(`Session::new(cookies)`, `with_local_storage(origin, entries)`). A
`SessionStore` holds named sessions for many workers:

- `save(name, session)`, `load(name)`, `remove(name)`, `names()`, `len()`,
  `clear()`.
- `prune_expired(now)` drops the expired cookies from every session and returns
  how many it dropped, and `Session::without_expired(now)` does the same for one
  session (cookies without an expiry are kept).
- A `BrowserPool` exposes its store through `sessions()`.
- Cloning a store gives another handle to the **same** store, so one worker can
  sign in and the others load the result.

```rust,ignore src/main.rs
use seleniumbase_rs::sb_cdp::{Cookie, Session, SessionStore};

let store = SessionStore::new();
store.save("alice", Session::new(vec![Cookie::new("sid", "s3cret")]));

let shared = store.clone();          // a second worker's handle to the same store
let alice = shared.load("alice");    // Some(Session) with the cookie
```

Nothing is written to disk, and the store's `Debug` output never shows a cookie
value, so it is safe to log.

</Tab>

<Tab title="Not covered" icon="circle-info">

The library has no OAuth, OpenID Connect, passkey or website-Basic-prompt helper:
you drive those with ordinary navigation and clicks, or with the fast logins
above. Its `proxy_auth` module answers *proxy* password prompts only and hands a
website's own `401` prompt back to the browser. The pure CDP engine can send any
command by name (`Page::execute`), which is how the `WebAuthn` virtual
authenticator commands could be used.

</Tab>

</Tabs>

## Where to go next

<Columns cols={2}>

<Card title="Sessions, cookies and storage" icon="cookie" color="#1a85d4" href="/concepts/sessions-and-state">
  What the browser keeps, and for how long.
</Card>

<Card title="Security policies" icon="lock" color="#d52c36" href="/concepts/security-policies">
  SameSite, CORS credentials and the other rules that shape a login.
</Card>

<Card title="Forms" icon="rectangle-list" color="#199f43" href="/concepts/forms">
  How the login form itself works, and how tests fill it in.
</Card>

<Card title="Glossary" icon="book" color="#a631be" href="/glossary">
  Short definitions of every term used in these pages.
</Card>

</Columns>
