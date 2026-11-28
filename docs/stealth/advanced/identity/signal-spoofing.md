---
title: "Browser & Graphics Spoofing"
sidebarTitle: "Browser & graphics spoofing"
description: "Coherent browser API and graphics spoofing across WebGL 1/2, WebGPU, Canvas/OffscreenCanvas, DOMRect geometry, Web Audio & AnalyserNode, font metrics, Client Hints, CDP stack sanitization, and Mimic specimens."
---

<Badge color="blue">Identity & Client</Badge> <Badge color="purple">Crate: seleniumbase-rs-stealth</Badge>

Modern anti-bot systems cross-check **WebGL parameter matrices, WebGPU adapter metadata, audio buffer hashes, canvas text/pixel rendering, subpixel `DOMRect` geometry, Client Hints, display gamut, and CDP evaluation stacks** for contradictions.

Rather than patching one getter at a time, you configure a single `Fingerprint` (or `StealthLayer`) and attach it to `LaunchOptions`, `BrowserConfig`, or a live `Page`.

## 1. Quick start: apply strict spoofing to a browser

Use `StealthLayer::strict` when a target inspects canvas, WebGL, audio, fonts, and `DOMRect` jitter together:

```rust
use seleniumbase_rs_stealth::client::{
    LaunchOptionsFingerprintExt, PageFingerprintExt, StealthLayer,
};
use seleniumbase_rs_stealth::identity::OsType;
use seleniumbase_rs_stealth::sb_cdp::{Browser, LaunchOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Enables all 42 JS spoofing surfaces + coherent seeded noise + CDP overrides
    let layer = StealthLayer::strict(OsType::Windows, 42);
    let options = LaunchOptions::builder()
        .headless(true)
        .build()?
        .with_stealth(&layer)?;

    let browser = Browser::launch(options).await?;
    let page = browser.default_page().await?;
    page.goto("https://example.com").await?;

    // Audit the live tab against expected claims
    let fp = layer.fingerprint().expect("strict layer builds a fingerprint");
    let diff = page.compare_fingerprint(fp).await?;
    assert!(diff.contradictions().is_empty(), "{diff:?}");

    browser.close().await?;
    Ok(())
}
```

## 2. Recorded specimens (`Spoof::Mimic`)

Instead of hand-authoring hardware parameters, `Spoof::Mimic(Specimen)` pins settings directly to recorded real browsers, GPU hardware profiles, and geo places:

```rust
use seleniumbase_rs_stealth::identity::fingerprint::mimic::{Build, Machine, Place, Specimen};
use seleniumbase_rs_stealth::identity::fingerprint::spoof::Spoof;
use seleniumbase_rs_stealth::identity::Fingerprint;

fn build_from_specimens() -> Result<Fingerprint, Box<dyn std::error::Error>> {
    let build = Build::named("chrome-156-windows")?;
    let machine = Machine::named("windows-uhd-620")?;
    let place = Place::named("de-de-berlin")?;

    let fp = Fingerprint::builder()
        .user_agent(Spoof::Mimic(Specimen::Build(build)))
        .webgl(Spoof::Mimic(Specimen::Machine(machine)))
        .screen(Spoof::Mimic(Specimen::Machine(machine)))
        .timezone(Spoof::Mimic(Specimen::Place(place)))
        .locale(Spoof::Mimic(Specimen::Place(place)))
        .build()?;

    assert!(fp.validate().is_coherent());
    Ok(fp)
}
```

| Specimen type | Lookup helper | What it supplies |
|---|---|---|
| `Specimen::Build(Build)` | `Build::named(...)` | Real User-Agent, `core_version`, brand matrix, Client Hints metadata |
| `Specimen::Machine(Machine)` | `Machine::named(...)` | Coherent CPU cores, RAM, screen geometry, DPR, color gamut, WebGL/WebGPU tables |
| `Specimen::Place(Place)` | `Place::named(...)` | IANA timezone, locale, `Accept-Language`, keyboard layout, coordinates |
| `Specimen::Rendering(Rendering)` | `Rendering::named(...)` | Captured reference outputs for rendering probes |

## 3. Fine-tuning WebGL, Canvas, Audio, Display & Client Hints

Every spoofing dimension is controlled cleanly on `FingerprintBuilder` and validated by `Fingerprint::validate()`:

```rust
use seleniumbase_rs_stealth::identity::fingerprint::{
    AudioNoise, CanvasNoise, Fingerprint, OsType, WebRtcPolicy,
};
use seleniumbase_rs_stealth::identity::fingerprint::settings::{
    ColorGamut, DynamicRange, MediaFeatures, WebGl,
};

fn custom_desktop_spoofing() -> Result<Fingerprint, Box<dyn std::error::Error>> {
    let fp = Fingerprint::builder()
        .os_type(OsType::Windows)
        .core_version(157)
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/157.0.0.0 Safari/537.36")
        .languages_list(&["en-US", "en"])
        .hardware_concurrency(8)
        .device_memory(8.0)
        .webgl(
            WebGl::new(
                "Google Inc. (NVIDIA)",
                "ANGLE (NVIDIA, NVIDIA GeForce RTX 4070 Direct3D11 vs_5_0 ps_5_0, D3D11)",
            )
            .max_texture_size(16384),
        )
        .canvas_noise(CanvasNoise::Persistent)
        .audio_noise(AudioNoise::Persistent)
        .audio_sample_rate(48_000)
        .webrtc_policy(Some(WebRtcPolicy::DisableNonProxiedUdp))
        .media_features(MediaFeatures {
            color_gamut: ColorGamut::Srgb,
            dynamic_range: DynamicRange::Standard,
            ..MediaFeatures::default()
        })
        .native_spoofing(true)
        .build()?;

    assert!(fp.validate().is_coherent());
    Ok(fp)
}
```

### Applying selective JS bundles (`StealthScriptBundle`)

If you want only specific script surfaces (for example hiding `webdriver`, scrubbing CDP stack markers, and keeping `canvas`/`audio` untouched for screenshot visual regression):

```rust
use seleniumbase_rs_stealth::client::script::{StealthScriptBundle, StealthScriptOptions};
use seleniumbase_rs_stealth::client::PageFingerprintExt;
use seleniumbase_rs_stealth::identity::OsType;
use seleniumbase_rs_stealth::sb_cdp::Page;

async fn inject_selective_bundle(page: &Page) -> Result<(), seleniumbase_rs_stealth::SeleniumBaseError> {
    let mut opts = StealthScriptOptions::balanced();
    opts.canvas_noise = false;
    opts.audio_noise = false;

    let bundle = StealthScriptBundle::build(OsType::Macos, 2026, opts);
    page.apply_stealth(&bundle).await
}
```

## 4. What each spoofing surface handles

- **Display, color gamut, and HDR (`MediaFeatures`)**: aligns CSS `@media (color-gamut / dynamic-range)` and `Emulation.setEmulatedMedia` with `screen.colorDepth`, `isExtended`, and `devicePixelRatio`.
- **WebGL 1/2 & WebGPU (`WebglProvider`, `WebGpuProvider`)**: emulates full parameter matrices (`MAX_TEXTURE_SIZE`, `MAX_DRAW_BUFFERS`, shader precision, extensions list) and aligns `navigator.gpu.requestAdapter()` / `GPUAdapterInfo`.
- **Canvas, `OffscreenCanvas`, & `DOMRect` (`CanvasNoiseProvider`, `DomRectProvider`)**: injects deterministic per-seed noise into `toDataURL`, `toBlob`, `getImageData`, `OffscreenCanvas`, and ~`1e-4` micro-jitter into `getBoundingClientRect` / `getClientRects`.
- **Web Audio (`AudioNoiseProvider`)**: sets `sampleRate` (`44100` / `48000`), hardware-plausible latency getters on the prototype, and deterministic perturbation on `getChannelData` / `AnalyserNode`.
- **Fonts & Client Hints (`FontsProvider`, `ClientHintsProvider`)**: filters `document.fonts.check()` against OS-appropriate fonts and aligns `navigator.userAgentData` with CDP `Network.setUserAgentOverride` (`native_spoofing(true)`).
- **Runtime Hygiene (`NativeToStringProvider`, `CdpMarkerProvider`)**: preserves `[native code]` in `Function.prototype.toString`, removes `$cdc_` / `__webdriver*` globals, and strips evaluation stack frames from `Error.prototype.stack`.

## 5. Multi-profile generation (`FingerprintGenerator`)

```rust
use seleniumbase_rs_stealth::client::PageFingerprintExt;
use seleniumbase_rs_stealth::identity::{FingerprintGenerator, OsType};
use seleniumbase_rs_stealth::sb_cdp::Page;

async fn apply_generated_to_tab(page: &Page) -> Result<(), Box<dyn std::error::Error>> {
    let generated = FingerprintGenerator::new(2026)
        .os(OsType::Macos)
        .audio_sample_rate(48_000)
        .hdr(true)
        .generate();

    assert!(generated.fingerprint.validate().is_coherent());
    page.apply_generated(&generated).await?;
    Ok(())
}
```

<Columns cols={2}>

<Card title="Stealth API: identity" icon="id-card" color="#199f43" href="/stealth/api/identity/overview">
  `Fingerprint`, `Spoof`, `StealthPolicy`, `Specimen`, `BayesianSampler`.
</Card>

<Card title="Stealth API: client" icon="mask" color="#1a85d4" href="/stealth/api/client/overview">
  `StealthLayer` levels, `StealthScriptBundle`, and 32 JS providers.
</Card>

<Card title="Binary patching" icon="wrench" color="#d47628" href="/stealth/advanced/core/binary-patching">
  Inspect and scrub `chromedriver` and Chromium binaries before launch.
</Card>

<Card title="Evasion providers" icon="sliders" color="#a631be" href="/stealth/advanced/client/providers">
  Register custom priority-ordered JS providers.
</Card>

</Columns>
