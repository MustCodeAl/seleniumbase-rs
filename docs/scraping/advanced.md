---
title: "Advanced scraping"
sidebarTitle: "Advanced scraping"
---

[Basics](/scraping/basics) reads a page and a list of URLs. These are the
tools for whole sites and large jobs.

## Crawl a site

The `crawler` plugin walks a site breadth first, one navigation at a time,
never leaving its scope (`same-origin` by default), and follows `robots.txt`
unless told not to. Build with `plugin-crawler`, then:

```toml sbase_config.toml
[plugins.crawler]
start = ["https://example.com/"]
max_depth = 2
max_pages = 200
delay_ms = 1000
state_file = "crawl-state.json"
```

```bash
sbrs plugin run crawler --out pages.jsonl
```

Each page writes one line (its `url`, `depth`, `status`, `title` and links),
and the report lists broken links, script errors and pages it could not reach.
With `state_file` an interrupted crawl resumes. See [crawler](/plugins/crawler).

## Run jobs in batches

The `batcher` plugin runs a list of jobs in chunks, each chunk in a browser
context of its own and each job in a tab of its own, with retries, a time
limit per job and a resume file. A job is whatever its runner takes: `open`
(the default) opens a URL, and `fetcher` or `extractor` hand the job to that
plugin. Build with `plugin-batcher` (and the runner's feature), then:

```toml sbase_config.toml
[plugins.batcher]
runner = "fetcher"
concurrency = 2
retries = 1
resume_file = "batch-resume.json"
```

```json jobs.json
{"jobs": [{"url": "https://example.com/a"}, {"url": "https://example.com/b"}]}
```

```bash
sbrs plugin run batcher --input jobs.json --out results.jsonl --plugin fetcher
```

A second run with the same `resume_file` skips the jobs that finished. See
[batcher](/plugins/batcher).

## Many workers

`BrowserPool` shares a few Chrome processes between many concurrent workers.
Each `acquire()` returns a lease with its own isolated context and a tab, so
workers cannot see each other's cookies:

```rust,no_run
use seleniumbase_rs::sb_cdp::{BrowserPool, LaunchOptions, PoolOptions};

async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    let pool = BrowserPool::new(
        LaunchOptions::builder().headless(true).build()?,
        PoolOptions::builder().max_browsers(2).contexts_per_browser(4).build()?,
    );
    let worker = pool.acquire().await?; // waits its turn if the pool is full
    worker.page().goto("https://example.com").await?;
    worker.release().await;
    pool.close().await;
    Ok(())
}
```

[Pure CDP](/tutorials/pure_cdp#many-workers-the-browser-pool) has the limits,
reuse and replacement rules.

## Keep a signed-in session

Sign in once, keep the cookies and local storage, and load them in another
worker: `lease.save_session("alice")` and `lease.load_session("alice")` on a
pool, backed by the pool's in-memory `SessionStore`. Nothing is written to
disk.

## Change what pages load

`page.intercept(rules)` answers each request by the first matching `Rule`:
block images and fonts to save bandwidth, serve a made-up response, or send a
request on with changes ([Intercepting requests](/tutorials/pure_cdp#intercepting-requests)).
The `fetcher` plugin's `strip` and `block` options do the same from
configuration.

## Next

When a site starts refusing the browser, go on with
[Stealthy scraping](/scraping/stealthy/overview).
