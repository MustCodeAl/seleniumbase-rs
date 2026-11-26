---
title: "BiDi Event Streaming & Streaming Network Dumper"
sidebarTitle: "BiDi & Streaming Capture"
description: "Real-time W3C WebDriver BiDi log/network event streaming, zero-copy streaming HAR/JSONL network dumper, and per-lease context fingerprinting."
---

Modern web automation requires high-fidelity observability without performance penalties. Traditional automation approaches either poll DevTools logs synchronously or buffer massive HTTP traffic in memory until the test run terminates, risking out-of-memory crashes on large capture sessions.

`seleniumbase-rs` introduces asynchronous event streaming and zero-copy disk streaming architectures:

- **WebDriver BiDi Event Streaming (`bidi_listen_logs`, `bidi_listen_network`)**: Real-time event streaming over W3C WebDriver BiDi directly into Tokio `mpsc` channels.
- **Zero-Copy Streaming Network Dumper (`HarStreamWriter`)**: Flushes CDP network events directly to disk using buffered I/O (`BufWriter`) in streaming HAR or JSONL format.
- **Per-Lease Browser Context Fingerprinting**: Injects unique fingerprints per `BrowserPool` lease without browser relaunches.
- **Humanized Behavioral Input (`human_click`, `human_type`)**: Simulates realistic Bézier mouse trajectories and natural keystroke timing jitter.

<Info>

**What you will learn**

- How to stream browser console logs and network events asynchronously via WebDriver BiDi.
- How to capture multi-gigabyte browser network sessions to disk without memory bloat.
- How to assign distinct fingerprint identities to individual contexts in a `BrowserPool`.
- How to dispatch human-paced clicks and keystrokes that defeat behavioral bot detectors.

</Info>

<Visibility for="agents">

Summary for AI exports: For BiDi streaming, enable `config.session.bidi = true` and call `sb.bidi_listen_logs().await?` or `sb.bidi_listen_network().await?` to obtain a `tokio::sync::mpsc::Receiver`. For large network capture, use `HarStreamWriter` / `page.stream_to_jsonl(path).await?` to write CDP network events directly to disk with `tokio::io::BufWriter` rather than buffering entries in RAM. For behavioral stealth, `ContextOptionsBuilder::fingerprint(fp)` provides per-lease fingerprint isolation in `BrowserPool::acquire_with(&options)`, while `sb.human_click(sel).await?` and `sb.human_type(sel, text).await?` emulate Bézier mouse curves and Gaussian keystroke pacing (50-120ms intervals).

</Visibility>

---

## WebDriver BiDi Event Subscriptions

W3C WebDriver BiDi enables bidirectional event streaming over a WebSocket connection. Unlike legacy WebDriver log commands that require periodic polling, BiDi pushes events immediately as they occur in the browser runtime.

### Streaming Console Logs (`bidi_listen_logs`, `bidi_forward_logs`)

```rust
use seleniumbase_rs::testing::{rstest, sb, Sb};
use seleniumbase_rs::{tokio, Result};
use futures::StreamExt;

#[rstest]
#[tokio::test]
async fn monitor_console_logs_via_bidi(sb: Sb) -> Result<()> {
    sb.run(async |sb| {
        // Subscribe to live BiDi console log event stream
        let mut stream = sb.bidi_listen_logs().await?;

        sb.open("https://app.example.com").await?;

        // Asynchronously await incoming log events pushed over the BiDi connection
        if let Ok(Some(Ok(entry))) = tokio::time::timeout(tokio::time::Duration::from_secs(2), stream.next()).await {
            println!("Captured BiDi console log: {:?}", entry);
        }

        sb.assert_element("#app-container").await?;
        Ok(())
    })
    .await
}
```

Or forward logs directly into a Tokio channel with `sb.bidi_forward_logs(sender)`:

```rust
use seleniumbase_rs::testing::{rstest, sb, Sb};
use seleniumbase_rs::{tokio, Result};

#[rstest]
#[tokio::test]
async fn forward_bidi_logs_to_channel(sb: Sb) -> Result<()> {
    sb.run(async |sb| {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let _forwarder = sb.bidi_forward_logs(tx).await?;

        sb.open("https://app.example.com").await?;

        if let Ok(Some(log_entry)) = tokio::time::timeout(tokio::time::Duration::from_secs(2), rx.recv()).await {
            println!("Received forwarded log: {:?}", log_entry);
        }

        Ok(())
    })
    .await
}
```

### Streaming Network Events (`bidi_listen_network`)

```rust
use seleniumbase_rs::testing::{rstest, sb, Sb};
use seleniumbase_rs::{tokio, Result};
use futures::StreamExt;

#[rstest]
#[tokio::test]
async fn stream_network_requests_via_bidi(sb: Sb) -> Result<()> {
    sb.run(async |sb| {
        let mut net_stream = sb.bidi_listen_network().await?;

        sb.open("https://api.example.com/status").await?;

        while let Ok(Some(Ok(event))) = tokio::time::timeout(tokio::time::Duration::from_millis(500), net_stream.next()).await {
            println!("BiDi Network Event: {:?}", event);
        }

        Ok(())
    })
    .await
}
```

---

## Zero-Copy Streaming HAR & JSONL Network Dumper

Capturing HTTP traffic (HAR archives) during long-running crawler runs or heavy media downloads can consume several gigabytes of RAM if kept in memory.

The `HarStreamWriter` streams network events directly to disk through a buffered writer (`tokio::io::BufWriter`), keeping memory overhead constant regardless of capture size.

```rust
use std::path::Path;
use seleniumbase_rs::sb_cdp::har::HarStreamWriter;
use seleniumbase_rs::testing::{rstest, sb_cdp, SbCdp};
use seleniumbase_rs::{tokio, Result};

#[rstest]
#[tokio::test]
async fn stream_network_to_disk(sb_cdp: SbCdp) -> Result<()> {
    sb_cdp.run(async |page| {
        let dump_path = Path::new("target/network_capture.jsonl");

        // Stream incoming network events directly to disk without memory bloat
        let capture = page.stream_to_jsonl(dump_path).await?;

        page.goto("https://media.example.com/gallery").await?;
        page.locator(".gallery-item").first().expect().to_be_visible().await?;

        // Finish capture and flush remaining entries
        let stats = capture.finish().await?;
        println!("Streamed {} network entries to disk", stats.entries_count);
        assert!(dump_path.exists());
        Ok(())
    })
    .await
}
```

---

## Per-Lease `BrowserPool` Fingerprint Isolation

When scraping at scale with a [`BrowserPool`](/tutorials/pure_cdp), recycling full browser processes to change identity is expensive.

With per-lease fingerprinting (via `seleniumbase-rs-stealth` / `seleniumbase-rs-evasion`), each leased browser context (tab) can be configured with an independent `Fingerprint`:

```rust
use seleniumbase_rs_stealth::evasion::fingerprint::{Fingerprint, OsType};
use seleniumbase_rs_stealth::evasion::identity::PageFingerprintExt;
use seleniumbase_rs_stealth::sb_cdp::{BrowserPool, LaunchOptions, PoolOptions};
use seleniumbase_rs_stealth::{tokio, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let pool = BrowserPool::new(
        LaunchOptions::builder().headless(true).build()?,
        PoolOptions::builder().max_browsers(2).build()?,
    );

    // Lease 1: Configured with a randomized fingerprint
    let fp1 = Fingerprint::randomized(OsType::Windows, 1);
    let lease1 = pool.acquire().await?;
    lease1.page().apply_fingerprint(&fp1).await?;

    lease1.page().goto("https://httpbin.org/user-agent").await?;
    lease1.release().await;

    // Lease 2: Configured with a distinct specific spoof identity
    let fp2 = Fingerprint::randomized(OsType::Macos, 2);
    let lease2 = pool.acquire().await?;
    lease2.page().apply_fingerprint(&fp2).await?;

    lease2.page().goto("https://httpbin.org/user-agent").await?;
    lease2.release().await;

    pool.close().await?;
    Ok(())
}
```

---

## Behavioral Humanized Input (`human_click`, `human_type`)

Anti-bot systems track pointer acceleration, angular deviations, and keystroke intervals. `BaseCase::human_click` and `BaseCase::human_type` replace instantaneous automated inputs with realistic human dynamics:

```rust
use seleniumbase_rs::testing::{rstest, sb, Sb};
use seleniumbase_rs::{tokio, Result};

#[rstest]
#[tokio::test]
async fn human_login_interaction(sb: Sb) -> Result<()> {
    sb.run(async |mut sb| {
        sb.open("https://auth.example.com/login").await?;
        sb.assert_element("#login-form").await?;

        // Human-paced typing: adds 50-120ms Gaussian intervals with natural jitter
        sb.human_type("#username", "audited_user").await?;
        sb.human_type("#password", "super_secret_passphrase").await?;

        // Human-like Bézier curve mouse trajectory with natural acceleration
        sb.human_click("#sign-in-button").await?;

        sb.assert_text("Dashboard", "#nav-header").await?;
        Ok(())
    })
    .await
}
```
