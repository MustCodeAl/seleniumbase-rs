---
title: "Advanced fingerprinting"
sidebarTitle: "Advanced fingerprinting"
description: "Multi-layer browser identity synthesis and prototype-coherent spoofing across WebGL 1/2, WebGPU, Canvas/OffscreenCanvas, DOMRect geometry, Web Audio & AnalyserNode, font metrics, Client Hints, CDP stack sanitization, and Mimic specimens."
---

<Badge color="blue">Module: evasion</Badge> <Badge color="purple">Crate: seleniumbase-rs-stealth</Badge>

Modern anti-bot systems do not rely on a single flag like `navigator.webdriver`. They cross-check **graphics limits, shader precision, audio analyzer buffers, canvas font probe bounding boxes, subpixel `DOMRect` geometry, high-entropy Client Hints, display color gamut, and CDP evaluation stacks** for internal contradictions.

`seleniumbase-rs-stealth` (in `seleniumbase-rs-stealth`, re-exported as `seleniumbase_rs_stealth::identity`) combines deterministic, hardware-coherent persona modeling (`src/identity/fingerprint/`, `src/identity/ and src/client/mimic.rs`, `src/identity/ and src/client/randomize.rs`) with 32 prototype- and protocol-level `EvasionProvider` passes (`src/identity/ and src/client/providers/builtin.rs`) and the 42-surface `StealthScriptBundle` (`src/client/script.rs`).

## 1. Fluent builder, JSON profiles, and validation

All fingerprint fields live on `Fingerprint` and serialize cleanly with JSON field names matching `FingerprintBuilder` setters:

```rust
use seleniumbase_rs_stealth::identity::fingerprint::{
    AudioNoise, CanvasNoise, Fingerprint, OsType, WebRtcPolicy,
};
use seleniumbase_rs_stealth::identity::settings::{ColorGamut, DynamicRange, MediaFeatures, WebGl};

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

let report = fp.validate();
assert!(report.is_coherent());
Ok::<(), Box<dyn std::error::Error>>(())
```

Before launching a browser, `Fingerprint::validate()` produces a `CoherenceReport` checking:

- **OS & User-Agent agreement** (`navigator.platform`, mobile `maxTouchPoints`, desktop vs mobile GPU strings).
- **Graphics & WebGL consistency** (e.g., rejecting Direct3D/ANGLE renderers paired with macOS or Linux).
- **Client Hints coherence** (`Sec-CH-UA` major versions matching `core_version`, `platform`, `mobile`, `formFactors`, and brand versions).
- **Locale & timezone pairing** via `randomize::Place` and `KeyboardLayout`.

You can also draw a complete, internally consistent persona from a single seed:

```rust
use seleniumbase_rs_stealth::identity::fingerprint::{Fingerprint, OsType};

let windows = Fingerprint::randomized(OsType::Windows, 42);
let chrome_156 = Fingerprint::randomized_with_major(OsType::Macos, 99, 156);
assert!(windows.validate().is_coherent());
assert_eq!(chrome_156.core_version(), Some(156));
```

## 2. Recorded specimens (`Spoof::Mimic`)

Rather than assembling device parameters manually, `Spoof::Mimic(Specimen)` (`src/identity/ and src/client/mimic.rs`) pins individual settings or entire dimensions to real, cataloged browser builds and hardware rows:

| Specimen kind | Example use | What it supplies |
|---|---|---|
| `Specimen::Build(Build)` | Chrome or Edge desktop/mobile release | Real frozen UA format, `core_version`, brand matrix, known full build version with provenance (`Provenance`) |
| `Specimen::Machine(Machine)` | `macbook-13-m1`, `windows-uhd-620`, etc. | Coherent CPU cores, RAM, screen geometry, DPR, color gamut, WebGL vendor/renderer & extension tables |
| `Specimen::Place(Place)` | `de-de-berlin`, locale/region pairs | IANA timezone, locale, `Accept-Language`, keyboard layout, coordinates |
| `Specimen::Rendering(Rendering)` | Recorded build + machine outputs | Captured reference outputs for rendering probes |

## 3. Display, color gamut, and HDR (`MediaFeatures`)

Bot scripts compare CSS `@media` queries against JavaScript properties (`screen.colorDepth`, `screen.isExtended`, `devicePixelRatio`):

- **`ColorGamut`**: `Srgb` (typical Windows/Linux desktop), `P3` (Apple Retina displays and wide-gamut displays), and `Rec2020`.
- **`DynamicRange`**: `Standard` (24-bit SDR) vs `High` (HDR monitors paired with `dynamic-range: high` and `video-dynamic-range: high`).
- **`ScreenProvider` & `WindowGeometryProvider`**: Overrides `screen.*` dimensions, `availWidth`/`availHeight`, `isExtended`, `outerWidth`/`outerHeight`, and synchronizes `Emulation.setEmulatedMedia` when CDP native spoofing is enabled.

## 4. WebGL 1/2 parameter matrices and WebGPU

Changing only unmasked WebGL strings (`37445` / `37446`) is trivial to detect when WebGL2 limits disagree with the claimed GPU backend:

- **`WebglProvider`**: Hooks `getParameter` on both `WebGLRenderingContext` and `WebGL2RenderingContext`, emulating WebGL1 and WebGL2 constants (`MAX_TEXTURE_SIZE`, `MAX_RENDERBUFFER_SIZE`, `MAX_COMBINED_TEXTURE_IMAGE_UNITS`, `MAX_VERTEX_UNIFORM_VECTORS`, WebGL2 `MAX_ARRAY_TEXTURE_LAYERS` / `MAX_UNIFORM_BUFFER_BINDINGS` / `MAX_DRAW_BUFFERS` / `MAX_COLOR_ATTACHMENTS`), `getSupportedExtensions()` matrices per OS/backend, `getShaderPrecisionFormat`, and deterministic `readPixels` perturbation when `CanvasNoise` is active.
- **`WebGpuProvider`**: Aligns `navigator.gpu.requestAdapter()`, `GPUAdapterInfo` (`vendor`, `architecture`), `GPUAdapter.prototype.info`, and `requestAdapterInfo()` with the configured GPU maker (`nvidia`, `apple`, `intel`, `amd`). On Linux personas without Intel drivers, `requestAdapter()` gracefully returns `Promise.resolve(null)` to avoid impossible GPU claims.

## 5. Canvas, `OffscreenCanvas`, and `DOMRect` protection

Modern detectors fingerprint rendering outside `<canvas>.toDataURL()` alone:

- **`CanvasNoiseProvider`**: Injects seeded, coordinate-deterministic noise into:
  - `HTMLCanvasElement.prototype.toDataURL` and `toBlob` (using an offscreen noisy clone so visible canvases remain unaltered).
  - `CanvasRenderingContext2D.prototype.getImageData`.
  - `OffscreenCanvas.prototype.convertToBlob` and `OffscreenCanvasRenderingContext2D.prototype.getImageData`.
  - `measureText` wrappers aligned with both 2D and `OffscreenCanvas` contexts.
- **`DomRectProvider`**: Applies deterministic ~`1e-4` CSS pixel micro-jitter derived from the session seed and element bounds to `Element.prototype.getBoundingClientRect`, `Element.prototype.getClientRects`, `Range.prototype.getBoundingClientRect`, and `Range.prototype.getClientRects`. Exact identities (`right = left + width`, `bottom = top + height`) stay intact while breaking cross-run subpixel font/MathML/SVG box hashes.

## 6. Web Audio, latency, and `AnalyserNode` consistency

`AudioNoiseProvider` coordinates prototype overrides without attaching own properties on instances (preserving empty `Object.keys()` checks on audio nodes):

- Overrides `AudioContext.prototype.sampleRate` when `audio_sample_rate` is configured (`44100` or `48000`).
- Emulates hardware-plausible `AudioContext.prototype.baseLatency`, `AudioContext.prototype.outputLatency`, and `AudioDestinationNode.prototype.maxChannelCount` (`2`).
- Perturbs `AudioBuffer.prototype.getChannelData`, `AudioBuffer.prototype.copyFromChannel`, `OfflineAudioContext.prototype.startRendering`, and `AnalyserNode.prototype` (`getFloatFrequencyData`, `getByteFrequencyData`, `getFloatTimeDomainData`, `getByteTimeDomainData`).

## 7. Font enumeration and Client Hints coherence

- **`FontsProvider`**: Filters `document.fonts.check()` and `FontFaceSet.prototype.load()` against the profile's OS-appropriate font allowlist, and shields `CanvasRenderingContext2D.prototype.measureText` by substituting unknown requested families with generic fallbacks before measurement.
- **`ClientHintsProvider`**: When `native_spoofing` is off or not yet applied at launch, mirrors `navigator.userAgentData` in secure contexts—including `brands`, `fullVersionList`, `platform`, `platformVersion`, `architecture`, `bitness`, `model`, `uaFullVersion`, `wow64`, and `formFactors`. When `native_spoofing` is enabled, `Emulation.setUserAgentOverride` delivers the identical Client Hints metadata directly from the browser engine so HTTP headers and JS agree automatically.

## 8. Runtime hygiene and binary patching

- **`NativeToStringProvider`**: Preserves `[native code]` output from `Function.prototype.toString` and native property descriptor metadata for every injected hook.
- **`CdpMarkerProvider`**: Strips `cdc_` / `$cdc_` / `__webdriver*` / `__selenium*` / `__driver*` / `__fxdriver*` globals across prototype chains and filters evaluation stack traces (`__puppeteer_evaluation_script__`, `__playwright_evaluation_script__`, `UtilityScript.`, `ExecutionContext.evaluate`, `devtools://`, `__sbcdp`) inside `Error.prototype.stack` while leaving `Error.prepareStackTrace` untouched.
- **Engine-level scrubbing**: Pairs with [Binary patching](/stealth/advanced/binary-patching) (`ChromedriverPatcher`, `ChromeBinaryPatcher`, and `sbrs patch-chromedriver` / `sbrs patch-chrome`) to neutralize automation flags and markers before the browser process runs.

## 9. High-level synthesis in `seleniumbase-rs-stealth`

For multi-profile orchestration or persisting profiles across browser and wire runs, `seleniumbase-rs-stealth` (`seleniumbase_rs_stealth::fingerprint`) adds:

- **`FingerprintGenerator`**: One seeded builder producing a `GeneratedFingerprint` bundled with a `StealthScriptBundle` and coherent `ProfileFingerprint` (including `DisplayMetadata`, `GpuMetadata`, `WebGlParameters::for_gpu`, `WebGpuMetadata::for_gpu`, `MediaDeviceMetadata::synthetic`, `AudioMetadata`, `BatteryMetadata`, `NetworkMetadata`, and `ClientHintsHighEntropy::for_browser`).
- **`ProfileFingerprint::to_fingerprint()` / `from_fingerprint()`**: Bidirectional conversion with `seleniumbase_rs_stealth::identity::fingerprint::Fingerprint`.

```rust
use seleniumbase_rs_stealth::identity::fingerprint::OsType;
use seleniumbase_rs_stealth::fingerprint::FingerprintGenerator;

let generated = FingerprintGenerator::new(2026)
    .os(OsType::Macos)
    .audio_sample_rate(48_000)
    .hdr(true)
    .generate();

let fp = generated.profile.to_fingerprint()?;
assert!(fp.validate().is_coherent());
Ok::<(), Box<dyn std::error::Error>>(())
```

<Columns cols={2}>

<Card title="Evasion providers" icon="microchip" color="#1a85d4" href="/stealth/advanced/providers">
  Execution order, priorities, and writing custom `EvasionProvider` passes.
</Card>

<Card title="Binary patching" icon="wrench" color="#d47628" href="/stealth/advanced/binary-patching">
  Inspect and scrub `chromedriver` and Chromium binaries before launch.
</Card>

<Card title="Network patching" icon="network-wired" color="#a631be" href="/concepts/network-patching">
  Understand TLS, HTTP/2, HTTP/3, and loopback MitM wire alignment.
</Card>

<Card title="Stealth settings" icon="sliders" color="#199f43" href="/api/stealth-settings">
  Full reference for `Fingerprint`, `Spoof`, and `StealthPolicy`.
</Card>

</Columns>
