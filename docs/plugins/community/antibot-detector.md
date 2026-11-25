---
title: "antibot-detector Plugin"
sidebarTitle: "antibot-detector"
description: "Multi-signal detection plugin identifying Cloudflare, DataDome, Akamai, Kasada, PerimeterX, Imperva, AWS WAF, FingerprintJS, reCAPTCHA, and hCaptcha."
---

<Badge color="purple">Community plugin</Badge> <Badge color="green">no unsafe</Badge>

`antibot-detector` (`plugins/antibot-detector` in [seleniumbase-rs-plugins](https://github.com/MustCodeAl/seleniumbase-rs-plugins)) performs multi-signal static and runtime analysis over page HTML, scripts, iframes, cookies, and response headers to classify anti-bot vendors, WAFs, CAPTCHAs, and fingerprinting scripts.

## Supported providers

| Vendor (`BotProtectionKind`) | Category (`ProtectionCategory`) | Example signals inspected (`SignalKind`) |
|---|---|---|
| `Cloudflare` | Bot Management / CAPTCHA / Waiting Room | Turnstile iframes (`cf-turnstile`), `cf-ray`, `cf-challenge-running`, `cf_clearance`, `__cf_bm`, `__cfwaitingroom` |
| `DataDome` | Bot Management / CAPTCHA | `datadome` cookie, `cid`, `geo.captcha-delivery.com` iframes, `x-datadome` header |
| `Akamai` | Bot Management | `_abck`, `bm_sz`, `bm_sv` cookies, `/akam/13/`, sensor scripts |
| `Kasada` | Bot Management | `ips.js`, KPSDK tokens (`kpsdkct`, `kpsdkcd`, `window.KPSDK`), signature path hashes |
| `PerimeterX` | Bot Management / CAPTCHA | `_px` / `_px2` / `_px3` cookies, `client.js`, `px-captcha` containers |
| `Imperva` | WAF / Bot Management | `visid_incap_*`, `incap_ses_*`, `/_Incapsula_Resource`, incident blocks |
| `AwsWaf` | WAF / CAPTCHA | `aws-waf-token` cookie, `token.awswaf.com` scripts, CAPTCHA containers |
| `FingerprintJs` | Fingerprinting | `@fingerprintjs/fingerprintjs`, `openfpcdn.io`, `api.fpjs.io`, `visitorId` |
| `ReCaptcha` / `HCaptcha` | CAPTCHA | Google reCAPTCHA v2/v3/Enterprise and hCaptcha scripts & widgets |

## Install

```sh Terminal
cargo add antibot-detector --git https://github.com/MustCodeAl/seleniumbase-rs-plugins
```

The default `plugin` feature enables `seleniumbase-rs-plugin` (`Plugin` and `Task` integration). Disable default features if you only want pure HTML/cookie/header analysis (`default-features = false`).

## Usage

### Standalone inspection

```rust
use antibot_detector::{AntibotDetector, BotProtectionKind, Cookie, Page};

let detector = AntibotDetector::new();
let html = r#"<html><body><div class="cf-turnstile" data-sitekey="0x4AAAAAA"></div></body></html>"#;
let page = Page::new(html).with_cookies(vec![Cookie::new("cf_clearance", "token123")]);

let detections = detector.detect(&page)?;
assert_eq!(detections[0].provider, BotProtectionKind::Cloudflare);
Ok::<(), antibot_detector::DetectionError>(())
```

### As a `seleniumbase-rs` plugin

Attach `AntibotDetector` to a `PluginSet` and run its `antibot-detector` task, which evaluates the active tab (`Page::from_handle`), records each `BotProtectionResult` to `TaskContext::sink`, and returns `{"detections": [...], "blocked": bool}`.

- **Plugin name**: `"antibot-detector"` (`antibot_detector::NAME`)
- **Declared capabilities**: `Capability::PageEvaluate`, `Capability::Task`
