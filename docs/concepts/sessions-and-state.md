---
title: "Sessions, cookies, storage and logging in"
sidebarTitle: "Sessions and logging in"
description: "Why HTTP forgets you and how browsers and sites remember: cookies in depth (Set-Cookie, Domain, Path, Secure, HttpOnly, SameSite, third-party cookies), web storage, origins, sessions and tokens, cookie logins, HTTP Basic auth, OAuth 2.0 and OpenID Connect, JWTs, one-time codes (TOTP) and passkeys; browser profiles and contexts; how to log in fast in tests with a saved signed-in state; and the TOTP, saved-cookie and session-store helpers of seleniumbase-rs."
---

<Info>

**In short:** HTTP has no memory, so after you prove who you are once (a
password, a code, a fingerprint), the site hands your browser a **token** (a
cookie, or a value your page keeps) and the browser sends it back with every
request. That token lives in state the browser keeps: cookies, `localStorage`,
`sessionStorage` and `IndexedDB`, all inside a browser profile. Everything about
logging in is a variation on that: how you prove yourself, what the token is,
and where it lives. Keep the profile, or save the state and load it into another
browser, and the site sees the same signed-in user without a new login, so a
test that understands this can skip the slow part.

</Info>

## HTTP forgets everything

The web's transport, [HTTP](/concepts/network#http-in-ten-minutes), is
**stateless**: each request is a self-contained question, and the server
answers it without remembering the last one. Ask for `/inbox` twice and, as far
as the protocol is concerned, two strangers asked.

Every feature that needs memory (a login, a shopping cart, a language choice, a
"don't show this again") has to be added on top. The browser carries the memory
and hands it back with each request. There are two places it can live:

- **In the request itself**, as a **cookie**: a small piece of text the browser
  sends automatically, so the server sees it on every request.
- **In the page's storage**, as **web storage**: data the page's own JavaScript
  reads and writes. The server sees it only if the script chooses to send it.

## Cookies, in depth

A cookie is born when a server answers with a `Set-Cookie` header. The browser
stores it, and sends it back in a `Cookie` header on later requests that match:

```http
HTTP/1.1 200 OK
Set-Cookie: sid=9f2c41ab7e; Domain=example.com; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age=2592000

GET /inbox HTTP/1.1
Host: shop.example.com
Cookie: sid=9f2c41ab7e; theme=dark
```

Everything after the first `name=value` pair is an **attribute** that tells
the browser when the cookie applies and how to protect it:

| Attribute | What it does |
|---|---|
| `Domain` | Which hosts get the cookie. `Domain=example.com` covers `example.com` and every subdomain. **Without it** the cookie is "host-only": it goes to the exact host that set it and to no subdomain. |
| `Path` | Only URLs under this path get the cookie. `Path=/` means the whole site; `Path=/admin` means `/admin` and below. |
| `Expires` and `Max-Age` | How long the cookie lives. `Expires` is a date; `Max-Age` is seconds from now (and wins if both are present). Without either it is a **session cookie** and disappears when the browser closes. With one it is **persistent** and survives restarts. |
| `Secure` | Send it only over HTTPS. |
| `HttpOnly` | Hide it from JavaScript: `document.cookie` cannot read it. This protects a login cookie from being stolen by an injected script. The browser still sends it. |
| `SameSite` | Whether the cookie is sent on requests that start from another site. `Strict`: never. `Lax` (the default in modern browsers): only on top-level navigations such as following a link. `None`: always, but then `Secure` is required. |
| `Partitioned` | Store this third-party cookie separately for each top-level site (see below). |

Cookie names can also carry a **prefix** that the browser enforces:
`__Secure-` requires `Secure`, and `__Host-` additionally requires `Path=/` and
no `Domain`, which pins the cookie to one host.

### First-party and third-party

A cookie is **first-party** when it belongs to the site shown in the address
bar and **third-party** when it is set by something embedded in the page: an
ad, a video player, a "like" button. The same third-party service embedded on
many sites can recognise you on all of them, which is how cross-site tracking
works. Browsers have been closing that door in different ways: blocking
third-party cookies, applying `SameSite=Lax` by default, and **partitioning**
(each top-level site gets its own copy of the third-party cookie, so the
embedded service cannot connect your visits).

"Same site" has a precise meaning: the **registrable domain**, the part of the
host you can buy, found with the Public Suffix List. `shop.example.com` and
`www.example.com` are the same site; `example.com` and `example.org` are not;
`alice.github.io` and `bob.github.io` are different sites, because `github.io`
is on the list.

### Limits and the rest

Browsers cap a cookie at about 4 KB and the number of cookies per domain (Chrome:
about 180), dropping the oldest. Cookies are also not private to a port or to
`http` versus `https` in the way origins are, which is part of why they are
easy to misuse. A cookie is the **key to the account**: if a server uses a
random `sid` value as proof of login, whoever holds that value is that user.

## Web storage

Cookies are small and travel on every request. For larger data that only the
page needs, browsers provide storage APIs. They are scoped to an **origin**
(defined below).

| Storage | Size | Lifetime | Shared between tabs | Sent to the server |
|---|---|---|---|---|
| `localStorage` | About 5 MB | Until deleted | Yes, all tabs of the origin | Never |
| `sessionStorage` | About 5 MB | Until the tab closes | No, one tab only | Never |
| `IndexedDB` | Large (a share of the disk) | Until deleted | Yes | Never |
| Cookies | About 4 KB each | Until they expire | Yes | With every matching request |

`localStorage` and `sessionStorage` hold strings as key and value pairs and are
synchronous. `IndexedDB` is a real database of objects, with indexes and
transactions, and is asynchronous. Single-page apps often keep their session
token, user settings or an offline copy of data in these places, not in
cookies, so saving only the cookies may not be enough to restore a login.

Two more places hold state: the **HTTP cache** (files the browser reuses
instead of downloading again) and **service workers** with their Cache Storage
(a script that can answer requests itself, which is how sites work offline).

## Origins and the same-origin policy

An **origin** is the triple of **scheme, host and port**, and two URLs are the
same origin only if all three match. The comparison table is in
[Origins: the unit of trust](/concepts/web-security#origins-the-unit-of-trust).

The **same-origin policy** is the browser's main security rule: a page can read
data only from its own origin. A script on `evil.example` cannot read your
`localStorage` for `bank.example`, cannot read the response of a request to
`bank.example`, and cannot reach into a `bank.example` iframe. Sites that want
to share data on purpose use **CORS** headers (`Access-Control-Allow-Origin`) or
`postMessage`.

This rule explains two behaviours you will meet in automation:

- Storage belongs to the origin, so you must be **on a page of that origin**
  before you can read or write its `localStorage`.
- A cookie belongs to a domain, so a cookie for `example.com` can only be
  added while the browser can attribute it to that domain.

## Sessions and tokens

**Authentication** answers "who are you?". **Authorization** answers "what may
you do?". They are different checks, and mixing them up is how
[broken access control](/concepts/web-security#broken-access-control) happens.
The sections from here on are about the first.

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
`localStorage` can be read by any script on the page. The sections above show how cookies and storage behave.

## How a login persists

```mermaid
sequenceDiagram
    participant U as Browser
    participant S as Server
    U->>S: POST /login with user name and password
    S->>S: check the password, create a session record
    S-->>U: 200 and Set-Cookie sid=9f2c41ab7e (HttpOnly, Secure)
    Note over U: the browser stores the cookie
    U->>S: GET /inbox with Cookie sid=9f2c41ab7e
    S->>S: look up the session record for that sid
    S-->>U: your inbox
```

The password is sent once. The server hands back an opaque **session id**, the
browser attaches it to every later request, and the server looks the id up.
Some sites keep a signed token (a JWT) in a cookie or in `localStorage`
instead; a "remember me" login is a persistent cookie with a long `Max-Age`.
The server decides when the login ends: the session record can expire, be
revoked, or be tied to details such as the IP address or the browser. A saved
state that worked yesterday can therefore stop working even though the cookie is
unchanged.

Until it ends, though, **the cookie is the login**: copy it into another browser
and that browser is logged in. That is the basis of the fast test logins below.

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
its own. See [Passkeys and WebAuthn](/concepts/modern-web#passkeys-and-webauthn)
for the commands.

## Profiles, and what is in one

A <Tooltip tip="The folder where Chrome keeps cookies, storage, history and settings">browser profile</Tooltip>
is a folder on disk, the **user data directory** (`--user-data-dir`). It holds
the cookie database, `localStorage`, `IndexedDB`, the cache, saved passwords,
extensions and settings. If you start Chrome again with the same folder, it is
the same user: still logged in, same history.

Two rules follow:

- **A fresh profile is a fresh visitor.** The default for automation: every
  run starts with an empty temporary profile and the site sees a first visit.
- **One browser per profile folder.** Chrome locks the folder, so two browsers
  cannot share it at the same time.

Chrome also has two modes that never write a profile to disk: **incognito**
(a temporary profile that is thrown away) and **guest** (a clean profile with
no sign-in).

## Contexts: isolation without a second browser

A **browser context** is the DevTools Protocol's name for what Chrome shows as
an incognito window: a set of tabs with their own cookies, storage and cache,
sharing nothing with any other context in the same Chrome process. It is far
cheaper than starting another browser, which makes it the right unit for
"one clean user per test" and for running many users in parallel.

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

### Saving a signed-in state

<Steps>

<Step title="Sign in once" icon="right-to-bracket">

Run the login flow in a browser.

</Step>

<Step title="Save the state" icon="floppy-disk">

Save the cookies (and storage, where the site uses it): `save_cookies` in
`BaseCase`, `page.cookies().save(path, filter)` for a file in the pure CDP
engine, `save_session` on a pool lease for an in-memory copy.

</Step>

<Step title="Load it next time" icon="rotate">

Open the site's origin, then load the state: `load_cookies`,
`page.cookies().load(path, filter)`, or `load_session`. Cookies belong to a
domain, so the page must be on that domain first.

</Step>

</Steps>

The in-memory form is a `Session`: the cookies plus the `localStorage` entries
of one origin, built with `Session::new(cookies)` and
`with_local_storage(origin, entries)`. A `SessionStore` keeps named sessions
for as long as the process runs; it does not write to disk. Both drop what has
expired: `Session::without_expired(now)` and `SessionStore::prune_expired(now)`,
where `now` is a Unix time in seconds, the unit a `Cookie`'s `expires` field
uses.

<Warning>

Saved cookies are credentials: anyone with the file is signed in as you. Keep
them out of version control. With the `turso` feature, `ProfileVault` stores
browser profiles encrypted; see
[Results database and encrypted profiles](/tutorials/results_and_profiles).

</Warning>

## How seleniumbase-rs uses it

### Sessions in each engine

<Tabs>

<Tab title="BaseCase (WebDriver)" icon="flask">

A WebDriver session is one browser started by the driver, with one profile.
`sb.quit()` ends it. `--reuse-session` (`SB_REUSE_SESSION`) shares browsers
between the tests of a run, so later tests start where earlier ones left off.

Cookies and storage have their own methods: `get_cookie`, `add_cookie`,
`delete_cookie`, `save_cookies` and `load_cookies` to a file, and
`get_local_storage_item`, `set_local_storage_item` and their session-storage
twins. See [Cookies and storage](/api/cookies-storage).

</Tab>

<Tab title="Pure CDP" icon="bolt">

A `Page` gives direct access to its state, all through the DevTools Protocol:

| Accessor | Type | What you can do |
|---|---|---|
| `page.cookies()` | `Cookies` | `all()`, `set(&[Cookie])`, `clear()`, `header()`, `save(path, filter)`, `load(path, filter)` |
| `page.local_storage()` | `Storage` | `get(key)`, `set(key, value)`, `remove(key)`, `clear()`, `entries()` |
| `page.session_storage()` | `Storage` | The same, for `sessionStorage` |
| `page.emulation()` | `Emulation` | `timezone(id)`, `locale(..)`, `geolocation(..)`, `user_agent(..)`, `offline(..)` |

A `Cookie` is built with `Cookie::new(name, value)` and the attribute methods
`domain`, `path`, `expires`, `secure`, `http_only` and `same_site(SameSite)`,
mirroring the table above.

```rust,ignore src/main.rs
use seleniumbase_rs::sb_cdp::{Cookie, SameSite};

page.goto("https://example.com").await?; // be on the origin first
page.cookies()
    .set(&[Cookie::new("theme", "dark")
        .domain("example.com")
        .path("/")
        .secure(true)
        .same_site(SameSite::Lax)])
    .await?;
page.local_storage().set("onboarding", "done").await?;

for cookie in page.cookies().all().await? {
    println!("{} = {} (HttpOnly: {})", cookie.name, cookie.value, cookie.http_only);
}
```

One `Browser` can hold several isolated `BrowserContext`s, made with
`Browser::new_context()`; `BrowserContext::dispose()` throws everything away.
`BrowserPool` hands out one context per lease and keeps signed-in states in a
`SessionStore`: a lease saves its state under a name, and another lease loads it.

```rust,ignore src/main.rs
let worker = pool.acquire().await?;
worker.page().goto("https://example.com/login").await?;
// ... sign in ...
worker.save_session("alice").await?;
worker.release().await;

let another = pool.acquire().await?;
another.page().goto("https://example.com").await?;
another.load_session("alice").await?; // signed in, without logging in again
another.release().await;
```

See [Pure CDP](/tutorials/pure_cdp).

</Tab>

<Tab title="Launch options" icon="rocket">

On launch, `LaunchOptions::builder()` decides which profile the browser uses:
`user_data_dir(dir)` for a folder you keep, `incognito(true)` or `guest(true)`
for a profile that is not kept. With none of them the browser gets a temporary
profile folder that is removed when it closes.

</Tab>

</Tabs>

### Logins, TOTP and saved sessions

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

<Card title="Cookies and storage API" icon="cookie-bite" color="#1a85d4" href="/api/cookies-storage">
  Every BaseCase cookie and storage method.
</Card>

<Card title="Browser profiles" icon="id-card" color="#199f43" href="/tutorials/browser_profiles">
  Profiles and how their settings map to a browser.
</Card>

<Card title="Pure CDP" icon="bolt" color="#d47628" href="/tutorials/pure_cdp">
  Contexts, pools and the session store.
</Card>

<Card title="The network" icon="network-wired" color="#d52c36" href="/concepts/network">
  The requests and headers that carry cookies.
</Card>

<Card title="Detection and spoofing" icon="mask" color="#a631be" href="/concepts/detection">
  Why a consistent identity matters as much as cookies.
</Card>

<Card title="Web security" icon="lock" color="#d52c36" href="/concepts/web-security">
  SameSite, CORS credentials and the other rules that shape a login.
</Card>

<Card title="Forms" icon="rectangle-list" color="#199f43" href="/concepts/forms">
  How the login form itself works, and how tests fill it in.
</Card>

<Card title="Glossary" icon="book" color="#a631be" href="/glossary">
  Short definitions of every term used in these pages.
</Card>

</Columns>
