---
title: "Pure CDP: Pool"
sidebarTitle: "Pool"
description: "A pool of Chrome processes and isolated contexts, and a store of saved sessions."
---

<Note>

Methods are listed as `name(arguments)`. End an async call with `.await?` when it is async. `BrowserPool::close` and `Lease::release` do not return a `Result`. Argument names are explained in [Reading the API reference](/tutorials/api_reference#reading-this-page).

</Note>

A `BrowserPool` keeps a few Chrome processes and hands out a `Lease`: one isolated context and a tab. `release` discards that context (its cookies and tabs) and frees the slot. Close the pool on every path, including a failure.

```rust
use seleniumbase_rs::sb_cdp::{BrowserPool, LaunchOptions, PoolOptions};

async fn one_lease() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    let launch = LaunchOptions::builder().headless(true).build()?;
    let options = PoolOptions::builder().max_browsers(2).build()?;
    let pool = BrowserPool::new(launch, options);
    let outcome = async {
        let lease = pool.acquire().await?;
        let outcome = lease.page().goto("https://example.com").await;
        lease.release().await;
        outcome
    }
    .await;
    pool.close().await;
    outcome
}
```

## PoolOptions

`PoolOptions::builder()` starts from the defaults: 2 browsers, 4 contexts on each, retire a browser after 100 leases, and wait 60 seconds for a free slot. `build` fails with `InvalidConfig` when a count is 0 or above 256, or the acquire timeout is zero.

| Method | Description |
|--------|-------------|
| `max_browsers(count)` | Most Chrome processes at once. |
| `contexts_per_browser(count)` | Most simultaneous leases on one process. |
| `retire_after(leases)` | Replace a process after it has served this many leases. |
| `acquire_timeout(timeout)` | How long `acquire` waits for a free slot (`Duration`). |
| `build()` | Checks the values and returns `PoolOptions`. |

On a finished `PoolOptions`: `max_browsers()`, `contexts_per_browser()` and `capacity()` (browsers times contexts).

## BrowserPool

| Method | Description |
|--------|-------------|
| `new(launch, options)` | A pool that launches Chrome with `launch`. |
| `with_launcher(options, launcher)` | A pool whose `launcher` closure returns each `Browser`. The closure is `Fn() -> Fut` and the future resolves to `Result<Browser, SeleniumBaseError>`. |
| `sessions()` | The `SessionStore` leases share. |
| `acquire()` | A `Lease`, waiting up to the acquire timeout. |
| `acquire_with(context)` | A lease whose context uses these `ContextOptions`. |
| `stats()` | A `PoolStats`. |
| `close()` | Closes every browser. Does not return a `Result`. |

`PoolStats` fields are public: `browsers`, `leased`, `launched`, `retired`, `sessions` (each a `usize`).

## Lease

| Method | Description |
|--------|-------------|
| `page()` | The tab. |
| `context()` | The `BrowserContext`. |
| `browser()` | The `Browser`. |
| `sessions()` | The pool's `SessionStore`. |
| `new_page()` | Another blank page in this context. |
| `new_page_at(url)` | Another page at `url`. |
| `save_session(name)` | Saves the context's cookies, and the local storage of the current page's origin when that origin is `http`, under `name`. |
| `load_session(name)` | Loads that session: its cookies (expired ones skipped), and its local storage when the current page is on the captured origin. Returns `false`, changing nothing, when `name` is unknown. |
| `release()` | Discards the context, with its cookies and tabs, and frees the slot. Consumes the lease. Does not return a `Result`. |

## Session and SessionStore

A `Session` is cookies plus, optionally, one origin's `localStorage`. `Session::new(cookies)` starts from the cookies. `with_local_storage(origin, entries)` adds a `BTreeMap<String, String>` for that origin. `cookies()`, `origin()` and `local_storage()` read them back. `without_expired(now)` drops cookies whose `expires` is at or before `now` (Unix seconds, `f64`). A cookie with no expiry is kept.

A `SessionStore` is an in-memory map. `SessionStore::new()` is empty.

| Method | Description |
|--------|-------------|
| `save(name, session)` | Stores `session` under `name`, replacing any previous one. |
| `load(name)` | A clone of the session, or `None`. |
| `remove(name)` | Removes it and returns it, or `None`. |
| `names()` | The names, as `Vec<String>`. |
| `len()` | How many sessions. |
| `is_empty()` | Whether the store is empty. |
| `clear()` | Removes every session. |
| `prune_expired(now)` | Drops expired cookies in every session. Returns how many cookies were dropped. |
