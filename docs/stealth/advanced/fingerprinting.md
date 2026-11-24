# Advanced Fingerprinting & Low-Level Browser Impersonation

This document details the advanced fingerprinting architecture, parameters, calculations,
and low-level native spoofing evasions in `seleniumbase-rs` and `seleniumbase-rs-stealth`.

Modern anti-bot solutions (Cloudflare Turnstile, DataDome, Akamai, Kasada, FingerprintJS Pro,
PerimeterX/HUMAN) do not rely solely on `navigator.webdriver`. Instead, they evaluate
cross-layer consistency across hundreds of browser signals: WebGL rendering parameters and
extension matrices, audio frequency response curves, canvas font probe dimensions, high-entropy
Client Hints, screen color gamut and dynamic range (HDR), SVG/MathML metrics, and CDP evaluation
stack traces.

`seleniumbase-rs` bridges the high-level fingerprint generation capabilities of tools like
Scrapfly, Browserforge, and Apify with low-level engine patches (Camoufox, ShardBrowser,
Obscura, Cloak-CDP, Eoka) into a clean, idiomatic Rust API with strict type safety.

---

## 1. Unified Profile JSON Architecture

Rather than relying on fragmented, arbitrary keys, `seleniumbase-rs` defines a **canonical profile representation** where JSON field names map 1:1 to the public [`Fingerprint`](file:///Users/notlaggy/Documents/GitFolder/seleniumbase-rs/src/evasion/fingerprint.rs) and `FingerprintBuilder` methods:

```json
{
  "user_agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
  "platform": "Win32",
  "languages": ["en-US", "en"],
  "hardware_concurrency": 8,
  "device_memory": 8.0,
  "webgl_vendor": "Google Inc. (NVIDIA)",
  "webgl_renderer": "ANGLE (NVIDIA, NVIDIA GeForce RTX 4080 Direct3D11 vs_5_0 ps_5_0, D3D11)",
  "canvas_noise": "persistent",
  "audio_noise": "persistent",
  "webrtc_policy": "disable_non_proxied_udp",
  "audio_sample_rate": 48000,
  "screen_width": 1920,
  "screen_height": 1080,
  "device_pixel_ratio": 1.0,
  "color_depth": 24,
  "color_gamut": "srgb",
  "hdr": false,
  "timezone": "America/New_York"
}
```

### Clean Deserialization & Direct Accessors
- **`languages`**: Accepts either a clean JSON string array (`["en-US", "en"]`) or a standard comma-separated string (`"en-US,en"`).
- **Direct Accessors**:
  ```rust
  let fp = Fingerprint::from_json(json_text)?;
  assert_eq!(fp.webgl_vendor(), Some("Google Inc. (NVIDIA)"));
  assert_eq!(fp.canvas_noise(), CanvasNoise::Persistent);
  assert_eq!(fp.audio_sample_rate(), Some(48000));
  ```

---

## 2. Display Characteristics, Color Gamut & Dynamic Range (HDR)

Modern anti-bot detection examines media query consistency (`window.matchMedia`) to detect
headless emulators and mismatched hardware claims.

### Color Gamut (`ColorGamut`)
- **`Srgb`**: Standard gamut for ordinary desktop and laptop displays (Windows and Linux).
- **`P3` (Display P3)**: Wide gamut native to Apple Retina displays (macOS, iOS) and high-end OLED mobile devices.
- **`Rec2020`**: Ultra-wide gamut used in professional HDR mastering.

### Dynamic Range (`DynamicRange` / `HdrMode`)
Anti-fingerprinting scripts probe `@media (dynamic-range: high)` and `@media (video-dynamic-range: high)`:
```rust
use seleniumbase_rs::evasion::settings::{ColorGamut, DynamicRange, MediaFeatures};

let features = MediaFeatures {
    color_gamut: ColorGamut::P3,
    dynamic_range: DynamicRange::High,
    ..MediaFeatures::default()
};
```
When injected, `Emulation.setEmulatedMedia` overrides both `color-gamut` and `dynamic-range` in lockstep, ensuring that CSS `@media` rules, `matchMedia('(dynamic-range: high)')`, and `screen.colorDepth` (30-bit for HDR vs 24-bit for SDR) remain coherent.

---

## 3. WebGL Hardware Emulation & Extension Matrices

Simply overriding `UNMASKED_VENDOR_WEBGL` (37445) and `UNMASKED_RENDERER_WEBGL` (37446) is a known
detection vector on Pixelscan and CreepJS if auxiliary parameters disagree with the claimed hardware.

### Full Parameter Emulation
`WebglProvider` hooks `getParameter` on both `WebGLRenderingContext` and `WebGL2RenderingContext` to emulate:
1. `gl.VENDOR` (`7936`): Fixed to `'WebKit'`.
2. `gl.RENDERER` (`7937`): Fixed to `'WebKit WebGL'`.
3. `gl.SHADING_LANGUAGE_VERSION` (`35724`): `'WebGL GLSL ES 1.0 (OpenGL ES GLSL ES 1.0 Chromium)'`.
4. `gl.MAX_TEXTURE_SIZE` (`3379`) & `gl.MAX_RENDERBUFFER_SIZE` (`34024`):
   - Desktop: `16384`
   - Mobile: `8192` (or `4096`)
5. `gl.MAX_VERTEX_ATTRIBS` (`34921`): `16`.
6. `gl.MAX_COMBINED_TEXTURE_IMAGE_UNITS` (`34930`): `32`.
7. `gl.MAX_VARYING_VECTORS` (`36347`): `30` (Desktop) / `15` (Mobile).

### Realistic WebGL Extension Matrices
Real Chromium instances expose a specific matrix of WebGL extensions depending on the GPU backend (ANGLE Direct3D11 on Windows, ANGLE Metal on macOS, Desktop OpenGL on Linux):

```rust
use seleniumbase_rs::evasion::randomize::windows_d3d11_extensions;
use seleniumbase_rs::evasion::settings::WebGl;

let gl = WebGl::new("Google Inc. (NVIDIA)", "ANGLE (NVIDIA, GeForce RTX 4070 Direct3D11)")
    .extensions(windows_d3d11_extensions())
    .max_texture_size(16384);
```
- **Mobile Profiles**: Desktop texture compression extensions (`WEBGL_compressed_texture_s3tc`, `EXT_texture_compression_bptc`, `EXT_texture_compression_rgtc`) are stripped from `getSupportedExtensions()` and return `null` on `getExtension()`, matching mobile Mali and Adreno GPUs.

---

## 4. Web Audio API & Sample Rate Spoofing

Audio fingerprinting renders a synthetic audio signal through an oscillator and `DynamicsCompressorNode`, measuring the resulting floating-point buffer digest.

### Dual Protections in `AudioNoiseProvider`
1. **Sample Rate Alignment (`audio_sample_rate`)**:
   - macOS / Apple Silicon: `48000 Hz`
   - Windows 10/11: `48000 Hz` (modern) or `44100 Hz`
   - Linux: `44100 Hz`
   `AudioContext.prototype.sampleRate` is mocked with a native getter to match the hardware persona.
2. **Buffer Perturbation**:
   - `AudioBuffer.prototype.getChannelData` and `copyFromChannel` apply deterministic pseudo-random noise seeded by the profile's session seed.
3. **`OfflineAudioContext.prototype.startRendering` Hook**:
   - Hooks the rendering Promise resolution to ensure the output buffer is perturbed before the page can calculate an unmasked SHA-256 or MD5 audio hash.

---

## 5. Dual-Layer Font Protection & Canvas Probe Shield

Font fingerprinting attacks operate via two distinct vectors:
1. `document.fonts.check()` / `FontFaceSet.prototype.load()`.
2. Canvas text measurement: setting `ctx.font = '72px "' + font + '", monospace'` and probing `ctx.measureText('mmmmmmmmllli').width`.

`FontsProvider` deploys a two-tier shield:
- **FontFaceSet Restrictor**: Restricts loading and checking to the profile's allowed system fonts.
- **Canvas Probe Shield**: Hooks `CanvasRenderingContext2D.prototype.measureText`. When an unauthorized font family is detected in `this.font`, it temporarily substitutes the font specification with generic `sans-serif` before measuring, ensuring that probe scripts cannot measure host font bounding box differences:

```javascript
// CanvasRenderingContext2D measureText shield
const originalMeasure = CanvasRenderingContext2D.prototype.measureText;
CanvasRenderingContext2D.prototype.measureText = sb.native(function measureText(text) {
  const family = parseFamily(this.font);
  if (family && !available(family)) {
    const saved = this.font;
    try {
      this.font = this.font.replace(new RegExp(family, 'g'), 'sans-serif');
      return originalMeasure.apply(this, arguments);
    } finally {
      this.font = saved;
    }
  }
  return originalMeasure.apply(this, arguments);
}, 'measureText', originalMeasure);
```

---

## 6. Client Hints High-Entropy Calculations

Chromium's `navigator.userAgentData.getHighEntropyValues()` exposes architecture, bitness, and detailed platform versions that must match the HTTP `Sec-CH-UA-*` headers exactly.

`seleniumbase-rs` calculates coherent values based on the persona:
| OS Persona | `architecture` | `bitness` | `platformVersion` | `wow64` | `model` |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Windows 11** | `"x86"` | `"64"` | `"15.0.0"` | `false` | `""` |
| **Windows 10** | `"x86"` | `"64"` | `"10.0.0"` | `false` | `""` |
| **macOS (Apple Silicon)** | `"arm"` | `"64"` | `"15.0.0"` | `false` | `""` |
| **Linux** | `"x86"` | `"64"` | `"6.5.0"` | `false` | `""` |
| **Android (Pixel 8)** | `"arm"` | `"64"` | `"14.0.0"` | `false` | `"Pixel 8"` |

---

## 7. MathML & SVG Subpixel Metric Normalization

Advanced bots inspect subpixel rendering of MathML expressions (`<math>`, `<mfrac>`) and SVG paths (`SVGGraphicsElement.prototype.getBBox`).

Because different operating systems rasterize fonts and glyph curves with slightly different floating-point bounding boxes, `seleniumbase-rs` normalizes subpixel dimensions on DOM rect accessors (`SVGGraphicsElement.prototype.getBBox` and `Element.prototype.getBoundingClientRect`) to eliminate cross-platform rasterization leaks.

---

## 8. Chrome Proprietary APIs & Environment Hygiene

Real desktop Chromium environments expose unique browser APIs that headless runners frequently omit:
1. **`performance.memory` Heap Emulation**:
   - `jsHeapSizeLimit` (`4,294,705,152` bytes on 64-bit systems)
   - `totalJSHeapSize` and `usedJSHeapSize` realistic dynamic allocation objects.
2. **`navigator.pdfViewerEnabled`**: Emulated as `true` on desktop personas matching genuine Chrome.
3. **Pointer Accuracy & Hover Capabilities**:
   - Desktop: `@media (pointer: fine)` and `@media (hover: hover)`.
   - Mobile: `@media (pointer: coarse)` and `@media (hover: none)`.
4. **Shader Precision Formats (`gl.getShaderPrecisionFormat`)**:
   - Emulates hardware shader precision (`rangeMin: 127`, `rangeMax: 127`, `precision: 23`) to prevent GPU vendor mismatch detection.

---

## 9. CDP Inspection & Stack Frame Sanitization (Cloak-CDP / Obscura)

When browser automation tools execute JavaScript via CDP `Runtime.evaluate`, internal stack traces expose automation scripts.

### Native Function & Error Stack Sanitization
1. **`Function.prototype.toString`**: Every hooked function reports `function () { [native code] }` and retains its original descriptor properties (`writable`, `enumerable`, `configurable`).
2. **`Error.prototype.stack` Filtering**:
   ```javascript
   // Removes CDP evaluation artifacts
   error.stack.split('\n').filter(line => (
     !line.includes('__puppeteer_evaluation_script__') &&
     !line.includes('__playwright_evaluation_script__') &&
     !line.includes('eval at <anonymous>')
   )).join('\n');
   ```
3. **Binary Scrubbing**: `scrub_k_automation_controlled` and `scrub_enable_automation` in `patcher.rs` ensure `navigator.webdriver` is missing at the C++ engine level without relying solely on JavaScript prototype deletion.

---

## 10. Complete Rust Example

```rust
use seleniumbase_rs::evasion::fingerprint::{
    AudioNoise, CanvasNoise, Fingerprint, OsType, WebRtcPolicy,
};
use seleniumbase_rs::evasion::settings::{ColorGamut, DynamicRange, MediaFeatures, WebGl};
use seleniumbase_rs::evasion::randomize::windows_d3d11_extensions;
use seleniumbase_rs::sb_cdp::ContextOptions;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Build an advanced fingerprint
    let fp = Fingerprint::builder()
        .os_type(OsType::Windows)
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
        .languages_list(&["en-US", "en"])
        .hardware_concurrency(16)
        .device_memory(16.0)
        .webgl(
            WebGl::new("Google Inc. (NVIDIA)", "ANGLE (NVIDIA, GeForce RTX 4080 Direct3D11)")
                .extensions(windows_d3d11_extensions())
                .max_texture_size(16384)
        )
        .canvas_noise(CanvasNoise::Persistent)
        .audio_noise(AudioNoise::Persistent)
        .audio_sample_rate(48000)
        .webrtc_policy(Some(WebRtcPolicy::DisableNonProxiedUdp))
        .media_features(MediaFeatures {
            color_gamut: ColorGamut::Srgb,
            dynamic_range: DynamicRange::Standard,
            ..MediaFeatures::default()
        })
        .build()?;

    // 2. Export / Save profile as unified JSON
    let json_file = "profiles/stealth_rtx4080.json";
    fp.save(json_file)?;

    // 3. Launch an undetected browser context
    let options = ContextOptions::builder()
        .viewport(1920, 1080)
        .fingerprint(fp)
        .build()?;

    println!("Advanced fingerprint initialized and ready.");
    Ok(())
}
```
