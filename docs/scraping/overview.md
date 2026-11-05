---
title: "Scraping"
sidebarTitle: "Overview"
---

Scraping is reading data out of pages: loading them, finding the parts you
want and keeping them, often for many pages at once. `seleniumbase-rs` gives
you a real browser to do it with, and plugins that do the common jobs for you.
When a site resists automated clients, the same jobs run with stealth. Stealth
is one aspect of scraping: the stealthy scraping pages live in the Stealth tab,
under [Advanced stealth](/stealth/advanced/overview), and link to the
[Stealth](/stealth/introduction) sections instead of repeating them.

<Note>

Scrape what you may: read a site's terms and its `robots.txt`, keep the rate
low, and do not collect personal data you have no right to. The `crawler`
plugin follows `robots.txt` by default.

</Note>

<Visibility for="agents">

Summary for AI exports: scraping in seleniumbase-rs uses the pure CDP engine (`sb_cdp::Page`, `Locator::all_waiting`, `text`, `attribute`, `Page::content`) and four built-in plugins on it: `fetcher` (fetch a list of URLs as the signed-in browser, record responses), `extractor` (structured data without selectors), `crawler` (walk a site within a scope) and `batcher` (many jobs with retries and a resume file), each run with `sbrs plugin run <name> --input FILE`. `BrowserPool` runs many isolated workers. The stealth repository (seleniumbase-rs-stealth) adds `BrowserScraper`, `HttpSession` and `StealthSession` for sites that block automated clients. Sections: Basics, Advanced scraping, Stealthy scraping, Advanced stealthy scraping.

</Visibility>

## Pick a tool

| You want to | Use | Read |
|---|---|---|
| Read a few pages with your own code | the pure CDP engine: `Page` and `Locator` | [Basics](/scraping/basics#read-a-page-with-your-own-code) |
| Fetch a list of URLs as the signed-in browser would | the `fetcher` plugin | [Basics](/scraping/basics#fetch-a-list-of-urls) |
| Read structured data without writing selectors | the `extractor` plugin | [Basics](/scraping/basics#read-structured-data) |
| Walk a whole site | the `crawler` plugin | [Advanced scraping](/scraping/advanced#crawl-a-site) |
| Run thousands of jobs with retries and a resume file | the `batcher` plugin | [Advanced scraping](/scraping/advanced#run-jobs-in-batches) |
| Run many isolated workers at once | `BrowserPool` | [Advanced scraping](/scraping/advanced#many-workers) |
| Scrape a site that blocks automated browsers | `BrowserScraper` (stealth repository) | [Stealthy scraping](/scraping/stealthy/overview) |
| Plain HTTP that looks like Chrome on the wire | `HttpSession` (stealth repository) | [HttpSession](/scraping/stealthy/http-session) |
| One identity moving between a browser and HTTP | `StealthSession` (stealth repository) | [Advanced stealthy scraping](/scraping/advanced-stealthy/overview) |

## The sections

- **[Basics](/scraping/basics)**: read a page with `Page` and `Locator`, fetch
  a list of URLs, read structured data, record what a page loads.
- **[Advanced scraping](/scraping/advanced)**: crawl a site, run jobs in
  batches, many workers, change what pages load, keep a signed-in session.

In the Stealth tab, under Advanced stealth:

- **[Stealthy scraping](/scraping/stealthy/overview)**: the stealth
  repository's `BrowserScraper` and `HttpSession`, and a first fetch.
- **[Advanced stealthy scraping](/scraping/advanced-stealthy/overview)**:
  `StealthSession`, identity files and challenge detection, examples, limits.

## Two repositories

The browser and the four plugins are part of this crate: the plugins are
built in, each behind its own cargo feature (`plugin-fetcher`,
`plugin-extractor`, `plugin-crawler`, `plugin-batcher`). `BrowserScraper`,
`HttpSession`, `StealthSession` and `MitmProxy` are in
[seleniumbase-rs-stealth](https://github.com/MustCodeAl/seleniumbase-rs-stealth),
which builds on this crate's public API; depend on it only when a site needs
it ([Which crate to depend on](/scraping/stealthy/overview#which-crate-to-depend-on)).
