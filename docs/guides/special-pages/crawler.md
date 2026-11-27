---
title: "Autonomous Web Crawling & Archiving"
sidebarTitle: "Web Crawling & Archiving"
description: "Scope-fenced site crawling with robots.txt compliance, resumable state, XML sitemap parsing, and ISO 28500 WARC archiving via seleniumbase-rs-plugin-crawler."
---

Crawling a site through a real browser requires scope enforcement, `robots.txt` compliance, resumable state, and bounded page/link budgets.

<Note>

Since v0.10.0, crawling, XML sitemap parsing, and WARC 1.1 archiving live in `seleniumbase-rs-plugin-crawler` inside [seleniumbase-rs-plugins](https://github.com/MustCodeAl/seleniumbase-rs-plugins) (enabled via `features = ["plugin-crawler"]` or directly as a crate dependency; see the [crawler plugin](/plugins/builtin/crawler) reference).

</Note>

`seleniumbase-rs-plugin-crawler` provides:

- **Scope-fenced `Crawler` plugin**: Breadth-first link crawler running on CDP (`sbrs plugin run crawler` or attached to a `PluginSet`) with `robots.txt` evaluation, `include`/`exclude` globs, depth/page limits, resumable `state_file`, and `on_test_end` broken-link verification.
- **XML Sitemap Discovery (`sitemap::parse_sitemap_urls`)**: Zero-dependency extraction of `<loc>` entries from `sitemap.xml` documents and sitemap indexes.
- **ISO 28500 WARC 1.1 Archiving (`warc::{WarcWriter, WarcRecordType}`)**: Streaming WARC archive writer for storing crawled HTTP requests, responses, and metadata.

<Info>

**What you will learn**

- How to configure and run `Crawler` via `sbase_config.toml` or Rust (`CrawlerConfig`, `Scope`, `Expect`).
- How to inspect `Report` for broken links, unreached targets, and console errors.
- How to parse `sitemap.xml` documents with `parse_sitemap_urls`.
- How to archive responses to WARC 1.1 files with `WarcWriter`.

</Info>

<Visibility for="agents">

Summary for AI exports: Since v0.10.0, crawling moved out of `seleniumbase-rs` core into `seleniumbase-rs-plugin-crawler` (see `/plugins/crawler`). It exports `Crawler`, `CrawlerConfig`, `Scope`, `Expect`, `Report`, `Broken`, `JsError`, `sitemap::parse_sitemap_urls`, and `warc::{WarcWriter, WarcRecordType}`. Run it from the CLI with `sbrs plugin run crawler --input crawl.json --out pages.jsonl` or register `Crawler::new(config)?` in a `PluginSet`.

</Visibility>

---

## Configuring and Running `Crawler`

### Via `sbase_config.toml` and `sbrs`

Configure crawl bounds and assertions under `[plugins.crawler]`:

```toml sbase_config.toml
[plugins]
enable = ["crawler"]

[plugins.crawler]
start = ["https://staging.example.com/"]
scope = "same-origin"
exclude = ["/logout*", "*?print=1"]
max_depth = 4
max_pages = 200
delay_ms = 500
respect_robots = true
state_file = "crawl-state.json"

[plugins.crawler.expect]
max_status = 399
must_reach = ["https://staging.example.com/pricing"]
no_console_errors = true
```

Run the crawl task and stream one JSON object per visited page to `pages.jsonl`:

```bash Terminal
sbrs plugin run crawler --input crawl.json --out pages.jsonl
```

### In Rust with `PluginSet`

Construct `Crawler` directly from `CrawlerConfig` (or `Crawler::configure(&json_value)`) and register it on `PluginSet::strict()`:

```rust
use seleniumbase_rs_plugin::PluginSet;
use seleniumbase_rs_plugin_crawler::{Crawler, CrawlerConfig, Expect, Scope};

let config = CrawlerConfig {
    start: vec!["https://staging.example.com/".into()],
    scope: Scope::SameOrigin,
    exclude: vec!["/logout*".into()],
    max_depth: 3,
    max_pages: 100,
    delay_ms: 500,
    respect_robots: true,
    expect: Expect {
        max_status: 399,
        must_reach: vec!["https://staging.example.com/pricing".into()],
        no_console_errors: true,
    },
    ..CrawlerConfig::default()
};

let crawler = Crawler::new(config)?;
let plugins = PluginSet::strict().with(crawler)?;
assert_eq!(plugins.task_names(), ["crawler"]);
```

While a crawl runs:

1. Each host's `/robots.txt` is loaded once inside the browser tab (so proxy and privacy plugins apply). Rules for `sbrs` take precedence over `*`, longest path match wins, and `Crawl-delay` is honored up to 60 seconds.
2. Each page is opened, its title, HTTP status, and `a[href]` links are collected, and `<meta name="robots" content="nofollow">` stops link expansion on that page.
3. Links outside `scope` are never queued, and `before_navigate` cancels any out-of-scope navigation.
4. When used inside a test suite, `on_test_end` fails the test with `Report::summary` if any page exceeded `expect.max_status`, a `must_reach` URL was missed, or console errors occurred while `no_console_errors = true`.

---

## XML Sitemap Discovery (`sitemap::parse_sitemap_urls`)

`seleniumbase_rs_plugin_crawler::sitemap::parse_sitemap_urls` extracts `<loc>` entries from standard XML sitemaps and sitemap indexes without pulling in an XML parser crate:

```rust
use seleniumbase_rs_plugin_crawler::sitemap::parse_sitemap_urls;

let raw_xml = r#"
    <?xml version="1.0" encoding="UTF-8"?>
    <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
        <url><loc>https://example.com/docs/intro</loc></url>
        <url><loc>https://example.com/docs/install</loc></url>
    </urlset>
"#;

let urls = parse_sitemap_urls(raw_xml);
assert_eq!(
    urls,
    vec![
        "https://example.com/docs/intro",
        "https://example.com/docs/install",
    ]
);
```

Feed discovered URLs into `CrawlerConfig::start` or a `crawl.json` input file (`{"start": [...]}`).

---

## ISO 28500 WARC 1.1 Archiving (`warc::WarcWriter`)

Archive captured crawl payloads in standard WARC 1.1 format using `WarcWriter` and `WarcRecordType`:

```rust
use seleniumbase_rs_plugin_crawler::warc::{WarcRecordType, WarcWriter};

let mut buffer = Vec::new();
let mut writer = WarcWriter::new(&mut buffer);

writer.write_record(
    WarcRecordType::Response,
    "https://example.com/",
    "text/html; charset=utf-8",
    b"<html><body><h1>Hello</h1></body></html>",
)?;
```

See the [`crawler` plugin reference](/plugins/builtin/crawler) for every option, JSONL output field, resume semantics, and safety limits.
