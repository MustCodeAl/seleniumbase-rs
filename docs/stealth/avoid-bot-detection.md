---
title: "Avoiding Bot Detection"
sidebarTitle: "Avoiding bot detection"
description: "Practical guide to preventing automated browser detection with seleniumbase-rs and seleniumbase-rs-stealth across Core Layer 0, identity, client masking, human input, proxy rotation, session persistence, and wire alignment."
---

<Badge color="blue">Guide</Badge> <Badge color="green">v0.10.0</Badge>

Modern anti-bot systems evaluate browsers across multiple layers simultaneously: launch flags, `chromedriver` binary signatures, headless anomalies, JavaScript prototype integrity, WebGL/canvas/audio consistency, input cadence, IP reputation, session state, and TLS/HTTP2 wire fingerprints. Patching a single flag such as `navigator.webdriver` is no longer sufficient.

This guide walks through each detection vector in order, showing how to address it cleanly with `seleniumbase-rs` (Core Layer 0) and `seleniumbase-rs-stealth` (Layers 1 through 4) without brittle workarounds or `sleep()` loops.

<Visibility for="agents">

Summary for AI exports: Practical guide to avoiding bot detection using `seleniumbase-rs` (Core Layer 0: `sb_cdp`, `DriverMode::Uc`, `undetected::patcher`, `sb_cdp::human`) and `seleniumbase-rs-stealth` (Layer 1 `identity`, Layer 2 `client` `StealthLayer` levels, Layer 3 `network` `ProxyPool` / `WireFormat` / `MitmProxy` / `http3`, Layer 4 `session` `CdpSession` / `HttpSession` / `HybridSession`). Covers disabling `navigator.webdriver` and automation markers, eliminating headless tells, generating coherent fingerprints and Client Hints, applying `StealthLayer` levels and native CDP overrides, rotating geo-aligned proxies, human-paced interaction via `Humanizer`, persisting cookies/identities, and wire-level TLS/HTTP2/HTTP3 alignment.

</Visibility>

## How anti-bot systems detect browser automation

Detectors correlate signals across four boundaries:

1. **Process and protocol layer**: `navigator.webdriver = true`, `cdc_` variables injected by `chromedriver`, `--enable-automation` switches, and `Runtime.enable` / CDP stack traces.
2. **Browser and fingerprint layer**: headless User-Agent strings, `800x600` viewports, missing `window.chrome` stubs, OS/GPU contradictions (for example a macOS User-Agent paired with a Direct3D WebGL renderer), and non-native `Function.prototype.toString` wrappers.
3. **Behavioral and session layer**: instant teleporting clicks, uniform keystroke intervals, high request volume from a single IP, cold cookie jars hitting deep endpoints directly, or mismatched timezone/locale vs exit IP country.
4. **Network wire layer**: TLS `ClientHello` (JA3/JA4/peetprint), HTTP/2 `SETTINGS` and pseudo-header ordering (Akamai string), and HTTP/3 QUIC transport parameters differing from the browser claimed in `User-Agent`.

Address them progressively. Most targets are satisfied by Steps 1 through 6 with zero BoringSSL compilation; add Step 8 only when a target enforces wire-level fingerprinting.

---

## 1. Replace standard WebDriver with Pure CDP or UC mode

Plain WebDriver launches Chrome through `chromedriver`, setting `navigator.webdriver = true`, adding `--enable-automation`, and injecting `$cdc_` markers into the document.

Prefer either the **Pure CDP engine** (`sb_cdp`, which talks directly to Chrome over a single WebSocket with no `chromedriver` binary and never enables noisy `Runtime`/`DOM`/`Debugger` domains) or **UC mode** (`DriverMode::Uc`, which strips automation switches and detaches cleanly on reconnect):

<CodeGroup>

```rust Pure CDP (recommended)
use seleniumbase_rs_stealth::sb_cdp::{Browser, LaunchOptions};
use seleniumbase_rs_stealth::{LaunchOptionsFingerprintExt, StealthLayer};

async fn open_pure_cdp() -> Result<(), Box<dyn std::error::Error>> {
    let layer = StealthLayer::default();
    let options = LaunchOptions::builder()
        .headless(true)
        .build()?
        .with_stealth(&layer)?;

    let browser = Browser::launch(options).await?;
    let page = browser.default_page().await?;
    page.goto("https://example.com").await?;
    browser.close().await?;
    Ok(())
}
```

```rust WebDriver UC mode
use seleniumbase_rs_stealth::{
    BaseCase, BrowserConfig, BrowserConfigFingerprintExt, DriverMode, OsType, StealthLayer,
};

async fn open_uc_mode() -> Result<(), Box<dyn std::error::Error>> {
    let layer = StealthLayer::balanced(OsType::Macos, 42);
    let config = BrowserConfig::builder()
        .mode(DriverMode::Uc)
        .build()?
        .with_stealth(&layer)?;

    let mut sb = BaseCase::new(config).await?;
    sb.open("https://example.com").await?;
    sb.quit().await?;
    Ok(())
}
```

</CodeGroup>

If you must keep a WebDriver workflow, scrub byte-level markers from `chromedriver` or Chrome binaries beforehand using Layer 0 binary patching (`seleniumbase_rs::undetected::patcher` or the CLI):

```sh Terminal
sbrs patch-chromedriver --inspect
sbrs patch-chromedriver --preset balanced
sbrs patch-chrome --preset chrome-binary --with-companions
```

See [Binary patching](/stealth/advanced/binary-patching) for preset details and backup restoration.

---

## 2. Eliminate headless browser tells

Classic headless Chrome leaks `HeadlessChrome` in the User-Agent, reports `0x0` outer window dimensions, sets `Notification.permission` inconsistently, and omits standard desktop media capabilities.

- `sb_cdp` uses Chrome's modern headless mode (`--headless=new`) rather than legacy headless.
- Every preset and `StealthLayer::balanced` / `StealthLayer::strict` applies headless fixes (`window_geometry`, `fullscreen_guard`, `visibility_guard`, `permissions`, `chrome_runtime`, and `Emulation.setFocusEmulationEnabled`).
- Avoid forcing `headless(false)` in committed tests; toggle headless outside code with `SB_HEADLESS=true` or `SB_HEADLESS=false` when diagnosing a tough target.

---

## 3. Generate coherent fingerprints, headers, and Client Hints

Rotating `User-Agent` strings alone often makes detection *worse*: detectors cross-check `User-Agent` against `navigator.platform`, `Sec-CH-UA` high-entropy Client Hints, WebGL vendor/renderer strings, `deviceMemory`, `hardwareConcurrency`, screen DPR, and timezone.

Use Layer 1 (`seleniumbase_rs_stealth::identity`) to synthesize or sample an entire coherent machine and place from one seed, then audit it before launching:

```rust
use seleniumbase_rs_stealth::identity::{
    audit_coherence, BayesianSampler, DeviceCategory, Fingerprint, OsType, RequestContext,
    SampleConstraints,
};

fn build_coherent_identity() -> Result<(), Box<dyn std::error::Error>> {
    // Deterministic seeded persona (passes coherence checks with zero warnings)
    let fp = Fingerprint::randomized(OsType::Windows, 42);
    let report = fp.validate();
    assert!(report.is_coherent() && report.warnings.is_empty());

    // Or sample from realistic joint hardware/OS distributions with matching HTTP headers
    let constraints = SampleConstraints::new()
        .device(DeviceCategory::Desktop)
        .seed(1234);
    let (generated, headers) =
        BayesianSampler::sample_with_headers(&constraints, RequestContext::Navigation)?;
    assert!(audit_coherence(&generated).is_clean());
    assert!(!headers.is_empty());
    Ok(())
}
```

For real hardware records, use `Spoof::mimic(...)` with cataloged `Machine`, `Build`, and `Place` specimens (`identity::mimic`). See [Fingerprints](/stealth/fingerprints) and [Stealth API: identity](/stealth/api/identity).

---

## 4. Pick the right stealth level and native CDP overrides

Layer 2 (`seleniumbase_rs_stealth::client`) lets you dial browser masking to match the target instead of injecting heavy noise everywhere:

- **`StealthLayer::minimal(os, seed)`**: 4 core surfaces (`webdriver`, `chrome_runtime`, `permissions`, `error_stack_scrub`).
- **`StealthLayer::balanced(os, seed)`** (and `StealthLayer::default()`): 35 surfaces covering workers, console debug canaries, mouse `screenX`/`screenY` leaks, WebRTC ICE filtering, `Intl`/timezone alignment, codecs, and geometry while keeping pixel/audio/SVG output lossless.
- **`StealthLayer::strict(os, seed)`**: all 42 surfaces, adding deterministic canvas, `OffscreenCanvas`, `toBlob`, Web Audio, `DOMRect`, SVG, and `performance.now()` perturbation.

When you want Chrome itself (rather than a JS getter) to report User-Agent, Client Hints, locale, timezone, geolocation, and screen dimensions, turn on `native_spoofing(true)` in Layer 1 `StealthPolicy` so JS providers step back where CDP overrides exist:

```rust
use seleniumbase_rs_stealth::identity::{Fingerprint, OsType, StealthPolicy};
use seleniumbase_rs_stealth::{LaunchOptionsFingerprintExt, PageFingerprintExt, StealthLayer};
use seleniumbase_rs_stealth::sb_cdp::{Browser, LaunchOptions};

async fn verify_self_consistency() -> Result<(), Box<dyn std::error::Error>> {
    let fp = Fingerprint::randomized(OsType::Macos, 99)
        .to_builder()
        .policy(StealthPolicy::balanced().native_spoofing(true))
        .build()?;

    let layer = StealthLayer::new(fp);
    let options = LaunchOptions::builder().headless(true).build()?.with_stealth(&layer)?;
    let browser = Browser::launch(options).await?;
    let page = browser.default_page().await?;
    page.goto("https://example.com").await?;

    // Audit the live tab for internal cross-surface contradictions
    let captured = page.capture_fingerprint().await?;
    for issue in captured.contradictions() {
        eprintln!("contradiction: {issue}");
    }

    browser.close().await?;
    Ok(())
}
```

See [Policies & levels](/stealth/policies) and [Stealth API: client](/stealth/api/client).

---

## 5. Rotate IPs with geo-aligned proxies and WebRTC protection

High request volume from one exit IP triggers rate limits and WAF blocks regardless of how clean the browser fingerprint is. Worse, using a German exit IP while presenting `en-US` and `America/New_York` (or leaking your real local IP over WebRTC) is an immediate red flag.

- Pair every proxy in `network::ProxyPool` with a `CountryCode` and pick a `RotationStrategy` (`RoundRobin`, `Random`, `LeastFailures`, `StickyPerSite`).
- Use `LocaleExt::localize` (or `CdpSessionBuilder::geo_proxies`) so `locale`, `Accept-Language`, `timezone`, and geolocation stay aligned with the proxy country automatically.
- Keep `WebRtcPolicy::DisableNonProxiedUdp` enabled (included in `StealthPolicy::balanced()`) so `RTCPeerConnection` cannot leak your host IP around the proxy.

```rust
use seleniumbase_rs_stealth::network::CountryCode;
use seleniumbase_rs_stealth::session::{CdpSession, SessionError};

async fn fetch_with_geo_rotation() -> Result<(), SessionError> {
    let mut session = CdpSession::builder()
        .geo_proxies([
            ("http://user:pass@de.proxy.example:8080", CountryCode::try_new("DE")?),
            ("http://user:pass@gb.proxy.example:8080", CountryCode::try_new("GB")?),
        ])?
        .max_challenge_attempts(4)
        .build()
        .await?;

    let result = session.fetch("https://example.com/").await?;
    println!("status={} bytes={}", result.status, result.body.len());
    session.close().await
}
```

Proxy credentials are stripped from `Debug`, logs, events, and error messages by construction. See [Proxies and network](/stealth/proxies).

---

## 6. Simulate human-paced interactions without raw sleeps

Anti-bot telemetry tracks cursor velocity curves, click coordinates inside bounding boxes, keystroke timing distributions, and scroll smoothness. Never add fixed `sleep()` calls to fake pacing or wait for page elements:

- Let `sb_cdp::Locator` and `BaseCase` assertions auto-wait on real DOM states (`wait_for_element_visible`, `locator.wait_for(State::Visible)`).
- Use `sb_cdp::human::Humanizer` (`page.human()`) for pointer movement (cubic Bezier paths with minimum-jerk speed profiles, Fitts's law timing, subtle tremor, and natural click offsets), Gaussian typing intervals with optional typo correction, and eased wheel scrolls:

```rust
use seleniumbase_rs_stealth::sb_cdp::human::Humanizer;
use seleniumbase_rs_stealth::sb_cdp::Page;

async fn human_checkout(page: &Page) -> Result<(), seleniumbase_rs_stealth::SeleniumBaseError> {
    let human = Humanizer::seeded(42);
    page.human(&human).click("#search-input").await?;
    page.human(&human).type_text("#search-input", "mechanical keyboard").await?;
    page.human(&human).press("Enter").await?;
    page.locator(".results-grid").wait_for_visible().await?;
    Ok(())
}
```

See [Human-like input](/stealth/human-input).

---

## 7. Manage cookies, isolated contexts, and persistent sessions

Repeatedly opening fresh, empty browsers to log in on every run looks automated and wastes time. Conversely, sharing one cookie jar across unrelated accounts contaminates sessions.

- **Isolate concurrent identities** with `BrowserPool::acquire()` or `browser.new_context()` so each flow gets its own browser context, cookies, storage, and per-lease fingerprint (`lease.page().apply_fingerprint(&fp).await?`).
- **Persist earned state** across runs with Layer 4 `CookieJar`, `Identity` passport files (written atomically with `0600` permissions), and `JsonLedger` cooldown/site tracking.
- **Let `MitigationPolicy` escalate cleanly** on challenges: solve in-browser, rotate egress after consecutive challenges, rotate fingerprint if needed, and honor `Retry-After` cooldowns instead of hammering a blocked endpoint.

See [Identity and cookies](/scraping/advanced-stealthy/identity) and [CdpSession](/scraping/stealthy/browser-scraper).

---

## 8. Align TLS, HTTP/2, and HTTP/3 wire fingerprints when required

When a target inspects the network handshake itself (JA3, JA4, peetprint, Akamai HTTP/2 string, or HTTP/3 QUIC settings) or ties clearance cookies to the exact TLS fingerprint that solved a browser challenge, enable Layer 3 (`features = ["network"]` or `features = ["http3"]`):

- **`session::HttpSession`**: replay earned browser cookies over plain HTTP/2 (and HTTP/3 via `Alt-Svc` + `sbrs-h3d`) presenting the measured Chrome wire handshake.
- **`network::MitmProxy`**: place a real Chrome behind an ephemeral-CA loopback proxy (`EphemeralCa` pinned with `--ignore-certificate-errors-spki-list`) so browser and HTTP requests share one identical wire signature.
- **`session::HybridSession`**: stay in fast HTTP mode until a challenge appears, automatically escalate to real Chrome behind `MitmProxy` to refresh clearance cookies, and drop back to HTTP mode.

<Warning>

Embedded wire measurements currently cover Chrome 155.0.8059.40 on macOS; unmeasured browser/OS pairs return typed `NotMeasured` or `VersionMismatch` errors rather than guessing. Follow each target site's terms of service and applicable laws, and review [Limits](/scraping/advanced-stealthy/limits) before relying on wire impersonation.

</Warning>

## Where to go next

<Columns cols={2}>

<Card title="Stealth overview" icon="user-secret" color="#199f43" href="/stealth/introduction">
  Core Layer 0 plus the four layers and stealth levels of seleniumbase-rs-stealth.
</Card>

<Card title="Policies & levels" icon="sliders" color="#1a85d4" href="/stealth/policies">
  Compare minimal, balanced, strict, native-only, and max presets.
</Card>

<Card title="Troubleshooting" icon="wrench" color="#d47628" href="/stealth/troubleshooting">
  Diagnose which surface a site is still flagging.
</Card>

<Card title="Stealthy scraping" icon="network-wired" color="#a631be" href="/scraping/stealthy/overview">
  CdpSession, HttpSession, MitmProxy, and HybridSession workflows.
</Card>

</Columns>
