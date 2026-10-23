---
title: "How web security works: attacks and browser policies"
sidebarTitle: "Web security"
description: "The OWASP Top 10 in plain words (injection, cross-site scripting, CSRF, clickjacking, SSRF, broken access control, vulnerable dependencies), the rules a browser enforces between websites (origins, the same-origin policy, CORS, Content Security Policy, Permissions-Policy, Referrer-Policy, COOP/COEP/CORP, mixed content, HSTS and SameSite cookies), security headers, testing only what you are authorized to test, and how seleniumbase-rs keeps its own dependencies and secrets safe."
---

<Info>

**In short:** most web attacks are not clever. They are a site trusting
something it should have checked: text typed into a form, a number in a URL, a
request that "looks like" it came from the user, a library nobody updated. This
page names the common ones in plain words, then explains the rules a browser
enforces between sites (the same-origin policy, CORS, CSP and friends) that stop
or limit them. When a test fails with a "blocked" message in the console, one of
those rules is nearly always the reason. It shows where a test can catch a
problem, and says clearly where testing stops and permission starts.

</Info>

<Warning>

**Only test systems you are authorized to test.** Your own application, a test
environment you were given, or a target whose owner has said in writing that
you may. Sending attack-shaped input to someone else's site is a crime in many
countries, even if you "only looked". Browser automation makes it easy to send
thousands of requests; that is a reason for more care, not less.

</Warning>

## The OWASP Top 10

**OWASP** is a non-profit that publishes a ranked list of the most common web
application risks, the **Top 10**. It is the usual starting vocabulary for
security talks and audits. The 2025 edition names these:

| # | Name | In plain words |
|---|---|---|
| A01 | Broken access control | A user can do or see something they should not |
| A02 | Security misconfiguration | Default passwords, debug pages, open storage, missing headers |
| A03 | Software supply chain failures | A library, tool or build step you depend on is vulnerable or hostile |
| A04 | Cryptographic failures | Secrets sent or stored without proper encryption |
| A05 | Injection | Input is treated as a command: SQL, shell, script |
| A06 | Insecure design | The feature is built so it cannot be made safe |
| A07 | Authentication failures | Weak logins, guessable resets, sessions that never end |
| A08 | Software or data integrity failures | Updates, plugins or data accepted without checking where they came from |
| A09 | Logging and alerting failures | An attack happens and nobody notices |
| A10 | Mishandling of exceptional conditions | Errors that leak secrets or leave the system open |

The sections below go through the ones a browser test touches most.

## Injection

**Injection** happens when a program builds a command out of text it received
and the text contains command syntax. The classic case is SQL.

```sql
-- The code builds this text from what the user typed in the "name" box:
SELECT * FROM users WHERE name = '<typed text>' AND active = 1;

-- A user types:  x' OR '1'='1
SELECT * FROM users WHERE name = 'x' OR '1'='1' AND active = 1;
-- The condition is now always true, so every user is returned.
```

The fix is to keep data and command apart: a **parameterized query** sends the
command and the values separately, so a value can never become syntax. The same
rule holds for shell commands (pass an argument list, not one long string), for
templates, and for any interpreter.

## Cross-site scripting (XSS)

**XSS** is injection into the *browser*. An attacker gets their JavaScript into
a page that other people view, and it runs with that site's rights: it can read
the page, send requests as the user and steal anything not marked `HttpOnly`.

- **Stored XSS**: the script is saved by the site (a comment, a profile name) and
  shown to every visitor.
- **Reflected XSS**: the script is in a link, and the site echoes it back in the
  page.
- **DOM-based XSS**: the page's own JavaScript writes untrusted text into the
  document with `innerHTML` or similar, and no server is involved.

Defences, from the inside out: **encode output** for its context (a name shown
in HTML is escaped as text, never inserted as markup); use APIs that insert text
(`textContent`); mark session cookies `HttpOnly`; and add a
[Content Security Policy](#content-security-policy) as a second wall.

A safe test types a harmless marker such as `"><i id="xss-marker">` into a field
and checks that the page shows it **as text**, not as an element. If a lookup of
`#xss-marker` finds an element, the input was inserted as markup.

## Cross-site request forgery (CSRF)

Browsers attach your cookies to every request to a site, whoever started the
request. **CSRF** abuses that: a hostile page makes your browser send a request
to a site where you are logged in.

```mermaid
sequenceDiagram
    participant U as You, logged in to bank.example
    participant E as Page on evil.example
    participant K as bank.example
    U->>E: Open a harmless-looking page
    E-->>U: Page with a hidden form that posts to the bank
    U->>K: Browser sends the POST with the bank cookie attached
    K-->>U: Transfer done. The attacker cannot read the answer, but it happened
```

*The same-origin policy hides the answer, not the request.*

Defences: a **CSRF token** (a secret per session, put in each form and checked
by the server, which a hostile page cannot read); `SameSite` cookies, which
leave the cookie off cross-site requests; and checking the `Origin` header on
requests that change data. See [forms](/concepts/forms) for how a test carries a
token, and the [SameSite section](#samesite-cookies) for `SameSite`.

## Clickjacking

In **clickjacking** the attacker frames your site in an invisible iframe on top
of a decoy button ("Win a prize") so that the victim's click lands on your
"Delete account" button.

The defence is to forbid framing: the header `Content-Security-Policy:
frame-ancestors 'none'` (or `'self'`), or the older `X-Frame-Options: DENY`. A
test can request the page and assert the header is there, or try to load it in
an iframe from another origin and check that the frame stays empty.

## Server-side request forgery (SSRF)

If a feature makes the **server** fetch a URL you give it (an avatar from a
link, a webhook, a "preview this page" tool), an attacker can give it an address
only the server can reach: an internal admin panel, or the cloud provider's
metadata address (`169.254.169.254`) that hands out credentials. The server
becomes the attacker's browser inside your network.

Defences: an allow-list of hosts, refusing private and link-local address
ranges (after resolving the name, and again after each redirect), and giving
the server no more network reach than it needs. In the 2025 Top 10 SSRF is part
of broken access control.

## Broken access control

Logging in proves who you are. **Access control** decides what you may do. It
breaks when the server trusts the client for that decision.

- **IDOR** (insecure direct object reference): `GET /invoices/1001` shows your
  invoice, and changing the number to `1002` shows someone else's.
- Hidden admin pages that are not linked but not protected either.
- A button hidden in the interface while the endpoint behind it accepts anyone.

This is the risk where browser tests earn their keep. Log in as an ordinary
user, then request an admin URL, or another user's record, and assert the
answer is `403` or `404`. Do the same logged out and expect a redirect to the
login page. See [logging in fast in tests](/concepts/sessions-and-state#logging-in-fast-in-tests)
for fast logins as different users.

## Vulnerable dependencies and the supply chain

Modern software is mostly other people's code. Your application pulls in
libraries, which pull in more; your build uses plugins and scripts; your tests
use drivers and browsers. Each is a place an attack can enter. The risks:

- A **known vulnerability** in a library you have not updated. The fix exists,
  you did not apply it.
- A **hostile or hijacked package**: a maintainer account is taken over, or a
  look-alike name (typosquatting) is published and installed by mistake.
- **Build-time code**: install scripts and build scripts run on your machine
  or CI with your secrets in reach.
- An **unmaintained** library that will never get a fix.

What helps: commit the lockfile and install only from it; pin versions on
purpose and review upgrades; run an **advisory scanner** that compares your
lockfile with a database of known problems; limit the **licences** and
**sources** you accept; remove dependencies you no longer use; and give CI jobs
only the secrets they need.

## Browser security policies

The attacks above are stopped, or limited, by rules that the browser enforces
between websites and that servers tighten or relax with response headers. They
are explained from the ground up: first what an origin is, then the same-origin
policy and the headers that adjust it.

### Origins: the unit of trust

An **origin** is three things together: the scheme, the host and the port.

| URL | Same origin as `https://app.example.com/home`? | Why |
|---|---|---|
| `https://app.example.com/other` | Yes | Only the path differs |
| `http://app.example.com/home` | No | Different scheme |
| `https://api.example.com/home` | No | Different host |
| `https://app.example.com:8443/home` | No | Different port |

A **site** is looser: the scheme plus the registrable domain, so `app.example.com`
and `api.example.com` are the same *site* but different *origins*. The
difference matters: CORS and the same-origin policy work on origins, while
SameSite cookies and site isolation work on sites.

A test that opens its page on `http://localhost:3000` and calls an API on
`http://localhost:8080` has two origins, because the ports differ. This is the
commonest source of CORS errors in test setups.

### The same-origin policy

The **same-origin policy** is the browser's oldest rule: a page may freely use
resources from its own origin, but code from one origin may not **read** data
from another.

- Blocked: reading the response of a `fetch` or `XMLHttpRequest` to another origin,
  reading the document of a cross-origin iframe, reading another origin's cookies
  or `localStorage`.
- Allowed: *embedding* a cross-origin image, stylesheet, script or iframe, and
  *sending* a form to another origin. The page just cannot look at the answer.

That last point is the key to understanding web security. The policy hides
answers, it does not stop **requests** from being sent. A hostile page can make
your browser send a request to your bank, with your bank cookies attached. The
answer is hidden from the attacker, but the action may already have happened.
That is a CSRF attack, covered in [CSRF](#cross-site-request-forgery-csrf) above, and
the reason for `SameSite` cookies later on this page.

### CORS: asking for a bigger window

**Cross-Origin Resource Sharing** (CORS) lets a server say "this other origin is
allowed to read my answers". The server does it with response headers; the
browser does the checking. The server never learns whether the browser accepted
the answer.

There are two paths, depending on how risky the request looks.

#### Simple requests

A request is **simple** when it could already have been made by an old-fashioned
HTML form: the method is `GET`, `HEAD` or `POST`, it adds no unusual headers and, for
`POST`, the body type is a form type or plain text. The browser sends it at once,
adds an `Origin` header, and then checks the answer's
`Access-Control-Allow-Origin` header before giving the page access to it.

#### Preflighted requests

Anything else (a `PUT` or `DELETE`, a JSON body, a custom header such as
`Authorization`) is **preflighted**. Before the real request the browser sends a
safe question, an `OPTIONS` request, to ask permission.

```mermaid
sequenceDiagram
    participant P as Page on app.example.com
    participant B as Browser
    participant S as API on api.example.com
    P->>B: fetch with PUT and a JSON body
    B->>S: OPTIONS preflight with Origin and Access-Control-Request-Method
    S-->>B: 204 with Allow-Origin, Allow-Methods, Allow-Headers and Max-Age
    Note over B: Allowed, so the real request may go
    B->>S: PUT with the JSON body
    S-->>B: 200 with Access-Control-Allow-Origin
    B-->>P: the response, now readable
```

*A preflighted request. If the preflight answer does not allow the request, the
real one is never sent.*

The headers involved:

| Header | Direction | Meaning |
|---|---|---|
| `Origin` | Request | Who is asking |
| `Access-Control-Request-Method` and `-Headers` | Preflight request | What the real request will use |
| `Access-Control-Allow-Origin` | Response | The one origin allowed, or `*` for anyone |
| `Access-Control-Allow-Methods` and `-Headers` | Preflight response | Which methods and headers are permitted |
| `Access-Control-Allow-Credentials` | Response | `true` lets the page send and read cookies; the origin can then not be `*` |
| `Access-Control-Max-Age` | Preflight response | How many seconds the browser may remember the answer |
| `Access-Control-Expose-Headers` | Response | Extra response headers the page may read |

<Accordion title="The mistakes that cause most CORS failures">

- **Wildcard with credentials.** `Access-Control-Allow-Origin: *` is refused when
  the request carries cookies. The server must name the exact origin and send
  `Vary: Origin`, so that a cache does not hand one origin's answer to another.
- **Preflight answered with an error.** The `OPTIONS` request often never reaches
  the application's routes, so it gets a `404`, a `401` or a redirect, and the
  real request is never made. The console shows a CORS error although the real
  problem is the server.
- **Header missing from the allow list.** Adding an `Authorization` or custom
  header to the call without adding it to `Access-Control-Allow-Headers`.
- **Redirects.** A redirect on a preflighted request is refused.

</Accordion>

<Tip>

CORS is a **browser** rule. A command such as `curl`, or an HTTP client library, is
never blocked by it. When a call works in your HTTP client and fails in the
browser, CORS is the first thing to suspect. It also means a test that only
calls the API directly will not catch a missing CORS header: the page itself
has to make the call.

</Tip>

### Content Security Policy

A **Content Security Policy** (CSP) is a header that tells the browser which
sources a page may load and run from. It is the strongest defence against
**cross-site scripting** (XSS), where an attacker gets their script into your
page, because even if the script is injected, the browser refuses to run it.

```http
Content-Security-Policy: default-src 'self'; script-src 'self' 'nonce-r4nd0m'; img-src 'self' https:; frame-ancestors 'none'; report-to csp
```

| Directive | Controls |
|---|---|
| `default-src` | The fallback for every resource type not listed |
| `script-src`, `style-src` | Where scripts and styles may come from |
| `img-src`, `font-src`, `media-src` | Images, fonts, audio and video |
| `connect-src` | Where `fetch`, `XMLHttpRequest`, WebSocket and EventSource may connect |
| `frame-src`, `frame-ancestors` | Which pages may be framed, and who may frame this one |
| `form-action`, `base-uri` | Where forms may post and what `base` may be set to |

A source is a host (`https://cdn.example.com`), a scheme (`https:`) or a keyword
in quotes: `'self'` (the page's own origin), `'none'`, `'unsafe-inline'`
(allow any inline script or style, which removes most of the protection) and
`'unsafe-eval'` (allow `eval`).

**Nonces and hashes** are how a strict policy still allows the page's own inline
code. The server makes a fresh random value for each response, puts it in the
header (`'nonce-r4nd0m'`) and in the page's own script elements
(`script nonce="r4nd0m"`). An injected script does not know the nonce, so it is
blocked. A `'sha256-...'` source allows one inline script with exactly that
hash. With `'strict-dynamic'`, a script that has a valid nonce may load further
scripts.

**Report-only mode.** The header `Content-Security-Policy-Report-Only` applies the
same policy without enforcing it. The browser runs everything and *reports*
what would have been blocked to a URL named in `report-to` (or the older
`report-uri`). Teams use it to try a new policy on live traffic before they turn
it on.

#### Why injected scripts can be blocked

Automation tools often add code to the page: a script element for highlighting,
a recorder overlay, an `eval` call. Under a strict CSP, the browser applies the
policy to that code like any other, and may refuse it. The console then shows
"Refused to execute inline script because it violates the following Content
Security Policy directive".

Scripts a tool sends through the debugging protocol or the driver
([CDP](/concepts/cdp), [WebDriver](/concepts/webdriver)) are evaluated by the
browser as a debugger would, and are **not** subject to the page's policy. The
difference is between *evaluating* a script in the page and *adding* an element
or calling `eval` from page code. If a helper stops working only on sites that
send a strict CSP, this is the first thing to look at.

### Permissions-Policy

**Permissions-Policy** (earlier called Feature-Policy) switches whole browser
features on or off for a page and for the frames inside it:

```http
Permissions-Policy: camera=(), geolocation=(self), microphone=(self "https://meet.example.com")
```

An empty list means "no origin", `self` means the page's own origin. A frame can
use a feature only if the policy allows it *and* the embedding page's `allow`
attribute on the iframe grants it. It is separate from the user's *permission
prompt* (the "allow location?" dialog): the policy decides whether the prompt may
even appear.

In a test, an unexpected permission prompt blocks the page until it is answered.
The DevTools Protocol can grant or deny permissions up front
(`Browser.grantPermissions`), so a run does not wait on a dialog.

### Referrer-Policy

When you follow a link, the browser tells the next site where you came from, in
the `Referer` header (the spelling error is historical). **Referrer-Policy**
decides how much of that address to send: `no-referrer` sends nothing,
`same-origin` only to the same site, and `strict-origin-when-cross-origin` (the
default) sends the full address within a site but only the origin to others, and
nothing when going from https to http.

Why tests care: some servers refuse a request that arrives without the expected
`Referer` (hotlink protection, some login redirects). A test that opens a deep
link directly sends none, which is a different path from clicking through.

### COOP, COEP and CORP: cross-origin isolation

Three headers work together to put a page in its own safe group, so that it can
use powerful features such as `SharedArrayBuffer` and high-resolution timers.

| Header | Set on | Effect |
|---|---|---|
| `Cross-Origin-Opener-Policy` (COOP) | The page | `same-origin` cuts the link (`window.opener`) between the page and windows opened from other origins |
| `Cross-Origin-Embedder-Policy` (COEP) | The page | `require-corp` refuses to load any cross-origin resource that has not opted in |
| `Cross-Origin-Resource-Policy` (CORP) | A resource | `same-origin`, `same-site` or `cross-origin`: who may load this file |

When a page sets COOP `same-origin` and COEP `require-corp`, the browser calls it
**cross-origin isolated** and exposes the strong features. Tests meet it as:
popups that lose `window.opener` (so a flow that depends on the opener breaks),
and images or scripts from a CDN that stop loading because they lack a CORP
header.

### Mixed content and HSTS

**Mixed content** is an https page that loads something over plain http. An
attacker on the network can change an http script and so take over the secure
page. Browsers therefore **block** active mixed content (scripts, frames,
fetches) and upgrade or warn about passive content such as images. The header
`Content-Security-Policy: upgrade-insecure-requests` asks the browser to
rewrite every http link on the page to https.

**HSTS** (HTTP Strict Transport Security) is a header on an https response that
tells the browser "only ever use https for this host":

```http
Strict-Transport-Security: max-age=31536000; includeSubDomains; preload
```

For `max-age` seconds the browser rewrites `http://` to `https://` before
sending anything, and it will not let the user click through a certificate error
for that host. For tests: a host that has sent HSTS once keeps forcing https in
that browser profile, so a staging site switched back to plain http seems broken
until the profile is new. `localhost` is treated as a secure context and needs
neither. A fresh profile per run avoids the stale rule; see
[sandboxing and isolation](/concepts/browser-architecture#site-isolation-and-the-sandbox).

### SameSite cookies

The `SameSite` attribute decides whether a cookie is sent on requests that
begin on **another site**.

| Value | Sent on cross-site requests? | Typical use |
|---|---|---|
| `Strict` | Never | Banking sessions |
| `Lax` (the default when the attribute is missing) | Only for top-level navigations with a safe method, such as clicking a link | Ordinary logins |
| `None` | Always, but the cookie must also be `Secure` | Embedded widgets, single sign-on across sites |

`Lax` as the default is what stops the classic CSRF attack: a hostile page can
still send a form `POST`, but your session cookie no longer goes with it. For
tests it explains why a login that works in a direct visit is lost when the page
is loaded inside an iframe, or after a cross-site redirect with a `POST`. See
[sessions, cookies and storage](/concepts/sessions-and-state) for the cookie
attributes `Secure`, `HttpOnly` and the `__Host-` prefix.

### What each rule means for a test

| You see | Likely rule | First thing to try |
|---|---|---|
| "blocked by CORS policy" | CORS | Check the preflight (`OPTIONS`) answer and the allowed origin |
| "Refused to execute inline script" | CSP | Look at the `Content-Security-Policy` response header |
| An iframe stays blank | CSP `frame-ancestors` or `X-Frame-Options` | See which header the framed page sends |
| A popup has no opener | COOP | Look for `Cross-Origin-Opener-Policy` |
| An image or script does not load on an https page | Mixed content or CORP | Open the console for the exact message |
| Login lost after a redirect | SameSite | Check the cookie's `SameSite` and `Secure` attributes |
| Page forced to https | HSTS | Use a fresh profile |

## Security headers

Many protections are a single response header. A test can read the headers of a
page and assert the set you expect.

| Header | What it gives you |
|---|---|
| `Content-Security-Policy` | Limits where scripts and other content may come from; also `frame-ancestors` |
| `Strict-Transport-Security` | Forces https for a host |
| `X-Content-Type-Options: nosniff` | Stops the browser guessing a file's type, so an uploaded text file is not run as a script |
| `Referrer-Policy` | Limits the address sent to the next site |
| `Permissions-Policy` | Switches off browser features the page does not need |
| `Cache-Control: no-store` | Keeps private pages out of caches, including the back button |
| `Set-Cookie` with `Secure`, `HttpOnly`, `SameSite` | Protects the session cookie |

Details of the first five are in [the browser policies above](#browser-security-policies).

## Testing security with a browser

A browser-driven suite is not a penetration test, but it is a good place for
**regression checks** of the rules you already know about:

<Steps>

<Step title="Headers and cookies" icon="list-check">

Load the main pages and assert the headers above, and that the session cookie
is `Secure` and `HttpOnly`.

</Step>

<Step title="Access rules" icon="user-lock">

For each protected page, check the answer as logged out, as an ordinary user and
as an admin.

</Step>

<Step title="Input handling" icon="keyboard">

Put harmless marker text with markup characters through each form and check it
is shown as text.

</Step>

<Step title="Sign-out and expiry" icon="right-from-bracket">

After signing out, check that the back button and a saved URL do not show
private data.

</Step>

</Steps>

For finding unknown problems use a dedicated scanner and a person who knows
the work, against a test environment. Keep the traffic away from production
data, and agree the scope and times with the owner first.

## How seleniumbase-rs uses it

### Dependencies, secrets and files

The library is a testing tool and not a scanner: it has no feature that attacks a
site. What it has is care for its **own** supply chain and for the secrets that
pass through it.

<Tabs>

<Tab title="Dependencies: deny.toml" icon="box">

The file `deny.toml` configures `cargo deny check`, which the project runs with
`cargo audit` as part of its checks. It has four parts:

- **Advisories**: known vulnerabilities fail the check. A small `ignore` list
  exists, and each entry carries a written reason (for example "reached only
  through an optional, off-by-default feature") and what upstream fix to wait
  for.
- **Licenses**: an allow-list of permissive licences (MIT, Apache-2.0, BSD and
  similar). Anything else fails.
- **Bans**: `wildcards = "deny"` refuses a dependency declared as "any version",
  and `multiple-versions = "warn"` flags two copies of one crate.
- **Sources**: `unknown-registry = "deny"` and `unknown-git = "deny"`; only one
  Git repository (the plugin API) is allowed, pinned to a revision.

</Tab>

<Tab title="Secrets in logs" icon="eye-slash">

- `sb_cdp::Session` and `SessionStore` (saved logins) print **counts**, never a
  cookie or storage value, in their `Debug` output. The module's own doctest
  asserts that a secret cookie value is not in the text.
- A proxy address is shown as `<redacted>` in the launcher's debug output.
- The MCP `dev` server's environment report passes the settings it shows
  through `redact_setting`, so proxy credentials appear as `[redacted]@host:port`.
- The page-saving tool of the MCP DevTools blanks the values of form inputs in
  the HTML it writes, and reports how many it blanked (`redacted_inputs`).
- Proxy credentials go to the **proxy** only: the pure CDP engine answers a
  password prompt with them only when its source is the proxy, and hands a
  website's own `401` prompt back to the browser.
- `sbrs encrypt` reads its passphrase from an environment variable and not from
  an argument, so it stays out of shell history. See
  [encryption](/concepts/encryption).

</Tab>

<Tab title="Files an assistant may write" icon="folder-closed">

The MCP tools write only inside the output folder, and refuse absolute paths
and `..`. See [sandboxing and isolation](/concepts/browser-architecture#site-isolation-and-the-sandbox).

</Tab>

</Tabs>

<Note>

Rust removes a whole class of bugs (use of freed memory, buffer overflows) from
the library itself, but not injection, access control or a stolen password. A
safe language is one layer of the defence, not the whole of it.

</Note>

### Content Security Policy and permissions

The library does not weaken CORS or the same-origin policy. It can, on request,
remove the Content Security Policy, which is the one that most often gets in the
way of an injected helper.

- **Disable CSP.** `BrowserConfig::with_disable_csp(true)` is the launch switch
  `--disable-csp` (also written `--dcsp` or `--no-csp`; `disable_csp`, `dcsp` and
  `no_csp` in the Python-style names). The doc comment says it strips the Content
  Security Policy from every response, Chromium only. It is done with a bundled
  browser extension (see [browser extensions](/concepts/browser-extensions));
  for Firefox the same switch sets the preference `security.csp.enable` to false.
- **Stealth policy.** The stealth layer has a `disable_csp` option in its policy
  (`Fingerprint::disable_csp(bool)`) and a `grant_permissions` option that grants
  the browser's permission prompts ahead of time. See
  [detection and spoofing](/concepts/detection).
- **Reading and rewriting headers.** Network interception in the pure CDP engine
  can look at and change responses, so a test can add a CORS header to a
  response or drop a framing header. See [network](/concepts/network).

<Warning>

Turning CSP off makes the page behave differently from production. A test that
passes with CSP disabled can hide the very blocked-script bug your users will
see. Use it for tools that must inject code into pages you do not control, and
run your own application's tests with its real policy on.

</Warning>

## Where to go next

<Columns cols={2}>

<Card title="Encryption" icon="key" color="#d52c36" href="/concepts/encryption">
  Hashing, encryption, TLS and keeping secrets out of test code.
</Card>

<Card title="Sessions and logging in" icon="user-lock" color="#199f43" href="/concepts/sessions-and-state">
  Logins, tokens, MFA and fast test logins.
</Card>

<Card title="Glossary" icon="book" color="#a631be" href="/glossary">
  Short definitions of every term used in these pages.
</Card>

<Card title="Network" icon="network-wired" color="#a631be" href="/concepts/network">
  Seeing and changing the requests and headers a page sends.
</Card>

<Card title="Browser architecture" icon="sitemap" color="#199f43" href="/concepts/browser-architecture">
  The sandbox and site isolation that protect the user from a page.
</Card>

</Columns>
