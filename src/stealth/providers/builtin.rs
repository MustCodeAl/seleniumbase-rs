//! Built-in [`EvasionProvider`](super::EvasionProvider) implementations.
//!
//! Each provider spoofs one browser dimension. They are registered, in
//! priority order, by [`all`]. Snippets are intentionally self-contained IIFEs
//! so they can also be used individually.

use super::{EvasionContext, EvasionProvider};
use crate::stealth::fingerprint::{
    BrowserType, CanvasNoiseMode, Fingerprint, MaskingMode, NoiseMode, OsType, PopupMode,
};

/// Returns `true` when a masking mode should emit its evasion.
fn masked(mode: MaskingMode) -> bool {
    matches!(mode, MaskingMode::Mask | MaskingMode::Custom)
}

/// Returns every built-in provider as trait objects.
pub fn all() -> Vec<Box<dyn EvasionProvider>> {
    vec![
        Box::new(NativeToStringProvider),
        Box::new(WebdriverProvider),
        Box::new(CdpMarkerProvider),
        Box::new(ChromeRuntimeProvider),
        Box::new(NavigatorPropsProvider),
        Box::new(PermissionsProvider),
        Box::new(PluginsProvider),
        Box::new(WindowGeometryProvider),
        Box::new(WebglProvider),
        Box::new(CanvasNoiseProvider),
        Box::new(AudioNoiseProvider),
        Box::new(WebRtcProvider),
        Box::new(BatteryProvider),
        Box::new(ConnectionProvider),
        Box::new(MediaDevicesProvider),
        Box::new(FontsProvider),
        Box::new(SpeechProvider),
        Box::new(BluetoothProvider),
        Box::new(HeadlessProvider),
        Box::new(PrepareStackTraceProvider),
        Box::new(ClientHintsProvider),
        Box::new(MediaCodecsProvider),
        Box::new(TimezoneProvider),
        Box::new(LocalizationProvider),
        Box::new(GeolocationProvider),
        Box::new(HairlineProvider),
        Box::new(IframeProvider),
        Box::new(AttachShadowProvider),
        Box::new(TrackerBlockProvider),
    ]
}

const PROTO: &str = "Object.getPrototypeOf(navigator)";

/// Installs `window.__sbNative`, a helper that marks a replacement function so
/// `Function.prototype.toString` reports `[native code]` for it.
pub struct NativeToStringProvider;

impl EvasionProvider for NativeToStringProvider {
    fn name(&self) -> &str {
        "native_tostring"
    }
    fn priority(&self) -> i32 {
        5
    }
    fn applies(&self, fp: &Fingerprint) -> bool {
        // Always useful; masking mode Natural disables only if explicitly set.
        !matches!(fp.flags.native_tostring_masking, MaskingMode::Disabled)
    }
    fn script(&self, _ctx: &EvasionContext) -> Option<String> {
        Some(
            r#"(function() {
  const nativeToString = Function.prototype.toString;
  const masked = new WeakMap();
  const proxy = new Proxy(nativeToString, {
    apply(target, thisArg, args) {
      if (masked.has(thisArg)) {
        const name = masked.get(thisArg) || (thisArg && thisArg.name) || '';
        return 'function ' + name + '() { [native code] }';
      }
      return Reflect.apply(target, thisArg, args);
    }
  });
  Function.prototype.toString = proxy;
  Object.defineProperty(window, '__sbNative', {
    value: function(fn, name) { try { masked.set(fn, name || (fn && fn.name)); } catch (e) {} return fn; },
    configurable: true
  });
})();"#
                .to_owned(),
        )
    }
}

/// Removes the `navigator.webdriver` flag and common automation hooks.
pub struct WebdriverProvider;

impl EvasionProvider for WebdriverProvider {
    fn name(&self) -> &str {
        "webdriver"
    }
    fn priority(&self) -> i32 {
        10
    }
    fn script(&self, _ctx: &EvasionContext) -> Option<String> {
        Some(format!(
            r#"(function() {{
  try {{ delete {PROTO}.webdriver; }} catch (e) {{}}
  try {{ Object.defineProperty({PROTO}, 'webdriver', {{ get: function() {{ return undefined; }} }}); }} catch (e) {{}}
  const hooks = ['callPhantom', '_phantom', '__nightmare', '__selenium_unwrapped',
    '__webdriver_evaluate', '__driver_evaluate', '__webdriver_script_fn',
    '__driver_unwrapped', '__fxdriver_evaluate', '__selenium_evaluate',
    '__webdriver_script_func', '_Selenium_IDE_Recorder', 'domAutomation',
    'domAutomationController'];
  hooks.forEach(function(h) {{ try {{ delete window[h]; }} catch (e) {{}} }});
}})();"#
        ))
    }
}

/// Scrubs `$cdc_` / `__webdriver` / `__driver` markers from the prototype chain.
pub struct CdpMarkerProvider;

impl EvasionProvider for CdpMarkerProvider {
    fn name(&self) -> &str {
        "cdp_markers"
    }
    fn priority(&self) -> i32 {
        15
    }
    fn script(&self, _ctx: &EvasionContext) -> Option<String> {
        Some(
            r#"(function() {
  const patterns = [/^\$?cdc_[a-zA-Z0-9]{22}_/, /^\$?cdc_/, /__webdriver/, /__selenium/, /__driver/, /\$chrome_/, /__fxdriver/];
  function scrub(obj) {
    if (!obj) return;
    Object.getOwnPropertyNames(obj).forEach(function(key) {
      if (patterns.some(function(p) { return p.test(key); })) {
        try { delete obj[key]; } catch (e) {}
      }
    });
  }
  let o = window;
  while (o) { scrub(o); o = Object.getPrototypeOf(o); }
  scrub(document);
})();"#
                .to_owned(),
        )
    }
}

/// Stubs `window.chrome` (runtime, app, csi, loadTimes).
pub struct ChromeRuntimeProvider;

impl EvasionProvider for ChromeRuntimeProvider {
    fn name(&self) -> &str {
        "chrome_runtime"
    }
    fn priority(&self) -> i32 {
        20
    }
    fn applies(&self, fp: &Fingerprint) -> bool {
        // Chromium personas; gate on either the dedicated or navigator flag.
        masked(fp.flags.chrome_runtime_masking) || masked(fp.flags.navigator_masking)
    }
    fn script(&self, _ctx: &EvasionContext) -> Option<String> {
        Some(
            r#"(function() {
  window.chrome = window.chrome || {};
  window.chrome.app = {
    isInstalled: false,
    InstallState: { DISABLED: 'disabled', INSTALLED: 'installed', NOT_INSTALLED: 'not_installed' },
    RunningState: { CANNOT_RUN: 'cannot_run', READY_TO_RUN: 'ready_to_run', RUNNING: 'running' },
    getDetails: function() {},
    getIsInstalled: function() { return false; },
    installState: function() { return Promise.resolve('not_installed'); },
    runningState: function() { return 'cannot_run'; },
  };
  const startLoad = Date.now() / 1000;
  window.chrome.loadTimes = function() {
    return {
      requestTime: startLoad, startLoadTime: startLoad, commitLoadTime: startLoad + 0.05,
      finishDocumentLoadTime: startLoad + 0.1, finishLoadTime: startLoad + 0.2,
      firstPaintTime: startLoad + 0.15, firstPaintAfterLoadTime: 0,
      navigationType: 'Other', wasFetchedViaSpdy: false, wasNpnNegotiated: true,
      npnNegotiatedProtocol: 'h2', wasAlternateProtocolAvailable: false, connectionInfo: 'h2',
    };
  };
  window.chrome.csi = function() {
    return { startE: Date.now(), onloadT: Date.now(), pageT: 1000 + Math.random() * 200, tran: 15 };
  };
  window.chrome.runtime = {
    id: undefined,
    OnInstalledReason: { CHROME_UPDATE: 'chrome_update', INSTALL: 'install', SHARED_MODULE_UPDATE: 'shared_module_update', UPDATE: 'update' },
    OnRestartRequiredReason: { APP_UPDATE: 'app_update', OS_UPDATE: 'os_update', PERIODIC: 'periodic' },
    PlatformArch: { ARM: 'arm', ARM64: 'arm64', MIPS: 'mips', MIPS64: 'mips64', X86_32: 'x86-32', X86_64: 'x86-64' },
    PlatformOs: { ANDROID: 'android', CROS: 'cros', LINUX: 'linux', MAC: 'mac', OPENBSD: 'openbsd', WIN: 'win' },
    RequestUpdateCheckStatus: { NO_UPDATE: 'no_update', THROTTLED: 'throttled', UPDATE_AVAILABLE: 'update_available' },
    connect: function() { return { postMessage: function(){}, disconnect: function(){}, onDisconnect: { addListener: function(){} }, onMessage: { addListener: function(){} } }; },
    sendMessage: function() { if (arguments.length && typeof arguments[arguments.length - 1] === 'function') { arguments[arguments.length - 1]({}); } return Promise.resolve({}); },
    getURL: function(path) { return 'chrome-extension://' + (this.id || '') + (path || ''); },
    getManifest: function() { return {}; },
  };
})();"#
                .to_owned(),
        )
    }
}

/// Overrides navigator properties (UA, platform, cores, memory, vendor, etc.).
pub struct NavigatorPropsProvider;

impl EvasionProvider for NavigatorPropsProvider {
    fn name(&self) -> &str {
        "navigator_props"
    }
    fn priority(&self) -> i32 {
        30
    }
    fn applies(&self, fp: &Fingerprint) -> bool {
        masked(fp.flags.navigator_masking)
    }
    fn script(&self, ctx: &EvasionContext) -> Option<String> {
        let fp = ctx.fingerprint;
        let native = fp.flags.native_spoofing;
        let mut out = String::from("(function() {\n");
        let mut define = |prop: &str, value: String| {
            out.push_str(&format!(
                "  try {{ Object.defineProperty({PROTO}, '{prop}', {{ get: function() {{ return {value}; }}, configurable: true }}); }} catch (e) {{}}\n"
            ));
        };

        if !native {
            let platform = fp
                .platform
                .clone()
                .unwrap_or_else(|| fp.os_type.platform().to_owned());
            define(
                "platform",
                format!("'{}'", EvasionContext::escape(&platform)),
            );

            if let Some(ua) = fp.user_agent.as_deref() {
                define("userAgent", format!("'{}'", EvasionContext::escape(ua)));
                define("appVersion", {
                    let av = fp
                        .app_version
                        .clone()
                        .unwrap_or_else(|| ua.replacen("Mozilla/", "", 1));
                    format!("'{}'", EvasionContext::escape(&av))
                });
            }
        }
        if let Some(cores) = fp.hardware_concurrency {
            define("hardwareConcurrency", cores.to_string());
        }
        if let Some(mem) = fp.device_memory {
            define("deviceMemory", mem.to_string());
        }
        if let Some(touch) = fp.max_touch_points {
            define("maxTouchPoints", touch.to_string());
        }
        let vendor = fp.vendor.clone().unwrap_or_else(|| {
            if fp.browser_type == BrowserType::MobileSafari || fp.os_type == OsType::Ios {
                "Apple Computer, Inc.".to_owned()
            } else {
                "Google Inc.".to_owned()
            }
        });
        define("vendor", format!("'{}'", EvasionContext::escape(&vendor)));
        let product_sub = fp
            .product_sub
            .clone()
            .unwrap_or_else(|| "20030107".to_owned());
        define(
            "productSub",
            format!("'{}'", EvasionContext::escape(&product_sub)),
        );
        if let Some(oscpu) = fp.oscpu.as_deref() {
            define("oscpu", format!("'{}'", EvasionContext::escape(oscpu)));
        }
        if let Some(build) = fp.build_id.as_deref() {
            define("buildID", format!("'{}'", EvasionContext::escape(build)));
        }
        // pdfViewerEnabled is expected true on modern desktop Chrome.
        define("pdfViewerEnabled", "true".to_owned());
        out.push_str("})();");
        Some(out)
    }
}

/// Fixes `navigator.permissions.query` and `Notification.permission`.
pub struct PermissionsProvider;

impl EvasionProvider for PermissionsProvider {
    fn name(&self) -> &str {
        "permissions"
    }
    fn priority(&self) -> i32 {
        35
    }
    fn applies(&self, fp: &Fingerprint) -> bool {
        masked(fp.flags.navigator_masking)
    }
    fn script(&self, _ctx: &EvasionContext) -> Option<String> {
        Some(
            r#"(function() {
  if (!navigator.permissions || !navigator.permissions.query) return;
  const orig = navigator.permissions.query.bind(navigator.permissions);
  const patched = function(parameters) {
    if (parameters && parameters.name === 'notifications') {
      return Promise.resolve({
        state: (typeof Notification !== 'undefined' && Notification.permission) || 'default',
        onchange: null,
        addEventListener: function() {}, removeEventListener: function() {},
        dispatchEvent: function() { return true; },
      });
    }
    return orig(parameters);
  };
  navigator.permissions.query = (window.__sbNative || function(f){return f;})(patched, 'query');
})();"#
                .to_owned(),
        )
    }
}

/// Spoofs `navigator.plugins` / `navigator.mimeTypes` with 5 PDF-viewer plugins.
pub struct PluginsProvider;

impl EvasionProvider for PluginsProvider {
    fn name(&self) -> &str {
        "plugins"
    }
    fn priority(&self) -> i32 {
        40
    }
    fn applies(&self, fp: &Fingerprint) -> bool {
        masked(fp.flags.navigator_masking)
    }
    fn script(&self, _ctx: &EvasionContext) -> Option<String> {
        Some(
            r#"(function() {
  const data = [
    { name: 'PDF Viewer', filename: 'internal-pdf-viewer', description: 'Portable Document Format' },
    { name: 'Chrome PDF Viewer', filename: 'internal-pdf-viewer', description: 'Portable Document Format' },
    { name: 'Chromium PDF Viewer', filename: 'internal-pdf-viewer', description: 'Portable Document Format' },
    { name: 'Microsoft Edge PDF Viewer', filename: 'internal-pdf-viewer', description: 'Portable Document Format' },
    { name: 'WebKit built-in PDF', filename: 'internal-pdf-viewer', description: 'Portable Document Format' },
  ];
  const mimeDefs = [
    { type: 'application/pdf', suffixes: 'pdf', description: 'Portable Document Format' },
    { type: 'text/pdf', suffixes: 'pdf', description: 'Portable Document Format' },
  ];
  function make() {
    const plugins = data.map(function(d) {
      const p = Object.create(Plugin.prototype);
      Object.defineProperties(p, {
        name: { value: d.name }, filename: { value: d.filename },
        description: { value: d.description }, length: { value: mimeDefs.length },
      });
      return p;
    });
    const arr = Object.create(PluginArray.prototype);
    plugins.forEach(function(p, i) { arr[i] = p; arr[p.name] = p; });
    Object.defineProperties(arr, {
      length: { value: plugins.length },
      item: { value: function(i) { return plugins[i]; } },
      namedItem: { value: function(n) { return plugins.find(function(p){ return p.name === n; }) || null; } },
      refresh: { value: function() {} },
    });
    return arr;
  }
  function makeMimes() {
    const mimes = mimeDefs.map(function(m) {
      const mm = Object.create(MimeType.prototype);
      Object.defineProperties(mm, {
        type: { value: m.type }, suffixes: { value: m.suffixes },
        description: { value: m.description }, enabledPlugin: { value: null },
      });
      return mm;
    });
    const arr = Object.create(MimeTypeArray.prototype);
    mimes.forEach(function(m, i) { arr[i] = m; arr[m.type] = m; });
    Object.defineProperties(arr, {
      length: { value: mimes.length },
      item: { value: function(i) { return mimes[i]; } },
      namedItem: { value: function(n) { return mimes.find(function(m){ return m.type === n; }) || null; } },
    });
    return arr;
  }
  try { Object.defineProperty(Object.getPrototypeOf(navigator), 'plugins', { get: make, configurable: true }); } catch (e) {}
  try { Object.defineProperty(Object.getPrototypeOf(navigator), 'mimeTypes', { get: makeMimes, configurable: true }); } catch (e) {}
})();"#
                .to_owned(),
        )
    }
}

/// Spoofs `screen.*`, `window.outer*`, and `devicePixelRatio`.
pub struct WindowGeometryProvider;

impl EvasionProvider for WindowGeometryProvider {
    fn name(&self) -> &str {
        "window_geometry"
    }
    fn priority(&self) -> i32 {
        45
    }
    fn applies(&self, fp: &Fingerprint) -> bool {
        masked(fp.flags.screen_masking)
    }
    fn script(&self, ctx: &EvasionContext) -> Option<String> {
        let fp = ctx.fingerprint;
        let native = fp.flags.native_spoofing;
        let (w, h) = match (fp.screen_width, fp.screen_height) {
            (Some(w), Some(h)) => (w, h),
            _ => fp.os_type.default_screen(),
        };
        let pr = fp.pixel_ratio.unwrap_or(1.0);
        let depth = fp.color_depth.unwrap_or(24);
        let avail_h = h.saturating_sub(40);
        if native {
            // Width/height/scale are handled by CDP Emulation.setDeviceMetricsOverride.
            // Only colorDepth is not covered by CDP, so patch it minimally.
            return Some(format!(
                r#"(function() {{
  try {{ Object.defineProperty(window.Screen.prototype, 'colorDepth', {{ get: function() {{ return {depth}; }}, configurable: true }}); }} catch (e) {{}}
  try {{ Object.defineProperty(window.Screen.prototype, 'pixelDepth', {{ get: function() {{ return {depth}; }}, configurable: true }}); }} catch (e) {{}}
}})();"#
            ));
        }
        Some(format!(
            r#"(function() {{
  const defs = {{
    width: {w}, height: {h}, availWidth: {w}, availHeight: {avail_h},
    colorDepth: {depth}, pixelDepth: {depth},
  }};
  Object.keys(defs).forEach(function(k) {{
    try {{ Object.defineProperty(window.Screen.prototype, k, {{ get: function() {{ return defs[k]; }}, configurable: true }}); }} catch (e) {{}}
  }});
  try {{ Object.defineProperty(window, 'outerWidth', {{ get: function() {{ return {w}; }}, configurable: true }}); }} catch (e) {{}}
  try {{ Object.defineProperty(window, 'outerHeight', {{ get: function() {{ return {h}; }}, configurable: true }}); }} catch (e) {{}}
  try {{ Object.defineProperty(window, 'devicePixelRatio', {{ get: function() {{ return {pr}; }}, configurable: true }}); }} catch (e) {{}}
}})();"#
        ))
    }
}

/// Overrides WebGL `UNMASKED_VENDOR_WEBGL` / `UNMASKED_RENDERER_WEBGL`.
pub struct WebglProvider;

impl EvasionProvider for WebglProvider {
    fn name(&self) -> &str {
        "webgl"
    }
    fn priority(&self) -> i32 {
        50
    }
    fn applies(&self, fp: &Fingerprint) -> bool {
        masked(fp.flags.graphics_masking)
    }
    fn script(&self, ctx: &EvasionContext) -> Option<String> {
        let fp = ctx.fingerprint;
        let (default_vendor, default_renderer) = if fp.os_type == OsType::Ios {
            ("Apple Inc.", "Apple GPU")
        } else {
            ("Intel Inc.", "Intel Iris OpenGL Engine")
        };
        let vendor = fp.webgl_vendor.as_deref().unwrap_or(default_vendor);
        let renderer = fp.webgl_renderer.as_deref().unwrap_or(default_renderer);
        let vendor_id = fp.webgl_vendor_id.as_deref().unwrap_or("");
        let renderer_id = fp.webgl_renderer_id.as_deref().unwrap_or("");
        Some(format!(
            r#"(function() {{
  const vendor = '{v}';
  const renderer = '{r}';
  const vendorId = '{vid}';
  const rendererId = '{rid}';
  function patch(ctx) {{
    if (!ctx || !ctx.prototype) return;
    const orig = ctx.prototype.getParameter;
    const patched = function(parameter) {{
      if (parameter === 37445) return vendor;
      if (parameter === 37446) return renderer;
      return orig.call(this, parameter);
    }};
    ctx.prototype.getParameter = (window.__sbNative || function(f){{return f;}})(patched, 'getParameter');
    if (vendorId) {{
      Object.defineProperty(ctx.prototype, '__sbVendorId', {{ value: vendorId, configurable: true }});
    }}
    if (rendererId) {{
      Object.defineProperty(ctx.prototype, '__sbRendererId', {{ value: rendererId, configurable: true }});
    }}
  }}
  patch(window.WebGLRenderingContext);
  patch(window.WebGL2RenderingContext);
}})();"#,
            v = EvasionContext::escape(vendor),
            r = EvasionContext::escape(renderer),
            vid = EvasionContext::escape(vendor_id),
            rid = EvasionContext::escape(renderer_id),
        ))
    }
}

/// Deterministic canvas noise on `toDataURL` / `toBlob` / `getImageData`.
pub struct CanvasNoiseProvider;

impl EvasionProvider for CanvasNoiseProvider {
    fn name(&self) -> &str {
        "canvas_noise"
    }
    fn priority(&self) -> i32 {
        55
    }
    fn applies(&self, fp: &Fingerprint) -> bool {
        matches!(fp.flags.canvas_noise, CanvasNoiseMode::Mask)
    }
    fn script(&self, ctx: &EvasionContext) -> Option<String> {
        let seed = ctx.seed;
        Some(format!(
            r#"(function() {{
  let state = {seed} >>> 0;
  function rand() {{ state = (state * 1664525 + 1013904223) >>> 0; return state / 4294967296; }}
  function tweak(canvas) {{
    try {{
      const ctx = canvas.getContext('2d');
      if (!ctx || !canvas.width || !canvas.height) return;
      const img = ctx.getImageData(0, 0, canvas.width, canvas.height);
      const d = img.data;
      for (let i = 0; i < d.length; i += 4) {{
        const n = Math.floor(rand() * 3) - 1;
        d[i] = Math.max(0, Math.min(255, d[i] + n));
      }}
      ctx.putImageData(img, 0, 0);
    }} catch (e) {{}}
  }}
  const origToDataURL = HTMLCanvasElement.prototype.toDataURL;
  HTMLCanvasElement.prototype.toDataURL = (window.__sbNative || function(f){{return f;}})(function() {{
    tweak(this); return origToDataURL.apply(this, arguments);
  }}, 'toDataURL');
  const origToBlob = HTMLCanvasElement.prototype.toBlob;
  if (origToBlob) {{
    HTMLCanvasElement.prototype.toBlob = (window.__sbNative || function(f){{return f;}})(function() {{
      tweak(this); return origToBlob.apply(this, arguments);
    }}, 'toBlob');
  }}
  const origGetImageData = CanvasRenderingContext2D.prototype.getImageData;
  CanvasRenderingContext2D.prototype.getImageData = (window.__sbNative || function(f){{return f;}})(function() {{
    const img = origGetImageData.apply(this, arguments);
    const d = img.data;
    for (let i = 0; i < d.length; i += 4) {{
      const n = Math.floor(rand() * 3) - 1;
      d[i] = Math.max(0, Math.min(255, d[i] + n));
    }}
    return img;
  }}, 'getImageData');
}})();"#
        ))
    }
}

/// Deterministic audio noise on `AudioBuffer.getChannelData` and
/// `AnalyserNode.getFloatFrequencyData`.
pub struct AudioNoiseProvider;

impl EvasionProvider for AudioNoiseProvider {
    fn name(&self) -> &str {
        "audio_noise"
    }
    fn priority(&self) -> i32 {
        60
    }
    fn applies(&self, fp: &Fingerprint) -> bool {
        masked(fp.flags.audio_masking)
    }
    fn script(&self, ctx: &EvasionContext) -> Option<String> {
        let seed = ctx.seed ^ 0x5DEE_CE66;
        Some(format!(
            r#"(function() {{
  let state = {seed} >>> 0;
  function rand() {{ state = (state * 1664525 + 1013904223) >>> 0; return state / 4294967296 - 0.5; }}
  if (window.AudioBuffer) {{
    const orig = AudioBuffer.prototype.getChannelData;
    AudioBuffer.prototype.getChannelData = (window.__sbNative || function(f){{return f;}})(function() {{
      const data = orig.apply(this, arguments);
      for (let i = 0; i < data.length; i += 100) {{ data[i] = data[i] + rand() * 1e-7; }}
      return data;
    }}, 'getChannelData');
  }}
  if (window.AnalyserNode) {{
    const origF = AnalyserNode.prototype.getFloatFrequencyData;
    AnalyserNode.prototype.getFloatFrequencyData = (window.__sbNative || function(f){{return f;}})(function(array) {{
      origF.call(this, array);
      for (let i = 0; i < array.length; i++) {{ array[i] += rand() * 0.0002; }}
    }}, 'getFloatFrequencyData');
  }}
  const AC = window.AudioContext || window.webkitAudioContext;
  if (AC) {{
    const origCreateAnalyser = AC.prototype.createAnalyser;
    AC.prototype.createAnalyser = (window.__sbNative || function(f){{return f;}})(function() {{
      const node = origCreateAnalyser.apply(this, arguments);
      const origGetFloat = node.getFloatFrequencyData.bind(node);
      node.getFloatFrequencyData = function(array) {{
        origGetFloat(array);
        for (let i = 0; i < array.length; i++) {{ array[i] += rand() * 0.0002; }}
      }};
      return node;
    }}, 'createAnalyser');
  }}
}})();"#
        ))
    }
}

/// Filters WebRTC STUN/TURN servers to prevent local-IP leaks.
pub struct WebRtcProvider;

impl EvasionProvider for WebRtcProvider {
    fn name(&self) -> &str {
        "webrtc"
    }
    fn priority(&self) -> i32 {
        65
    }
    fn applies(&self, fp: &Fingerprint) -> bool {
        masked(fp.flags.webrtc_masking)
    }
    fn script(&self, ctx: &EvasionContext) -> Option<String> {
        let public_ip = ctx.fingerprint.webrtc_public_ip.clone().unwrap_or_default();
        let local_ip = ctx.fingerprint.webrtc_local_ip.clone().unwrap_or_default();
        let has_custom_ips = !public_ip.is_empty() || !local_ip.is_empty();
        Some(format!(
            r#"(function() {{
  const RTC = window.RTCPeerConnection || window.webkitRTCPeerConnection;
  if (!RTC) return;
  function filterConfig(config) {{
    if (config && Array.isArray(config.iceServers)) {{
      config.iceServers = config.iceServers.filter(function(s) {{
        const urls = [].concat(s.urls || s.url || []);
        return urls.every(function(u) {{ return typeof u === 'string' && !/^stun:|^turn:/i.test(u); }});
      }});
    }}
    return config;
  }}
  function replaceIps(sdp) {{
    if (typeof sdp !== 'string') return sdp;
    return sdp.split(/\r?\n/).map(function(line) {{
      if ({has_public} && line.indexOf('a=candidate:') === 0) {{
        const parts = line.split(' ');
        if (parts.length >= 5) parts[4] = '{public_ip}';
        return parts.join(' ');
      }}
      if ({has_local} && line.indexOf('c=IN IP') === 0) {{
        const parts = line.split(' ');
        if (parts.length >= 3) parts[2] = '{local_ip}';
        return parts.join(' ');
      }}
      return line;
    }}).join('\r\n');
  }}
  const Patched = function(config, constraints) {{
    const pc = new RTC(filterConfig(config), constraints);
    if ({has_custom_ips}) {{
      const origCreateOffer = pc.createOffer.bind(pc);
      pc.createOffer = function(opts) {{ return origCreateOffer(opts).then(function(o) {{ if (o && o.sdp) o.sdp = replaceIps(o.sdp); return o; }}); }};
      const origSetLocal = pc.setLocalDescription.bind(pc);
      pc.setLocalDescription = function(desc) {{ if (desc && desc.sdp) desc.sdp = replaceIps(desc.sdp); return origSetLocal(desc); }};
    }}
    return pc;
  }};
  Patched.prototype = RTC.prototype;
  window.RTCPeerConnection = (window.__sbNative || function(f){{return f;}})(Patched, 'RTCPeerConnection');
  window.webkitRTCPeerConnection = window.RTCPeerConnection;
  if ({has_custom_ips} && window.RTCIceCandidate) {{
    const OrigCandidate = window.RTCIceCandidate;
    window.RTCIceCandidate = function(candidateInit) {{
      let c = candidateInit;
      if (typeof c === 'string') c = {{ candidate: c, sdpMid: '', sdpMLineIndex: 0 }};
      if (c && typeof c.candidate === 'string') {{
        if ({has_public}) {{
          const parts = c.candidate.split(' ');
          if (parts.length >= 5) parts[4] = '{public_ip}';
          c.candidate = parts.join(' ');
        }}
      }}
      return new OrigCandidate(c);
    }};
    window.RTCIceCandidate.prototype = OrigCandidate.prototype;
  }}
}})();"#,
            has_public = !public_ip.is_empty(),
            has_local = !local_ip.is_empty(),
            has_custom_ips = has_custom_ips,
            public_ip = EvasionContext::escape(&public_ip),
            local_ip = EvasionContext::escape(&local_ip),
        ))
    }
}

/// Spoofs the Battery Status API.
pub struct BatteryProvider;

impl EvasionProvider for BatteryProvider {
    fn name(&self) -> &str {
        "battery"
    }
    fn priority(&self) -> i32 {
        70
    }
    fn applies(&self, fp: &Fingerprint) -> bool {
        masked(fp.flags.battery_masking)
    }
    fn script(&self, ctx: &EvasionContext) -> Option<String> {
        let b = ctx.fingerprint.battery.clone().unwrap_or_default();
        let charging_time = if b.charging_time.is_finite() {
            b.charging_time.to_string()
        } else {
            "Infinity".to_owned()
        };
        let discharging_time = if b.discharging_time.is_finite() {
            b.discharging_time.to_string()
        } else {
            "Infinity".to_owned()
        };
        Some(format!(
            r#"(function() {{
  const info = {{ charging: {charging}, chargingTime: {charging_time}, dischargingTime: {discharging_time}, level: {level},
    addEventListener: function() {{}}, removeEventListener: function() {{}}, dispatchEvent: function() {{ return true; }},
    onchargingchange: null, onchargingtimechange: null, ondischargingtimechange: null, onlevelchange: null }};
  const getBattery = function() {{ return Promise.resolve(info); }};
  try {{ Object.defineProperty(Object.getPrototypeOf(navigator), 'getBattery', {{ value: (window.__sbNative || function(f){{return f;}})(getBattery, 'getBattery'), configurable: true }}); }} catch (e) {{}}
}})();"#,
            charging = b.charging,
            level = b.level,
        ))
    }
}

/// Spoofs the Network Information API (`navigator.connection`).
pub struct ConnectionProvider;

impl EvasionProvider for ConnectionProvider {
    fn name(&self) -> &str {
        "connection"
    }
    fn priority(&self) -> i32 {
        72
    }
    fn applies(&self, fp: &Fingerprint) -> bool {
        masked(fp.flags.connection_masking)
    }
    fn script(&self, ctx: &EvasionContext) -> Option<String> {
        let c = ctx.fingerprint.connection.clone().unwrap_or_default();
        Some(format!(
            r#"(function() {{
  const conn = {{ effectiveType: '{et}', rtt: {rtt}, downlink: {downlink}, saveData: {save_data},
    type: 'wifi', downlinkMax: Infinity,
    addEventListener: function() {{}}, removeEventListener: function() {{}}, onchange: null,
    dispatchEvent: function() {{ return true; }} }};
  ['connection', 'mozConnection', 'webkitConnection'].forEach(function(prop) {{
    try {{ Object.defineProperty(Object.getPrototypeOf(navigator), prop, {{ get: function() {{ return conn; }}, configurable: true }}); }} catch (e) {{}}
  }});
}})();"#,
            et = EvasionContext::escape(&c.effective_type),
            rtt = c.rtt,
            downlink = c.downlink,
            save_data = c.save_data,
        ))
    }
}

/// Spoofs `navigator.mediaDevices.enumerateDevices`.
pub struct MediaDevicesProvider;

impl EvasionProvider for MediaDevicesProvider {
    fn name(&self) -> &str {
        "media_devices"
    }
    fn priority(&self) -> i32 {
        75
    }
    fn applies(&self, fp: &Fingerprint) -> bool {
        masked(fp.flags.media_devices_masking)
    }
    fn script(&self, ctx: &EvasionContext) -> Option<String> {
        let fp = ctx.fingerprint;
        if !fp.media_devices.is_empty() {
            let entries = fp
                .media_devices
                .iter()
                .map(|d| {
                    format!(
                        "{{ deviceId: '{}', groupId: '{}', kind: '{}', label: '{}', toJSON: function() {{ return this; }} }}",
                        EvasionContext::escape(&d.device_id),
                        EvasionContext::escape(&d.group_id),
                        EvasionContext::escape(&d.kind),
                        EvasionContext::escape(&d.label),
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            return Some(format!(
                r#"(function() {{
  if (!navigator.mediaDevices || !navigator.mediaDevices.enumerateDevices) return;
  const devices = [{entries}];
  const patched = function() {{ return Promise.resolve(devices.map(function(d) {{ return Object.assign({{}}, d, {{ toJSON: function() {{ return this; }} }}); }})); }};
  navigator.mediaDevices.enumerateDevices = (window.__sbNative || function(f){{return f;}})(patched, 'enumerateDevices');
}})();"#
            ));
        }
        let audio_in = fp.audio_inputs.unwrap_or(1);
        let audio_out = fp.audio_outputs.unwrap_or(1);
        let video_in = fp.video_inputs.unwrap_or(1);
        Some(format!(
            r#"(function() {{
  if (!navigator.mediaDevices || !navigator.mediaDevices.enumerateDevices) return;
  const labels = ['Default', 'Communications', 'Microphone', 'Camera', 'Speaker'];
  const patched = function() {{
    const out = [];
    for (let i = 0; i < {audio_in}; i++) out.push({{ deviceId: 'audioinput-' + i, groupId: 'grp-' + i, kind: 'audioinput', label: labels[i % labels.length], toJSON: function() {{ return this; }} }});
    for (let i = 0; i < {audio_out}; i++) out.push({{ deviceId: 'audiooutput-' + i, groupId: 'grp-' + i, kind: 'audiooutput', label: labels[i % labels.length], toJSON: function() {{ return this; }} }});
    for (let i = 0; i < {video_in}; i++) out.push({{ deviceId: 'videoinput-' + i, groupId: 'grp-' + i, kind: 'videoinput', label: 'Camera ' + i, toJSON: function() {{ return this; }} }});
    return Promise.resolve(out);
  }};
  navigator.mediaDevices.enumerateDevices = (window.__sbNative || function(f){{return f;}})(patched, 'enumerateDevices');
}})();"#
        ))
    }
}

/// Restricts font enumeration and loading to the configured font list.
pub struct FontsProvider;

impl EvasionProvider for FontsProvider {
    fn name(&self) -> &str {
        "fonts"
    }
    fn priority(&self) -> i32 {
        76
    }
    fn applies(&self, fp: &Fingerprint) -> bool {
        masked(fp.flags.fonts_masking) && !fp.fonts.is_empty()
    }
    fn script(&self, ctx: &EvasionContext) -> Option<String> {
        let fonts = ctx.fingerprint.fonts.clone();
        let families: Vec<String> = fonts
            .iter()
            .map(|f| format!("'{}'", EvasionContext::escape(f)))
            .collect();
        let set = families.join(", ");
        Some(format!(
            r#"(function() {{
  const allowed = new Set([{set}]);
  function makeFace(family) {{
    return {{ family: family, status: 'loaded', load: function() {{ return Promise.resolve(this); }}, }}
  }}
  function parseFamily(spec) {{
    if (typeof spec !== 'string') return '';
    return spec.replace(/['"]/g, '').split(',')[0].trim();
  }}
  if (document.fonts) {{
    const origLoad = document.fonts.load.bind(document.fonts);
    document.fonts.load = function(fontSpec, text) {{
      const family = parseFamily(fontSpec);
      if (allowed.has(family)) return Promise.resolve([makeFace(family)]);
      return origLoad(fontSpec, text);
    }};
    document.fonts.check = function(fontSpec, text) {{
      return allowed.has(parseFamily(fontSpec));
    }};
  }}
  if (window.FontFace) {{
    const Orig = window.FontFace;
    window.FontFace = function(family, source, descriptors) {{
      if (!allowed.has(parseFamily(family))) {{
        return new Orig('sans-serif', 'url(data:application/font-woff2;base64,)', descriptors);
      }}
      return new Orig(family, source, descriptors);
    }};
    window.FontFace.prototype = Orig.prototype;
  }}
}})();"#,
            set = set
        ))
    }
}

/// Spoofs `speechSynthesis.getVoices()`.
pub struct SpeechProvider;

impl EvasionProvider for SpeechProvider {
    fn name(&self) -> &str {
        "speech"
    }
    fn priority(&self) -> i32 {
        78
    }
    fn applies(&self, fp: &Fingerprint) -> bool {
        masked(fp.flags.speech_masking)
    }
    fn script(&self, ctx: &EvasionContext) -> Option<String> {
        let voices = if ctx.fingerprint.speech_voices.is_empty() {
            crate::stealth::fingerprint::SpeechVoice::default_set()
        } else {
            ctx.fingerprint.speech_voices.clone()
        };
        let entries = voices
            .iter()
            .map(|v| {
                format!(
                    "{{ name: '{n}', lang: '{l}', default: {d}, localService: {ls}, voiceURI: '{u}' }}",
                    n = EvasionContext::escape(&v.name),
                    l = EvasionContext::escape(&v.lang),
                    d = v.default,
                    ls = v.local_service,
                    u = EvasionContext::escape(&v.voice_uri),
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        Some(format!(
            r#"(function() {{
  if (!window.speechSynthesis) return;
  const voices = [{entries}];
  const patched = function() {{ return voices; }};
  try {{ Object.defineProperty(window.speechSynthesis, 'getVoices', {{ value: (window.__sbNative || function(f){{return f;}})(patched, 'getVoices'), configurable: true }}); }} catch (e) {{}}
}})();"#
        ))
    }
}

/// Provides a minimal Web Bluetooth stub.
pub struct BluetoothProvider;

impl EvasionProvider for BluetoothProvider {
    fn name(&self) -> &str {
        "bluetooth"
    }
    fn priority(&self) -> i32 {
        80
    }
    fn applies(&self, fp: &Fingerprint) -> bool {
        masked(fp.flags.bluetooth_masking)
    }
    fn script(&self, _ctx: &EvasionContext) -> Option<String> {
        Some(
            r#"(function() {
  if (navigator.bluetooth) return;
  const bt = {
    getAvailability: function() { return Promise.resolve(false); },
    requestDevice: function() { return Promise.reject(new DOMException('User cancelled', 'NotFoundError')); },
    addEventListener: function() {}, removeEventListener: function() {},
  };
  try { Object.defineProperty(Object.getPrototypeOf(navigator), 'bluetooth', { get: function() { return bt; }, configurable: true }); } catch (e) {}
})();"#
                .to_owned(),
        )
    }
}

/// Fixes headless-mode signals (matchMedia, missing chrome).
pub struct HeadlessProvider;

impl EvasionProvider for HeadlessProvider {
    fn name(&self) -> &str {
        "headless"
    }
    fn priority(&self) -> i32 {
        85
    }
    fn applies(&self, fp: &Fingerprint) -> bool {
        masked(fp.flags.headless_masking)
    }
    fn script(&self, _ctx: &EvasionContext) -> Option<String> {
        Some(
            r#"(function() {
  const orig = window.matchMedia;
  if (orig) {
    const patched = function(query) {
      const mql = orig.call(window, query);
      if (query === '(prefers-reduced-motion: reduce)' || query === '(prefers-color-scheme: dark)') {
        return { matches: false, media: query, onchange: null, addListener: function() {}, removeListener: function() {}, addEventListener: function() {}, removeEventListener: function() {}, dispatchEvent: function() { return false; } };
      }
      if (query === '(any-pointer: fine)') {
        return { matches: true, media: query, onchange: null, addListener: function() {}, removeListener: function() {}, addEventListener: function() {}, removeEventListener: function() {}, dispatchEvent: function() { return false; } };
      }
      return mql;
    };
    window.matchMedia = (window.__sbNative || function(f){return f;})(patched, 'matchMedia');
  }
  try { Object.defineProperty(document, 'hidden', { get: function() { return false; }, configurable: true }); } catch (e) {}
  try { Object.defineProperty(document, 'visibilityState', { get: function() { return 'visible'; }, configurable: true }); } catch (e) {}
  try { Object.defineProperty(window, 'outerWidth', { get: function() { return window.innerWidth; }, configurable: true }); } catch (e) {}
  try { Object.defineProperty(window, 'outerHeight', { get: function() { return window.innerHeight; }, configurable: true }); } catch (e) {}
})();"#
                .to_owned(),
        )
    }
}

/// Traps `Error.prepareStackTrace` assignments so anti-bot scripts cannot
/// detect CDP by inspecting stack-trace formatting.
pub struct PrepareStackTraceProvider;

impl EvasionProvider for PrepareStackTraceProvider {
    fn name(&self) -> &str {
        "prepare_stack_trace"
    }
    fn priority(&self) -> i32 {
        87
    }
    fn script(&self, _ctx: &EvasionContext) -> Option<String> {
        Some(
            r#"(function() {
  try {
    let current = Error.prepareStackTrace;
    Object.defineProperty(Error, 'prepareStackTrace', {
      get: function() { return current; },
      set: function(fn) { current = fn; },
      configurable: true
    });
  } catch (e) {}
})();"#
                .to_owned(),
        )
    }
}

/// Spoofs `HTMLMediaElement.canPlayType` to report support for common codecs
/// (H.264, AAC) that headless Chrome sometimes omits, a signal checked by
/// Turnstile/DataDome-style scripts.
pub struct MediaCodecsProvider;

impl EvasionProvider for MediaCodecsProvider {
    fn name(&self) -> &str {
        "media_codecs"
    }
    fn priority(&self) -> i32 {
        89
    }
    fn script(&self, _ctx: &EvasionContext) -> Option<String> {
        Some(
            r#"(function() {
  if (!HTMLMediaElement || !HTMLMediaElement.prototype.canPlayType) return;
  const orig = HTMLMediaElement.prototype.canPlayType;
  HTMLMediaElement.prototype.canPlayType = (window.__sbNative || function(f){return f;})(function(type) {
    const t = String(type).toLowerCase();
    if (t.indexOf('video/mp4') !== -1 || t.indexOf('video/webm') !== -1) return 'probably';
    if (t.indexOf('audio/mp4') !== -1 || t.indexOf('audio/mpeg') !== -1 || t.indexOf('audio/aac') !== -1) return 'probably';
    return orig.call(this, type);
  }, 'canPlayType');
})();"#
                .to_owned(),
        )
    }
}

/// Overrides `navigator.userAgentData` (Client Hints).
pub struct ClientHintsProvider;

impl EvasionProvider for ClientHintsProvider {
    fn name(&self) -> &str {
        "client_hints"
    }
    fn priority(&self) -> i32 {
        88
    }
    fn applies(&self, fp: &Fingerprint) -> bool {
        masked(fp.flags.client_hints_masking)
            && fp.client_hints.is_some()
            && !fp.flags.native_spoofing
    }
    fn script(&self, ctx: &EvasionContext) -> Option<String> {
        let ch = ctx.fingerprint.client_hints.as_ref()?;
        let brand_json = |list: &[crate::stealth::fingerprint::BrandVersion]| {
            list.iter()
                .map(|b| {
                    format!(
                        "{{ brand: '{}', version: '{}' }}",
                        EvasionContext::escape(&b.brand),
                        EvasionContext::escape(&b.version)
                    )
                })
                .collect::<Vec<_>>()
                .join(", ")
        };
        let brands = brand_json(&ch.brands);
        let full = brand_json(&ch.full_version_list);
        Some(format!(
            r#"(function() {{
  const brands = [{brands}];
  const fullList = [{full}];
  const highEntropy = {{
    architecture: '{arch}', bitness: '{bitness}', model: '{model}',
    platform: '{platform}', platformVersion: '{platform_version}',
    uaFullVersion: '{ua_full}', fullVersionList: fullList, brands: brands, mobile: {mobile},
  }};
  const uaData = {{
    brands: brands, mobile: {mobile}, platform: '{platform}',
    getHighEntropyValues: function(hints) {{
      const res = {{ brands: brands, mobile: {mobile}, platform: '{platform}' }};
      (hints || []).forEach(function(h) {{ if (h in highEntropy) res[h] = highEntropy[h]; }});
      return Promise.resolve(res);
    }},
    toJSON: function() {{ return {{ brands: brands, mobile: {mobile}, platform: '{platform}' }}; }},
  }};
  try {{ Object.defineProperty(Object.getPrototypeOf(navigator), 'userAgentData', {{ get: function() {{ return uaData; }}, configurable: true }}); }} catch (e) {{}}
}})();"#,
            arch = EvasionContext::escape(&ch.architecture),
            bitness = EvasionContext::escape(&ch.bitness),
            model = EvasionContext::escape(&ch.model),
            platform = EvasionContext::escape(&ch.platform),
            platform_version = EvasionContext::escape(&ch.platform_version),
            ua_full = EvasionContext::escape(&ch.ua_full_version),
            mobile = ch.mobile,
        ))
    }
}

/// Spoofs the time zone used by `Intl.DateTimeFormat` and `Date`.
pub struct TimezoneProvider;

impl EvasionProvider for TimezoneProvider {
    fn name(&self) -> &str {
        "timezone"
    }
    fn priority(&self) -> i32 {
        90
    }
    fn applies(&self, fp: &Fingerprint) -> bool {
        masked(fp.flags.timezone_masking) && !(fp.flags.native_spoofing && fp.timezone.is_some())
    }
    fn script(&self, ctx: &EvasionContext) -> Option<String> {
        let zone = ctx
            .fingerprint
            .timezone
            .as_deref()
            .unwrap_or("America/New_York");
        Some(format!(
            r#"(function() {{
  const zone = '{zone}';
  const OrigDTF = Intl.DateTimeFormat;
  const Patched = function(...args) {{
    if (!(this instanceof Patched)) return new Patched(...args);
    if (!args[1]) args[1] = {{}};
    if (!args[1].timeZone) args[1].timeZone = zone;
    return new OrigDTF(...args);
  }};
  Patched.prototype = OrigDTF.prototype;
  Patched.supportedLocalesOf = OrigDTF.supportedLocalesOf;
  Intl.DateTimeFormat = Patched;
  const origResolved = OrigDTF.prototype.resolvedOptions;
  OrigDTF.prototype.resolvedOptions = function() {{
    const opts = origResolved.call(this);
    opts.timeZone = zone;
    return opts;
  }};
}})();"#,
            zone = EvasionContext::escape(zone),
        ))
    }
}

/// Spoofs localization-related values.
pub struct LocalizationProvider;

impl EvasionProvider for LocalizationProvider {
    fn name(&self) -> &str {
        "localization"
    }
    fn priority(&self) -> i32 {
        92
    }
    fn applies(&self, fp: &Fingerprint) -> bool {
        (masked(fp.flags.localization_masking) || masked(fp.flags.navigator_masking))
            && !fp.flags.native_spoofing
    }
    fn script(&self, ctx: &EvasionContext) -> Option<String> {
        let fp = ctx.fingerprint;
        let langs = fp.languages.as_deref().unwrap_or("en-US,en");
        let list: Vec<&str> = langs
            .split(',')
            .map(|s| s.split(';').next().unwrap_or(s).trim())
            .collect();
        let primary = list.first().copied().unwrap_or("en-US");
        let array = list
            .iter()
            .map(|s| format!("'{}'", EvasionContext::escape(s)))
            .collect::<Vec<_>>()
            .join(", ");
        Some(format!(
            r#"(function() {{
  try {{ Object.defineProperty({PROTO}, 'language', {{ get: function() {{ return '{primary}'; }}, configurable: true }}); }} catch (e) {{}}
  try {{ Object.defineProperty({PROTO}, 'languages', {{ get: function() {{ return [{array}]; }}, configurable: true }}); }} catch (e) {{}}
}})();"#,
            primary = EvasionContext::escape(primary),
        ))
    }
}

/// Spoofs `navigator.geolocation.getCurrentPosition` (CDP fallback).
pub struct GeolocationProvider;

impl EvasionProvider for GeolocationProvider {
    fn name(&self) -> &str {
        "geolocation"
    }
    fn priority(&self) -> i32 {
        95
    }
    fn applies(&self, fp: &Fingerprint) -> bool {
        masked(fp.flags.geolocation_masking) && fp.latitude.is_some() && !fp.flags.native_spoofing
    }
    fn script(&self, ctx: &EvasionContext) -> Option<String> {
        let fp = ctx.fingerprint;
        let lat = fp.latitude.unwrap_or(0.0);
        let lon = fp.longitude.unwrap_or(0.0);
        let alt = fp.altitude.unwrap_or(0.0);
        let acc = fp.accuracy.unwrap_or(100.0);
        let allow = matches!(fp.flags.geolocation_popup, PopupMode::Allow);
        Some(format!(
            r#"(function() {{
  if (!navigator.geolocation) return;
  const coords = {{ latitude: {lat}, longitude: {lon}, altitude: {alt}, accuracy: {acc}, altitudeAccuracy: {acc}, heading: null, speed: null }};
  const pos = {{ coords: coords, timestamp: Date.now() }};
  const orig = navigator.geolocation.getCurrentPosition.bind(navigator.geolocation);
  navigator.geolocation.getCurrentPosition = (window.__sbNative || function(f){{return f;}})(function(success, error, options) {{
    if ({allow}) {{ setTimeout(function() {{ success && success(pos); }}, 0); return; }}
    return orig(success, error, options);
  }}, 'getCurrentPosition');
}})();"#
        ))
    }
}

/// Fixes the Modernizr hairline headless detection.
pub struct HairlineProvider;

impl EvasionProvider for HairlineProvider {
    fn name(&self) -> &str {
        "hairline"
    }
    fn priority(&self) -> i32 {
        110
    }
    fn applies(&self, fp: &Fingerprint) -> bool {
        matches!(
            fp.flags.graphics_noise,
            NoiseMode::Mask | NoiseMode::Natural
        )
    }
    fn script(&self, _ctx: &EvasionContext) -> Option<String> {
        Some(
            r#"(function() {
  const desc = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'offsetHeight');
  if (!desc || !desc.get) return;
  Object.defineProperty(HTMLDivElement.prototype, 'offsetHeight', {
    ...desc,
    get: function() { if (this.id === 'modernizr') return 1; return desc.get.apply(this); }
  });
})();"#
                .to_owned(),
        )
    }
}

/// Keeps iframe `contentWindow` self/window references consistent.
pub struct IframeProvider;

impl EvasionProvider for IframeProvider {
    fn name(&self) -> &str {
        "iframe"
    }
    fn priority(&self) -> i32 {
        120
    }
    fn script(&self, _ctx: &EvasionContext) -> Option<String> {
        Some(
            r#"(function() {
  const orig = document.createElement;
  document.createElement = (window.__sbNative || function(f){return f;})(function(tagName, options) {
    const el = orig.call(document, tagName, options);
    if (tagName && String(tagName).toLowerCase() === 'iframe') {
      try {
        const cw = el.contentWindow;
        if (cw) {
          Object.defineProperty(cw, 'self', { get: function() { return cw; } });
          Object.defineProperty(cw, 'window', { get: function() { return cw; } });
        }
      } catch (e) {}
    }
    return el;
  }, 'createElement');
})();"#
                .to_owned(),
        )
    }
}

/// Forces all `Element.attachShadow` calls to use `mode: 'open'`.
///
/// Closed shadow roots make automation difficult and can be a signal that the
/// page is running in a controllable environment. This provider does not
/// change the return value's API surface; it only ensures the shadow root is
/// reachable via `element.shadowRoot`.
pub struct AttachShadowProvider;

impl EvasionProvider for AttachShadowProvider {
    fn name(&self) -> &str {
        "attach_shadow"
    }
    fn priority(&self) -> i32 {
        125
    }
    fn applies(&self, fp: &Fingerprint) -> bool {
        !matches!(fp.flags.headless_masking, MaskingMode::Disabled)
    }
    fn script(&self, _ctx: &EvasionContext) -> Option<String> {
        Some(
            r#"(function() {
  if (!Element.prototype.attachShadow) return;
  const orig = Element.prototype.attachShadow;
  Element.prototype.attachShadow = (window.__sbNative || function(f){return f;})(function(options) {
    const opts = (options && typeof options === 'object') ? options : {};
    return orig.call(this, { ...opts, mode: 'open' });
  }, 'attachShadow');
})();"#
                .to_owned(),
        )
    }
}

/// Records a tracker/fingerprint-script host block list on `window`.
///
/// The actual blocking is best enforced by a CDP/network interceptor; this
/// snippet exposes the list so a page-side fetch guard can consult it, and
/// documents which hosts should be dropped.
pub struct TrackerBlockProvider;

impl EvasionProvider for TrackerBlockProvider {
    fn name(&self) -> &str {
        "tracker_block"
    }
    fn priority(&self) -> i32 {
        130
    }
    fn applies(&self, fp: &Fingerprint) -> bool {
        fp.flags.block_trackers
    }
    fn script(&self, ctx: &EvasionContext) -> Option<String> {
        let hosts = ctx.fingerprint.tracker_hosts();
        let array = hosts
            .iter()
            .map(|h| format!("'{}'", EvasionContext::escape(h)))
            .collect::<Vec<_>>()
            .join(", ");
        Some(format!(
            r#"(function() {{
  const blocked = [{array}];
  Object.defineProperty(window, '__sbBlockedTrackers', {{ value: blocked, configurable: true }});
  const origFetch = window.fetch;
  if (origFetch) {{
    window.fetch = (window.__sbNative || function(f){{return f;}})(function(input, init) {{
      const url = (typeof input === 'string') ? input : (input && input.url) || '';
      if (blocked.some(function(h) {{ return url.indexOf(h) !== -1; }})) {{
        return Promise.reject(new TypeError('Blocked by anti-detect profile'));
      }}
      return origFetch.apply(this, arguments);
    }}, 'fetch');
  }}
}})();"#
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stealth::fingerprint::{BrandVersion, ClientHints, Fingerprint};

    fn ctx_for(fp: &Fingerprint) -> EvasionContext<'_> {
        EvasionContext::new(fp)
    }

    #[test]
    fn all_providers_are_priority_ordered() {
        let providers = all();
        assert_eq!(providers.len(), 29);
        let mut last = i32::MIN;
        for p in &providers {
            assert!(p.priority() >= last, "provider {} out of order", p.name());
            last = p.priority();
        }
    }

    #[test]
    fn provider_names_are_unique() {
        let providers = all();
        let mut names: Vec<&str> = providers.iter().map(|p| p.name()).collect();
        names.sort_unstable();
        let count = names.len();
        names.dedup();
        assert_eq!(names.len(), count, "duplicate provider name");
    }

    #[test]
    fn webdriver_snippet_mentions_webdriver() {
        let fp = Fingerprint::windows_desktop();
        let script = WebdriverProvider.script(&ctx_for(&fp)).unwrap();
        assert!(script.contains("webdriver"));
    }

    #[test]
    fn webgl_substitutes_vendor_and_renderer() {
        let fp = Fingerprint::windows_desktop();
        let script = WebglProvider.script(&ctx_for(&fp)).unwrap();
        assert!(script.contains("Google Inc. (NVIDIA)"));
        assert!(script.contains("37445"));
        assert!(script.contains("37446"));
    }

    #[test]
    fn webgl_includes_optional_ids() {
        let mut fp = Fingerprint::windows_desktop();
        fp.webgl_vendor_id = Some("0x10de".into());
        fp.webgl_renderer_id = Some("0x1f91".into());
        let script = WebglProvider.script(&ctx_for(&fp)).unwrap();
        assert!(script.contains("__sbVendorId"));
        assert!(script.contains("__sbRendererId"));
        assert!(script.contains("0x10de"));
        assert!(script.contains("0x1f91"));
    }

    #[test]
    fn canvas_noise_substitutes_seed() {
        let fp = Fingerprint::windows_desktop();
        let ctx = ctx_for(&fp);
        let seed = ctx.seed;
        let script = CanvasNoiseProvider.script(&ctx).unwrap();
        assert!(script.contains(&format!("{seed} >>> 0")));
    }

    #[test]
    fn window_geometry_includes_screen_width() {
        let fp = Fingerprint::windows_desktop();
        let script = WindowGeometryProvider.script(&ctx_for(&fp)).unwrap();
        assert!(script.contains("1920"));
    }

    #[test]
    fn plugins_lists_pdf_viewers() {
        let fp = Fingerprint::windows_desktop();
        let script = PluginsProvider.script(&ctx_for(&fp)).unwrap();
        assert!(script.contains("PDF Viewer"));
        assert!(script.contains("application/pdf"));
    }

    #[test]
    fn battery_defaults_are_emitted() {
        let fp = Fingerprint::windows_desktop();
        let script = BatteryProvider.script(&ctx_for(&fp)).unwrap();
        assert!(script.contains("getBattery"));
        assert!(script.contains("Infinity"));
    }

    #[test]
    fn client_hints_only_applies_when_set() {
        let mut fp = Fingerprint::windows_desktop();
        assert!(!ClientHintsProvider.applies(&fp));
        fp.client_hints = Some(ClientHints {
            brands: vec![BrandVersion::new("Chromium", "124")],
            full_version_list: vec![BrandVersion::new("Chromium", "124.0.0.0")],
            platform: "Windows".to_owned(),
            platform_version: "15.0.0".to_owned(),
            architecture: "x86".to_owned(),
            bitness: "64".to_owned(),
            model: String::new(),
            ua_full_version: "124.0.0.0".to_owned(),
            mobile: false,
        });
        assert!(ClientHintsProvider.applies(&fp));
        let script = ClientHintsProvider.script(&ctx_for(&fp)).unwrap();
        assert!(script.contains("userAgentData"));
        assert!(script.contains("Chromium"));
    }

    #[test]
    fn tracker_block_gated_by_flag() {
        let mut fp = Fingerprint::windows_desktop();
        assert!(!TrackerBlockProvider.applies(&fp));
        fp.flags.block_trackers = true;
        assert!(TrackerBlockProvider.applies(&fp));
    }

    #[test]
    fn native_tostring_runs_first() {
        let providers = all();
        assert_eq!(providers[0].name(), "native_tostring");
    }
}
