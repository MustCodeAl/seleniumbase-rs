---
title: "Remaining BaseCase Helpers"
sidebarTitle: "More helpers"
---

export const LayoutTree = ({ children }) => (
  <div className="not-prose layout-tree" style={{ margin: "1.25rem 0", fontFamily: "var(--font-sans)", fontSize: "14px", color: "var(--text-secondary)" }}>
    <style>{`
      .layout-tree summary { list-style: none; }
      .layout-tree summary::-webkit-details-marker { display: none; }
      .layout-tree .lt-row:hover { background-color: var(--surface-subtle); }
      .layout-tree .lt-fold { color: #d5a910; }
      .dark .layout-tree .lt-fold { color: #ffd452; }
      .layout-tree .lt-fold-closed { display: none; }
      .layout-tree details:not([open]) summary .lt-fold-open { display: none; }
      .layout-tree details:not([open]) summary .lt-fold-closed { display: flex; }
      .layout-tree .lt-hl { color: var(--primary); background-color: color-mix(in srgb, var(--primary) 8%, transparent); }
      .dark .layout-tree .lt-hl { color: var(--primary-light); }
      .layout-tree .lt-ic-gray { color: #84848a; }
      .dark .layout-tree .lt-ic-gray { color: #adadb1; }
      .layout-tree .lt-ic-blue { color: #1a85d4; }
      .dark .layout-tree .lt-ic-blue { color: #69b1ff; }
      .layout-tree .lt-ic-cyan { color: #1ca1c7; }
      .dark .layout-tree .lt-ic-cyan { color: #68cdf2; }
      .layout-tree .lt-ic-green { color: #199f43; }
      .dark .layout-tree .lt-ic-green { color: #5ecc71; }
      .layout-tree .lt-ic-indigo { color: #693acf; }
      .dark .layout-tree .lt-ic-indigo { color: #9d6afb; }
      .layout-tree .lt-ic-orange { color: #d47628; }
      .dark .layout-tree .lt-ic-orange { color: #ffa359; }
      .layout-tree .lt-ic-pink { color: #d32a61; }
      .dark .layout-tree .lt-ic-pink { color: #ff678d; }
      .layout-tree .lt-ic-purple { color: #a631be; }
      .dark .layout-tree .lt-ic-purple { color: #d568ea; }
      .layout-tree .lt-ic-red { color: #d52c36; }
      .dark .layout-tree .lt-ic-red { color: #ff6762; }
      .layout-tree .lt-ic-yellow { color: #d5a910; }
      .dark .layout-tree .lt-ic-yellow { color: #ffd452; }
    `}</style>
    {children}
  </div>
)

export const LayoutRow = ({ icon, name, note, highlight = false, button = false }) => (
  <div className={highlight ? "lt-row lt-hl" : "lt-row"} style={{ display: "flex", alignItems: "flex-start", gap: "6px", minHeight: "30px", padding: "5px 8px", margin: "0 2px", borderRadius: "6px", lineHeight: "20px", cursor: button ? "pointer" : "default" }}>
    <span style={{ width: "16px", height: "20px", flexShrink: 0, display: "flex", alignItems: "center", justifyContent: "center", color: highlight ? "inherit" : "var(--text-muted)" }}>{icon}</span>
    <span style={{ whiteSpace: "nowrap", fontFamily: "var(--font-mono, ui-monospace, SFMono-Regular, Menlo, monospace)", fontSize: "13px", fontWeight: 600, color: highlight ? "inherit" : "var(--text-primary)" }}>{name}</span>
    {note ? <span style={{ color: "var(--text-muted)", fontSize: "13px", fontStyle: "italic", marginLeft: "10px", opacity: 0.9 }}>{note}</span> : null}
  </div>
)

export const LayoutFolderIcon = ({ kind }) => (
  <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
    {kind === "empty" ? (
      <path fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinejoin="round" d="M2 4.2A1.2 1.2 0 0 1 3.2 3h3.1c.32 0 .62.13.85.35L8.4 4.6h4.4A1.2 1.2 0 0 1 14 5.8v6a1.2 1.2 0 0 1-1.2 1.2H3.2A1.2 1.2 0 0 1 2 11.8z" />
    ) : kind === "open" ? (
      <g fill="currentColor">
        <path opacity=".5" d="M1.5 4A1.5 1.5 0 0 1 3 2.5h3.38a1.5 1.5 0 0 1 1.06.44L8.5 4H13a1.5 1.5 0 0 1 1.5 1.5V7H4.2a1.5 1.5 0 0 0-1.4.95L1.5 11.3z" />
        <path d="M4.2 8h10.6a.7.7 0 0 1 .65.95l-1.6 4.1a1.5 1.5 0 0 1-1.4.95H2.2a.7.7 0 0 1-.65-.95l1.25-3.2A1.5 1.5 0 0 1 4.2 8z" />
      </g>
    ) : (
      <g fill="currentColor">
        <path d="M1.5 4A1.5 1.5 0 0 1 3 2.5h3.38a1.5 1.5 0 0 1 1.06.44L8.5 4H1.5z" />
        <path opacity=".6" d="M1.5 4.5V12A1.5 1.5 0 0 0 3 13.5h10a1.5 1.5 0 0 0 1.5-1.5V6A1.5 1.5 0 0 0 13 4.5z" />
      </g>
    )}
  </svg>
)

export const LayoutFileIcon = () => (
  <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
    <path fill="currentColor" opacity=".5" d="M8 1v3a3 3 0 0 0 3 3h3v5.5a2.5 2.5 0 0 1-2.5 2.5h-7A2.5 2.5 0 0 1 2 12.5v-9A2.5 2.5 0 0 1 4.5 1z" />
    <path fill="currentColor" d="M9.5 1a.5.5 0 0 1 .354.146l4 4A.5.5 0 0 1 14 5.5V6h-3a2 2 0 0 1-2-2V1z" />
  </svg>
)

export const LayoutDir = ({ name, note, open = false, highlight = false, children }) =>
  children ? (
    <details open={open}>
      <summary>
        <LayoutRow icon={<span className={highlight ? "" : "lt-fold"} style={{ display: "flex" }}><span className="lt-fold-open" style={{ display: "flex" }}><LayoutFolderIcon kind="open" /></span><span className="lt-fold-closed"><LayoutFolderIcon kind="closed" /></span></span>} name={name} note={note} highlight={highlight} button />
      </summary>
      <div style={{ marginLeft: "17.5px", borderLeft: "1px solid var(--border-subtle)", paddingLeft: "6px" }}>
        {children}
      </div>
    </details>
  ) : (
    <LayoutRow icon={<span className={highlight ? "" : "lt-fold"} style={{ display: "flex" }}><LayoutFolderIcon kind="empty" /></span>} name={name} note={note} highlight={highlight} />
  )

export const layoutIconColor = (name) => {
  const n = String(name).toLowerCase()
  const ext = n.includes(".") ? n.split(".").pop() : ""
  if (n === "dockerfile" || n.startsWith("dockerfile.") || n.startsWith("docker-compose")) return "blue"
  if (["rs", "json", "jsonc", "html", "htm", "zip", "gz", "tgz", "crx", "xpi", "swift", "svg"].includes(ext)) return "orange"
  if (["ts", "tsx", "mts", "cts", "py"].includes(ext)) return "blue"
  if (["js", "mjs", "cjs", "jsx"].includes(ext)) return "yellow"
  if (["md", "mdx", "sh", "bash", "zsh", "fish", "ps1"].includes(ext)) return "green"
  if (["css", "scss"].includes(ext)) return "indigo"
  if (["png", "jpg", "jpeg", "gif", "webp", "ico"].includes(ext)) return "pink"
  if (ext === "go") return "cyan"
  return "gray"
}

export const LayoutFile = ({ name, note, highlight = false }) => (
  <LayoutRow icon={<span className={highlight ? "" : "lt-ic-" + layoutIconColor(name)} style={{ display: "flex" }}><LayoutFileIcon /></span>} name={name} note={note} highlight={highlight} />
)

This guide covers additional `BaseCase` helpers that close the parity gap with
Python SeleniumBase. These helpers handle page artifacts, window introspection,
on-page messaging, console logs, traffic generation, charts, and deferred
assertions.

<Info>

**What you will learn**

- How to save screenshots and page source.
- How to inspect and manipulate the browser window.
- How to post messages and read console logs.
- How to use deferred assertions for soft failures.

</Info>

## Page artifacts

```rust
let screenshot = sb.save_screenshot("home.png").await?;
let source = sb.save_page_source("home.html").await?;
```

Both methods return a `PathBuf` pointing to the saved file in the logs directory
(`latest_logs/`).

<Warning>

The name must be a plain file name: `"../home.png"` or `"/tmp/home.png"` is
refused with an `InvalidConfig` error instead of writing outside the logs
directory. A file of the same name is replaced.

</Warning>

For an exact
path of your own, use `save_screenshot_to_path`. Files the framework names
itself (`save_screenshot_to_logs`) never replace an earlier one.

## Reading text from the screen (OCR)

<Badge color="blue">Feature: ocr</Badge>

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

The text comes back one line per line of text, top to bottom.

<Info>

The first
`Ocr::load` downloads the two models (about 12 MB) into the cache folder (below)
and checks them against pinned SHA-256 hashes; later runs use the saved copies.
On a machine without network access, set `SB_OCR_MODELS` to a folder that holds
`text-detection.rten` and `text-recognition.rten`. Build with
`--features ocr`; `tests/ocr_chrome.rs` reads a heading from a real Chrome.

</Info>

The cache folder, by system:

<CodeGroup>

```text macOS
~/Library/Caches/seleniumbase-rs/ocr
```

```text Linux
~/.cache/seleniumbase-rs/ocr
```

```text Windows
%LOCALAPPDATA%\seleniumbase-rs\ocr
```

</CodeGroup>

## Visual regression testing (pixels)

<Badge color="blue">Feature: visual</Badge>

`check_window` is the port of Python's: it compares the page's *layout*, the
tags of the HTML and their attributes, and keeps the screenshot only for a
person to look at. A new colour, font, image or position that leaves the HTML
alone passes it. With the `visual` feature, `assert_visual_match` compares the
*pixels* instead:

```rust
sb.open("https://example.com").await?;
sb.check_window("home", 3).await?;     // the layout, as in Python
sb.assert_visual_match("home").await?;   // the screenshot, by structural similarity
```

The first run saves the screenshot as `visual_baseline/<test>/home.png`, where
`<test>` is the test's whole path: the test binary (the crate, or the file in
`tests/`), the module path and the function, joined with `.`, such as
`cart.checkout.renders`, so tests of the same name in other modules or files
keep separate baselines. (`check_window` names its folder after the function
alone, as Python does.) Later runs compare with it and fail with `AssertionFailed` when the
similarity is below 0.999; the failure leaves two more files next to the baseline:

<LayoutTree>
  <LayoutDir name="visual_baseline" open>
    <LayoutDir name="<test>" open note="the test's whole path, such as cart.checkout.renders">
      <LayoutFile name="home.png" highlight note="the baseline, saved by the first run" />
      <LayoutFile name="home.actual.png" note="the new screenshot, left by a failing run" />
      <LayoutFile name="home.diff.png" note="the new screenshot in grey with what changed painted red" />
    </LayoutDir>
  </LayoutDir>
</LayoutTree>

A screenshot of another size fails without
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
sb.post_message_for("Transient message", Duration::from_secs(3)).await?;
```

These helpers inject a visible banner into the page, useful for demos or
MasterQA reports. The banner replaces the one already showing and goes away
after the run's message duration (`post_message_for` takes its own); in a headless
browser that is at most 0.75 s, as in Python. Each call returns 0.15 s after the
banner has gone, like Python's `pause=True`.

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

<Note>

Earlier versions of this crate returned the URL with a `?ref=` parameter added
instead, which sends no `Referer` at all.

</Note>

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
sb.deferred_assert_exact_text("Done", "#result").await?;
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
