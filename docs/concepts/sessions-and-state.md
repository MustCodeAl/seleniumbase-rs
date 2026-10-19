---
title: "How sessions, cookies and storage work"
sidebarTitle: "Sessions, cookies and storage"
description: "What a browser session holds, where cookies and web storage live, and how seleniumbase-rs saves and restores a signed-in state."
---

<Info>

**In short:** a site remembers you through state the browser keeps: cookies,
`localStorage` and `sessionStorage`. That state lives in a browser profile. Keep
the profile, or save the state and load it into another browser, and the site
sees the same signed-in user without a new login.

</Info>

## What the browser keeps

| State | Lives in | Sent to the site | Lasts |
|---|---|---|---|
| Cookies | The profile, per domain | With every matching request | Until they expire or are deleted |
| `localStorage` | The profile, per origin | Never; read by page script | Until deleted |
| `sessionStorage` | One tab, per origin | Never; read by page script | Until the tab closes |

A <Tooltip tip="The folder where Chrome keeps cookies, storage, history and settings">profile</Tooltip>
is a folder on disk (`--user-data-dir`). A fresh profile, the default, means a
fresh visitor every run.

## Sessions in each engine

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

One `Browser` can hold several isolated `BrowserContext`s, each like a separate
private window with its own cookies and storage. `BrowserPool` hands out pages
from a few browsers and keeps signed-in states in a `SessionStore`: a lease saves
its state under a name, and another lease loads it.

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

</Tabs>

## Saving a signed-in state

<Steps>

<Step title="Sign in once" icon="right-to-bracket">

Run the login flow in a browser.

</Step>

<Step title="Save the state" icon="floppy-disk">

Save the cookies (and storage, where the site uses it): `save_cookies` in
`BaseCase`, `save_session` on a pool lease.

</Step>

<Step title="Load it next time" icon="rotate">

Open the site's origin, then load the state: `load_cookies`, or `load_session`.
Cookies belong to a domain, so the page must be on that domain first.

</Step>

</Steps>

<Warning>

Saved cookies are credentials: anyone with the file is signed in as you. Keep
them out of version control. With the `turso` feature, `ProfileVault` stores
browser profiles encrypted; see
[Results database and encrypted profiles](/tutorials/results_and_profiles).

</Warning>

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

<Card title="Detection and spoofing" icon="mask" color="#a631be" href="/concepts/detection">
  Why a consistent identity matters as much as cookies.
</Card>

</Columns>
