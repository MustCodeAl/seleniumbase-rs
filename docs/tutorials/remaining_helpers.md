---
title: "Remaining BaseCase Helpers"
---

# Remaining BaseCase Helpers

This guide covers additional `BaseCase` helpers that close the parity gap with
Python SeleniumBase. These helpers handle page artifacts, window introspection,
on-page messaging, console logs, traffic generation, charts, and deferred
assertions.

## What you will learn

- How to save screenshots and page source.
- How to inspect and manipulate the browser window.
- How to post messages and read console logs.
- How to use deferred assertions for soft failures.

## Page artifacts

```rust
let screenshot = sb.save_screenshot("home.png").await?;
let source = sb.save_page_source("home.html").await?;
```

Both methods return a `PathBuf` pointing to the saved file in the logs directory
(`latest_logs/`). The name must be a plain file name: `"../home.png"` or
`"/tmp/home.png"` is refused with an `InvalidConfig` error instead of writing
outside the logs directory. A file of the same name is replaced. For an exact
path of your own, use `save_screenshot_to_path`. Files the framework names
itself (`save_screenshot_to_logs`) never replace an earlier one.

## Reading text from the screen (OCR)

With the `ocr` feature, `seleniumbase_rs::ocr::Ocr` reads the text a page
shows from its screenshot, including text drawn on a canvas or inside an image,
which the DOM does not hold as text. It runs
[ocrs](https://github.com/robertknight/ocrs), pure Rust, so nothing has to be
installed.

```rust
use seleniumbase_rs::ocr::Ocr;

let ocr = Ocr::load().await?; // slow: load once and clone it
let text = ocr.read_page(&sb).await?; // works with an sb_cdp::Page too
assert!(text.contains("Welcome"));

let from_file = ocr.text_in_png(&std::fs::read("home.png")?).await?;
```

The text comes back one line per line of text, top to bottom. The first
`Ocr::load` downloads the two models (about 12 MB) into the cache folder
(`~/Library/Caches/seleniumbase-rs/ocr` on macOS, `~/.cache/seleniumbase-rs/ocr`
on Linux, `%LOCALAPPDATA%\seleniumbase-rs\ocr` on Windows) and checks them
against pinned SHA-256 hashes; later runs use the saved copies. On a machine
without network access, set `SB_OCR_MODELS` to a folder that holds
`text-detection.rten` and `text-recognition.rten`. Build with
`--features ocr`; `tests/ocr_chrome.rs` reads a heading from a real Chrome.

## Visual regression testing (pixels)

`check_window` is the port of Python's: it compares the page's *layout*, the
tags of the HTML and their attributes, and keeps the screenshot only for a
person to look at. A new colour, font, image or position that leaves the HTML
alone passes it. With the `visual` feature, `assert_visual_match` compares the
*pixels* instead:

```rust
sb.open("https://example.com").await?;
sb.check_window("home", 3.0).await?;     // the layout, as in Python
sb.assert_visual_match("home").await?;   // the screenshot, by structural similarity
```

The first run saves the screenshot as `visual_baseline/<test>/home.png`, where
`<test>` is the test's whole path: the test binary (the crate, or the file in
`tests/`), the module path and the function, joined with `.`, such as
`cart.checkout.renders`, so tests of the same name in other modules or files
keep separate baselines. (`check_window` names its folder after the function
alone, as Python does.) Later runs compare with it and fail with `AssertionFailed` when the
similarity is below 0.999; the failure leaves `home.actual.png` (the new
screenshot) and `home.diff.png` (the new screenshot in grey with what changed
painted red) next to the baseline. A screenshot of another size fails without
a comparison: set a fixed window size for visual tests. Run once with
`RunSettings::with_visual_baseline_reset(true)` to accept intended changes, as
for `check_window`.

For another folder or threshold, or with the pure CDP engine, use
`seleniumbase_rs::visual::VisualBaseline` directly. `check_page` takes a
`BaseCase` or an `sb_cdp::Page`, and `check` takes PNG bytes:

```rust
use seleniumbase_rs::visual::{VisualBaseline, VisualOutcome};

let baselines = VisualBaseline::new("tests/visual").with_threshold(0.9995);
match baselines.check_page("checkout", &page).await? {
    VisualOutcome::Created { baseline } => println!("saved {}", baseline.display()),
    VisualOutcome::Matched { score } => println!("matched, {score:.5}"),
    _ => {}
}
let logo = baselines.check("logo", &std::fs::read("logo.png")?)?;
```

The score is the mean structural similarity (SSIM over 8 × 8 pixel windows) of
the red, green and blue channels, the lowest of the three: 1.0 for identical
images. Because it is a mean over the whole screenshot, a small change moves it
little: in headless Chrome at 800 × 600 a rewritten 48 px heading scored 0.980
and failed, while one changed word of 16 px text scored 0.99926 and passed. To
catch changes that small, compare a screenshot of the element, or raise the
threshold. Rendering differs between machines and Chrome versions, so keep
baselines per platform. The rustdoc of `seleniumbase_rs::visual` explains the
algorithm and the threshold in full; `tests/visual_chrome.rs` runs both
engines against a real Chrome.

## Window introspection

```rust
let (x, y, width, height) = sb.get_window_rect().await?;
sb.maximize().await?;
sb.minimize().await?;
```

## On-page messaging

```rust
sb.post_success_message("Operation completed").await?;
sb.post_error_message("Something went wrong").await?;
sb.post_message_for("Transient message", 3).await?;
```

These helpers inject a visible banner into the page, useful for demos or
MasterQA reports. The banner replaces the one already showing and goes away
after the run's message duration.

## Console logs

```rust
sb.start_recording_console_logs().await?;
// Write to the browser console, as Python's console_log_string and
// console_log_script(script) do:
sb.console_log_string("Custom log entry").await?;
sb.console_log_script("document.title").await?;
let logs = sb.get_recorded_console_logs().await?;
```

## Referral / traffic helpers

```rust
// Open the blog, build a link to the shop in it and click the link.
sb.generate_referral("https://example.com/blog", "https://example.com/shop", None).await?;
// The same, ten times, and click `#add-to-cart` on the shop page each time.
sb.generate_traffic(
    "https://example.com/blog",
    "https://example.com/shop",
    10,
    Some("#add-to-cart"),
).await?;
// Click through a list of pages, one link in each page to the next one,
// starting from the page the browser is on.
sb.generate_referral_chain(&["https://example.com/a", "https://example.com/b"]).await?;
sb.generate_traffic_chain(&["https://example.com/a", "https://example.com/b"], 3).await?;
```

These are real click-throughs, as Python's are: the start page gets a link to the
destination in place of its body, the link is clicked, and the destination
receives a `Referer` header naming the start page. (Chrome's default referrer
policy sends the whole address to a page of the same origin and the origin alone
to a page of another origin.) Pass an empty string as the start page to begin on
the page the browser is on. A page that is not a valid URL, or a chain of fewer
than two pages, is refused before anything is clicked.

Earlier versions of this crate returned the URL with a `?ref=` parameter added
instead, which sends no `Referer` at all.

## Multi-series charts

```rust
sb.create_bar_chart("Sales").await?;
sb.add_data_point("Jan", 100).await?;
sb.add_data_point("Feb", 150).await?;
sb.add_series_to_chart("Last Year", &[("Jan".into(), 80), ("Feb".into(), 120)]).await?;
let path = sb.display_chart().await?;
```

## Deferred assertions

Deferred assertions check the page at once, collect the failures with the URL
of the page they saw, and only fail the test when `process_deferred_asserts` is
called:

```rust
sb.deferred_assert_element_present("#result").await?;
sb.deferred_assert_exact_text("#result", "Done").await?; // (selector, text)
sb.process_deferred_asserts().await?;
```

Use them when you want to validate many independent conditions and report all
failures at once.

## Running the example

```bash
cargo run --example remaining_helpers
```

The example demonstrates many of these helpers in sequence, and checks the
effect of each one, so a run that finishes has shown that the helper did what
its name says. It needs Chrome and a matching chromedriver, but not the network:
it opens a page it serves from `127.0.0.1` (the `examples/local_site` module that
the browser examples share). The screenshot, page source and chart land in
`latest_logs/`.
