---
title: "Scraping basics"
sidebarTitle: "Basics"
---

A browser runs the page's own scripts before you read it, so content that
JavaScript draws is there too. That is the reason to scrape with one rather
than with plain HTTP. This page reads pages with the pure CDP engine, then
hands the common jobs to the built-in plugins.

## Read a page with your own code

```rust,no_run
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};

async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    let browser = Browser::launch(LaunchOptions::builder().headless(true).build()?).await?;
    let page = browser.default_page().await?;
    page.goto("https://example.com/products").await?;

    for item in page.locator(".product").all_waiting().await? {
        let name = item.locator(".name").text().await?;
        let link = item.locator("a").attribute("href").await?;
        println!("{name}: {link:?}");
    }

    let html = page.content().await?; // the whole document, for a parser of your own
    println!("{} bytes", html.len());
    browser.close().await?;
    Ok(())
}
```

- `all_waiting()` waits up to the locator's timeout for a first match, then
  returns one locator per match, as Python's `find_elements` does. It gives an
  empty list when nothing appears in time.
- `text()` reads the rendered text (and the value of a form field), `html()`
  the outer HTML, `attribute(name)` an attribute or `None`.
- `page.content()` returns the whole document.

[Pure CDP](/tutorials/pure_cdp) covers locators, frames, cookies and waiting
in full.

## Fetch a list of URLs

The `fetcher` plugin fetches each URL from inside one tab, with the cookies
the browser holds for each site, as the signed-in browser would. Build with
the `plugin-fetcher` feature, then:

```json urls.json
{"urls": ["https://example.com/a", "https://example.com/b"], "delay_ms": 1000}
```

```bash
sbrs plugin run fetcher --input urls.json --out pages.jsonl
```

Each URL writes one line, `{index, url, status, mime, text, truncated}` or
`{index, url, error}`, and the summary goes to standard error. Options such as
`strip` (resource kinds to fail, to save bandwidth) and `block` (address
patterns to fail) go in `[plugins.fetcher]`; see [fetcher](/plugins/fetcher).

## Read structured data

The `extractor` plugin reads data without selectors: with nothing else asked,
it finds the structure of the page (JSON-LD, then tables, then repeated
elements) and reads it. Build with `plugin-extractor`, then:

```json pages.json
{"urls": ["https://example.com/products"]}
```

```bash
sbrs plugin run extractor --input pages.json --out records.jsonl
```

Each page writes `{index, url, title, schema, records, problems}`. A job can
also name what to read (`select`, `item`, `schema`); see
[extractor](/plugins/extractor).

## Record what a page loads

Many pages fetch their data as JSON. The `fetcher` plugin's `capture` option
records the responses of the addresses you name (`capture = ["*/api/*"]`), and
Rust code reads them with `Fetcher::captured()` or `take_captured()`. To keep
everything a tab loads, record a HAR file
([Recording a HAR](/tutorials/pure_cdp#recording-a-har)).

## Next

- [Advanced scraping](/scraping/advanced): crawl, batch, many workers.
- [Stealthy scraping](/scraping/stealthy/overview): when a site blocks
  automated browsers.
