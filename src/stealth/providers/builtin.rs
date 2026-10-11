//! Built-in [`EvasionProvider`](super::EvasionProvider) implementations.
//!
//! Each provider spoofs one browser dimension. They are registered, in
//! priority order, by [`all`]. Snippets are self-contained IIFEs, so they can
//! also be run one at a time.
//!
//! The snippets follow three rules, each checked against a real Chrome by
//! `tests/stealth_providers_chrome.rs`:
//!
//! - **Patch what the browser patches.** A replaced method goes on its
//!   prototype, and a replaced getter on the prototype that owns it, never on
//!   the instance, so `Object.getOwnPropertyNames(object)` is unchanged.
//! - **Look native.** Replacements are made with the `sb` helpers described in
//!   [`providers`](super), so their name, length, source and `prototype` are
//!   what the browser's own function would show.
//! - **Be the same on every read.** A getter returns one object, and noise is
//!   a function of the seed and the position, never of how many reads came
//!   before.

use super::{EvasionContext, EvasionProvider, SB_PREAMBLE};
use crate::stealth::fingerprint::{
    BrowserType, CanvasNoiseMode, Fingerprint, MaskingMode, NoiseMode, OsType, PopupMode,
};

/// Returns `true` when a masking mode should emit its evasion.
fn masked(mode: MaskingMode) -> bool {
    matches!(mode, MaskingMode::Mask | MaskingMode::Custom)
}

/// Fills `@NAME@` markers in a JavaScript template, in one pass.
///
/// `@SB@` is the [`SB_PREAMBLE`]. A marker that is not in `values` stays as
/// written, and text that is substituted in is never searched for markers, so
/// a value cannot smuggle in another one.
fn render(template: &str, values: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(template.len() + 512);
    let mut rest = template;
    while let Some(start) = rest.find('@') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let Some(end) = after.find('@') else {
            out.push('@');
            out.push_str(after);
            return out;
        };
        let key = &after[..end];
        if key == "SB" {
            out.push_str(SB_PREAMBLE);
            rest = &after[end + 1..];
        } else if let Some((_, value)) = values.iter().find(|(name, _)| *name == key) {
            out.push_str(value);
            rest = &after[end + 1..];
        } else {
            out.push('@');
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

/// A single-quoted JavaScript string literal for `text`.
fn quoted(text: &str) -> String {
    format!("'{}'", EvasionContext::escape(text))
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

/// The hash both noise providers draw from: the same `(seed, index)` always
/// gives the same 32-bit value, with no state between calls.
const NOISE_HASH: &str = r##"function hash(index) {
    let h = (seed ^ Math.imul(index + 1, 0x9E3779B1)) >>> 0;
    h = Math.imul(h ^ (h >>> 15), 0x85EBCA77) >>> 0;
    h = Math.imul(h ^ (h >>> 13), 0xC2B2AE3D) >>> 0;
    return (h ^ (h >>> 16)) >>> 0;
  }"##;

/// Installs the shared `sb` helpers and masks `Function.prototype.toString`,
/// so that replaced functions report `[native code]`.
///
/// The helpers live in the scope of the assembled bootstrap script, not on
/// `window`; see [`SB_PREAMBLE`]. Run on its own, outside
/// [`EvasionRegistry::bootstrap`](super::EvasionRegistry::bootstrap), this
/// snippet has no shared scope to put them in and leaves a `__sb` global
/// behind, so use the registry.
#[derive(Clone, Copy, Debug, Default)]
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
            r##"(function () {
  const nativeToString = Function.prototype.toString;
  const names = new WeakMap();
  const patchedToString = new Proxy(nativeToString, {
    apply: function (target, thisArg, args) {
      if (names.has(thisArg)) {
        return 'function ' + names.get(thisArg) + '() { [native code] }';
      }
      return Reflect.apply(target, thisArg, args);
    }
  });
  names.set(patchedToString, 'toString');
  Function.prototype.toString = patchedToString;
  function mark(fn, name) {
    names.set(fn, name);
    return fn;
  }
  // A stand-in for a method: named `name`, as long as `like`, with no
  // `prototype`, because a method defined with shorthand syntax has none.
  function native(fn, name, like) {
    if (typeof fn !== 'function') { return fn; }
    const label = String(name || (like && like.name) || fn.name || '');
    const holder = { [label](...args) { return Reflect.apply(fn, this, args); } };
    const wrapper = holder[label];
    try {
      Object.defineProperty(wrapper, 'length', { value: like ? like.length : fn.length, configurable: true });
    } catch (e) {}
    return mark(wrapper, label);
  }
  // A constructor keeps its own `prototype`, so it is renamed in place.
  function ctor(fn, name) {
    if (typeof fn !== 'function') { return fn; }
    const label = String(name || fn.name || '');
    try { Object.defineProperty(fn, 'name', { value: label, configurable: true }); } catch (e) {}
    return mark(fn, label);
  }
  // Replaces an accessor's getter and keeps the property's other attributes.
  function getter(object, property, fn) {
    const current = Object.getOwnPropertyDescriptor(object, property);
    Object.defineProperty(object, property, {
      get: native(fn, 'get ' + property),
      set: current ? current.set : undefined,
      enumerable: current ? current.enumerable : true,
      configurable: true
    });
  }
  try { __sb = { native: native, ctor: ctor, getter: getter }; } catch (e) {}
})();"##
                .to_owned(),
        )
    }
}

/// Makes `navigator.webdriver` read `false`, as in a browser nobody automates,
/// and removes common automation hooks from `window`.
#[derive(Clone, Copy, Debug, Default)]
pub struct WebdriverProvider;

impl EvasionProvider for WebdriverProvider {
    fn name(&self) -> &str {
        "webdriver"
    }
    fn priority(&self) -> i32 {
        10
    }
    fn script(&self, _ctx: &EvasionContext) -> Option<String> {
        Some(render(
            r##"(function () {
  @SB@
  sb.getter(Object.getPrototypeOf(navigator), 'webdriver', function () { return false; });
  const hooks = ['callPhantom', '_phantom', '__nightmare', '__selenium_unwrapped',
    '__webdriver_evaluate', '__driver_evaluate', '__webdriver_script_fn',
    '__driver_unwrapped', '__fxdriver_evaluate', '__selenium_evaluate',
    '__webdriver_script_func', '_Selenium_IDE_Recorder', 'domAutomation',
    'domAutomationController'];
  hooks.forEach(function (h) { try { delete window[h]; } catch (e) {} });
})();"##,
            &[],
        ))
    }
}

/// Scrubs `$cdc_` / `__webdriver` / `__driver` markers from the prototype chain.
#[derive(Clone, Copy, Debug, Default)]
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
            r##"(function() {
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
})();"##
                .to_owned(),
        )
    }
}

/// Stubs `window.chrome` (runtime, app, csi, loadTimes).
#[derive(Clone, Copy, Debug, Default)]
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
        Some(render(
            r##"(function () {
  @SB@
  window.chrome = window.chrome || {};
  window.chrome.app = {
    isInstalled: false,
    InstallState: { DISABLED: 'disabled', INSTALLED: 'installed', NOT_INSTALLED: 'not_installed' },
    RunningState: { CANNOT_RUN: 'cannot_run', READY_TO_RUN: 'ready_to_run', RUNNING: 'running' },
    getDetails: sb.native(function getDetails() {}, 'getDetails'),
    getIsInstalled: sb.native(function getIsInstalled() { return false; }, 'getIsInstalled'),
    installState: sb.native(function installState() { return Promise.resolve('not_installed'); }, 'installState'),
    runningState: sb.native(function runningState() { return 'cannot_run'; }, 'runningState'),
  };
  const startLoad = Date.now() / 1000;
  window.chrome.loadTimes = sb.native(function loadTimes() {
    return {
      requestTime: startLoad, startLoadTime: startLoad, commitLoadTime: startLoad + 0.05,
      finishDocumentLoadTime: startLoad + 0.1, finishLoadTime: startLoad + 0.2,
      firstPaintTime: startLoad + 0.15, firstPaintAfterLoadTime: 0,
      navigationType: 'Other', wasFetchedViaSpdy: false, wasNpnNegotiated: true,
      npnNegotiatedProtocol: 'h2', wasAlternateProtocolAvailable: false, connectionInfo: 'h2',
    };
  }, 'loadTimes');
  window.chrome.csi = sb.native(function csi() {
    return { startE: Date.now(), onloadT: Date.now(), pageT: 1000 + Math.random() * 200, tran: 15 };
  }, 'csi');
  window.chrome.runtime = {
    id: undefined,
    OnInstalledReason: { CHROME_UPDATE: 'chrome_update', INSTALL: 'install', SHARED_MODULE_UPDATE: 'shared_module_update', UPDATE: 'update' },
    OnRestartRequiredReason: { APP_UPDATE: 'app_update', OS_UPDATE: 'os_update', PERIODIC: 'periodic' },
    PlatformArch: { ARM: 'arm', ARM64: 'arm64', MIPS: 'mips', MIPS64: 'mips64', X86_32: 'x86-32', X86_64: 'x86-64' },
    PlatformOs: { ANDROID: 'android', CROS: 'cros', LINUX: 'linux', MAC: 'mac', OPENBSD: 'openbsd', WIN: 'win' },
    RequestUpdateCheckStatus: { NO_UPDATE: 'no_update', THROTTLED: 'throttled', UPDATE_AVAILABLE: 'update_available' },
    connect: sb.native(function connect() {
      return { postMessage: function(){}, disconnect: function(){}, onDisconnect: { addListener: function(){} }, onMessage: { addListener: function(){} } };
    }, 'connect'),
    sendMessage: sb.native(function sendMessage() {
      if (arguments.length && typeof arguments[arguments.length - 1] === 'function') { arguments[arguments.length - 1]({}); }
      return Promise.resolve({});
    }, 'sendMessage'),
    getURL: sb.native(function getURL(path) { return 'chrome-extension://' + (this.id || '') + (path || ''); }, 'getURL'),
    getManifest: sb.native(function getManifest() { return {}; }, 'getManifest'),
  };
})();"##,
            &[],
        ))
    }
}

/// Overrides navigator properties (UA, platform, cores, memory, vendor, etc.).
#[derive(Clone, Copy, Debug, Default)]
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
        let mut body = String::new();
        let mut define = |prop: &str, value: String| {
            body.push_str(&format!(
                "  try {{ sb.getter(proto, '{prop}', function () {{ return {value}; }}); }} catch (e) {{}}\n"
            ));
        };

        if !native {
            let platform = fp
                .platform
                .clone()
                .unwrap_or_else(|| fp.os_type.platform().to_owned());
            define("platform", quoted(&platform));

            if let Some(ua) = fp.user_agent.as_deref() {
                define("userAgent", quoted(ua));
                let app_version = fp
                    .app_version
                    .clone()
                    .unwrap_or_else(|| ua.replacen("Mozilla/", "", 1));
                define("appVersion", quoted(&app_version));
            }
        }
        if let Some(cores) = fp.hardware_concurrency {
            define("hardwareConcurrency", cores.to_string());
        }
        if let Some(mem) = fp.device_memory {
            define("deviceMemory", EvasionContext::number(mem));
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
        define("vendor", quoted(&vendor));
        let product_sub = fp
            .product_sub
            .clone()
            .unwrap_or_else(|| "20030107".to_owned());
        define("productSub", quoted(&product_sub));
        if let Some(oscpu) = fp.oscpu.as_deref() {
            define("oscpu", quoted(oscpu));
        }
        if let Some(build) = fp.build_id.as_deref() {
            define("buildID", quoted(build));
        }
        // pdfViewerEnabled is expected true on modern desktop Chrome.
        define("pdfViewerEnabled", "true".to_owned());

        Some(render(
            "(function () {\n  @SB@\n  const proto = Object.getPrototypeOf(navigator);\n@BODY@})();",
            &[("BODY", &body)],
        ))
    }
}

/// Makes `navigator.permissions.query` agree with `Notification.permission`.
///
/// A headless Chrome answers `denied` to one and `prompt` to the other, which
/// is a known tell. The answer for `notifications` becomes the permission
/// state that `Notification.permission` names (`default` is `prompt`); every
/// other query is the browser's own.
#[derive(Clone, Copy, Debug, Default)]
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
        Some(render(
            r##"(function () {
  @SB@
  if (typeof Permissions === 'undefined' || !Permissions.prototype.query) { return; }
  const original = Permissions.prototype.query;
  // Notification.permission says "default" where a PermissionState says "prompt".
  const states = { granted: 'granted', denied: 'denied', 'default': 'prompt' };
  Permissions.prototype.query = sb.native(function query(descriptor) {
    const result = original.apply(this, arguments);
    if (!(descriptor && descriptor.name === 'notifications' && typeof Notification !== 'undefined')) {
      return result;
    }
    return result.then(function (status) {
      try {
        Object.defineProperty(status, 'state', {
          value: states[Notification.permission] || status.state, enumerable: true, configurable: true
        });
      } catch (e) {}
      return status;
    });
  }, 'query', original);
})();"##,
            &[],
        ))
    }
}

/// Spoofs `navigator.plugins` / `navigator.mimeTypes` with 5 PDF-viewer plugins.
///
/// Both lists are built once and returned on every read, as Chrome does.
#[derive(Clone, Copy, Debug, Default)]
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
        Some(render(
            r##"(function () {
  @SB@
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
  let plugins = null;
  let mimes = null;
  function makePlugins() {
    const list = data.map(function (d) {
      const p = Object.create(Plugin.prototype);
      Object.defineProperties(p, {
        name: { value: d.name }, filename: { value: d.filename },
        description: { value: d.description }, length: { value: mimeDefs.length },
      });
      return p;
    });
    const arr = Object.create(PluginArray.prototype);
    list.forEach(function (p, i) { arr[i] = p; arr[p.name] = p; });
    Object.defineProperties(arr, {
      length: { value: list.length },
      item: { value: sb.native(function item(i) { return list[i] || null; }, 'item') },
      namedItem: { value: sb.native(function namedItem(n) { return list.find(function (p) { return p.name === n; }) || null; }, 'namedItem') },
      refresh: { value: sb.native(function refresh() {}, 'refresh') },
    });
    return arr;
  }
  function makeMimes() {
    const first = navigator.plugins[0];
    const list = mimeDefs.map(function (m) {
      const mm = Object.create(MimeType.prototype);
      Object.defineProperties(mm, {
        type: { value: m.type }, suffixes: { value: m.suffixes },
        description: { value: m.description }, enabledPlugin: { value: first },
      });
      return mm;
    });
    const arr = Object.create(MimeTypeArray.prototype);
    list.forEach(function (m, i) { arr[i] = m; arr[m.type] = m; });
    Object.defineProperties(arr, {
      length: { value: list.length },
      item: { value: sb.native(function item(i) { return list[i] || null; }, 'item') },
      namedItem: { value: sb.native(function namedItem(n) { return list.find(function (m) { return m.type === n; }) || null; }, 'namedItem') },
    });
    return arr;
  }
  const proto = Object.getPrototypeOf(navigator);
  try { sb.getter(proto, 'plugins', function () { return plugins || (plugins = makePlugins()); }); } catch (e) {}
  try { sb.getter(proto, 'mimeTypes', function () { return mimes || (mimes = makeMimes()); }); } catch (e) {}
})();"##,
            &[],
        ))
    }
}

/// Spoofs `screen.*`, `window.outer*`, and `devicePixelRatio`.
///
/// This provider owns the outer window size; the
/// [headless](HeadlessProvider) fixes leave it alone, so the two cannot undo
/// each other. A window is as large as the persona's screen, and never smaller
/// than the viewport it really has.
#[derive(Clone, Copy, Debug, Default)]
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
        let depth = depth.to_string();
        if native {
            // Width/height/scale are handled by CDP Emulation.setDeviceMetricsOverride.
            // Only colorDepth is not covered by CDP, so patch it minimally.
            return Some(render(
                r##"(function () {
  @SB@
  try { sb.getter(Screen.prototype, 'colorDepth', function () { return @DEPTH@; }); } catch (e) {}
  try { sb.getter(Screen.prototype, 'pixelDepth', function () { return @DEPTH@; }); } catch (e) {}
})();"##,
                &[("DEPTH", &depth)],
            ));
        }
        Some(render(
            r##"(function () {
  @SB@
  const defs = {
    width: @W@, height: @H@, availWidth: @W@, availHeight: @AVAIL_H@,
    colorDepth: @DEPTH@, pixelDepth: @DEPTH@,
  };
  Object.keys(defs).forEach(function (k) {
    try { sb.getter(Screen.prototype, k, function () { return defs[k]; }); } catch (e) {}
  });
  // outerWidth, outerHeight and devicePixelRatio are replaceable accessors that
  // live on window itself; keep their setter and enumerability.
  function replaceable(property, read) {
    const current = Object.getOwnPropertyDescriptor(window, property);
    Object.defineProperty(window, property, {
      get: sb.native(read, 'get ' + property),
      set: current ? current.set : undefined,
      enumerable: current ? current.enumerable : true,
      configurable: true
    });
  }
  try { replaceable('outerWidth', function () { return Math.max(@W@, window.innerWidth); }); } catch (e) {}
  try { replaceable('outerHeight', function () { return Math.max(@AVAIL_H@, window.innerHeight); }); } catch (e) {}
  try { replaceable('devicePixelRatio', function () { return @PR@; }); } catch (e) {}
})();"##,
            &[
                ("W", &w.to_string()),
                ("H", &h.to_string()),
                ("AVAIL_H", &avail_h.to_string()),
                ("DEPTH", &depth),
                ("PR", &EvasionContext::number(pr)),
            ],
        ))
    }
}

/// Overrides WebGL `UNMASKED_VENDOR_WEBGL` / `UNMASKED_RENDERER_WEBGL`.
///
/// The fingerprint's optional GPU vendor and renderer ids are not emitted: no
/// browser API shows them to a page, and a property to carry them would only
/// be one more thing to find.
#[derive(Clone, Copy, Debug, Default)]
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
        Some(render(
            r##"(function () {
  @SB@
  const vendor = @VENDOR@;
  const renderer = @RENDERER@;
  function patch(context) {
    if (!context || !context.prototype) { return; }
    const original = context.prototype.getParameter;
    context.prototype.getParameter = sb.native(function getParameter(parameter) {
      if (parameter === 37445) { return vendor; }
      if (parameter === 37446) { return renderer; }
      return original.apply(this, arguments);
    }, 'getParameter', original);
  }
  patch(window.WebGLRenderingContext);
  patch(window.WebGL2RenderingContext);
})();"##,
            &[("VENDOR", &quoted(vendor)), ("RENDERER", &quoted(renderer))],
        ))
    }
}

/// Deterministic canvas noise on `toDataURL` / `toBlob` / `getImageData`.
///
/// The noise is a function of the seed and the pixel's position, so reading
/// the same canvas twice gives the same bytes, and a sub-rectangle carries the
/// noise of the same pixels as the whole canvas. `toDataURL` and `toBlob`
/// read a noisy copy: the canvas itself is never changed.
#[derive(Clone, Copy, Debug, Default)]
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
        Some(render(
            r##"(function () {
  @SB@
  const seed = @SEED@ >>> 0;
  @HASH@
  // Nudges the red channel of each pixel by -1, 0 or +1. The pixel at
  // (x0 + column, y0 + row) always gets the same nudge.
  function perturb(data, width, x0, y0) {
    for (let i = 0; i < data.length; i += 4) {
      const pixel = i / 4;
      const column = pixel % width;
      const row = (pixel - column) / width;
      const nudge = (hash((y0 + row) * 65536 + x0 + column) % 3) - 1;
      if (nudge !== 0) { data[i] = Math.max(0, Math.min(255, data[i] + nudge)); }
    }
  }
  const createElement = Document.prototype.createElement;
  const getContext = HTMLCanvasElement.prototype.getContext;
  const originalGetImageData = CanvasRenderingContext2D.prototype.getImageData;
  const putImageData = CanvasRenderingContext2D.prototype.putImageData;
  const originalToDataURL = HTMLCanvasElement.prototype.toDataURL;
  const originalToBlob = HTMLCanvasElement.prototype.toBlob;
  // A noisy copy of a canvas, or null when it cannot be read (empty, or tainted
  // by another origin); then the genuine canvas answers, or throws, as it would.
  function noisyCopy(canvas) {
    try {
      if (!canvas.width || !canvas.height) { return null; }
      const copy = createElement.call(document, 'canvas');
      copy.width = canvas.width;
      copy.height = canvas.height;
      const context = getContext.call(copy, '2d');
      context.drawImage(canvas, 0, 0);
      const image = originalGetImageData.call(context, 0, 0, copy.width, copy.height);
      perturb(image.data, image.width, 0, 0);
      putImageData.call(context, image, 0, 0);
      return copy;
    } catch (e) {
      return null;
    }
  }
  HTMLCanvasElement.prototype.toDataURL = sb.native(function toDataURL() {
    return originalToDataURL.apply(noisyCopy(this) || this, arguments);
  }, 'toDataURL', originalToDataURL);
  if (originalToBlob) {
    HTMLCanvasElement.prototype.toBlob = sb.native(function toBlob() {
      return originalToBlob.apply(noisyCopy(this) || this, arguments);
    }, 'toBlob', originalToBlob);
  }
  CanvasRenderingContext2D.prototype.getImageData = sb.native(function getImageData(sx, sy) {
    const image = originalGetImageData.apply(this, arguments);
    perturb(image.data, image.width, Math.floor(Number(sx)) || 0, Math.floor(Number(sy)) || 0);
    return image;
  }, 'getImageData', originalGetImageData);
})();"##,
            &[("SEED", &ctx.seed.to_string()), ("HASH", NOISE_HASH)],
        ))
    }
}

/// Deterministic audio noise on `AudioBuffer.getChannelData`,
/// `AudioBuffer.copyFromChannel` and `AnalyserNode.getFloatFrequencyData`.
///
/// A buffer's samples are perturbed once, the first time any of them is read,
/// so later reads see the same values; the analyser's noise depends only on the
/// seed and the bin.
#[derive(Clone, Copy, Debug, Default)]
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
        Some(render(
            r##"(function () {
  @SB@
  const seed = @SEED@ >>> 0;
  @HASH@
  function noise(index) { return hash(index) / 4294967296 - 0.5; }
  if (window.AudioBuffer) {
    const originalGetChannelData = AudioBuffer.prototype.getChannelData;
    const originalCopyFromChannel = AudioBuffer.prototype.copyFromChannel;
    const done = new WeakSet();
    AudioBuffer.prototype.getChannelData = sb.native(function getChannelData(channel) {
      const data = originalGetChannelData.apply(this, arguments);
      if (!done.has(data.buffer)) {
        done.add(data.buffer);
        const offset = (Number(channel) || 0) * 100003;
        for (let i = 0; i < data.length; i += 100) { data[i] += noise(i + offset) * 1e-7; }
      }
      return data;
    }, 'getChannelData', originalGetChannelData);
    if (originalCopyFromChannel) {
      AudioBuffer.prototype.copyFromChannel = sb.native(function copyFromChannel(destination, channel) {
        this.getChannelData(channel);
        return originalCopyFromChannel.apply(this, arguments);
      }, 'copyFromChannel', originalCopyFromChannel);
    }
  }
  if (window.AnalyserNode) {
    const originalFloat = AnalyserNode.prototype.getFloatFrequencyData;
    AnalyserNode.prototype.getFloatFrequencyData = sb.native(function getFloatFrequencyData(array) {
      originalFloat.apply(this, arguments);
      for (let i = 0; i < array.length; i++) { array[i] += noise(i) * 0.0002; }
    }, 'getFloatFrequencyData', originalFloat);
  }
})();"##,
            &[("SEED", &seed.to_string()), ("HASH", NOISE_HASH)],
        ))
    }
}

/// Filters WebRTC STUN/TURN servers to prevent local-IP leaks.
#[derive(Clone, Copy, Debug, Default)]
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
        Some(render(
            r##"(function () {
  @SB@
  const RTC = window.RTCPeerConnection || window.webkitRTCPeerConnection;
  if (!RTC) { return; }
  const publicIp = @PUBLIC_IP@;
  const localIp = @LOCAL_IP@;
  const customIps = !!(publicIp || localIp);
  function filterConfig(config) {
    if (config && Array.isArray(config.iceServers)) {
      return Object.assign({}, config, {
        iceServers: config.iceServers.filter(function (s) {
          const urls = [].concat(s.urls || s.url || []);
          return urls.every(function (u) { return typeof u === 'string' && !/^(stuns?|turns?):/i.test(u); });
        })
      });
    }
    return config;
  }
  function replaceIps(sdp) {
    if (typeof sdp !== 'string') { return sdp; }
    return sdp.split(/\r?\n/).map(function (line) {
      if (publicIp && line.indexOf('a=candidate:') === 0) {
        const parts = line.split(' ');
        if (parts.length >= 5) { parts[4] = publicIp; }
        return parts.join(' ');
      }
      if (localIp && line.indexOf('c=IN IP') === 0) {
        const parts = line.split(' ');
        if (parts.length >= 3) { parts[2] = localIp; }
        return parts.join(' ');
      }
      return line;
    }).join('\r\n');
  }
  function decorate(pc) {
    const createOffer = pc.createOffer.bind(pc);
    pc.createOffer = function (opts) {
      return createOffer(opts).then(function (o) { if (o && o.sdp) { o.sdp = replaceIps(o.sdp); } return o; });
    };
    const setLocal = pc.setLocalDescription.bind(pc);
    pc.setLocalDescription = function (desc) {
      if (desc && desc.sdp) { desc.sdp = replaceIps(desc.sdp); }
      return setLocal(desc);
    };
  }
  const Patched = new Proxy(RTC, {
    construct: function (target, args, newTarget) {
      const pc = Reflect.construct(target, [filterConfig(args[0])].concat(args.slice(1)), newTarget);
      if (customIps) { decorate(pc); }
      return pc;
    }
  });
  sb.ctor(Patched, 'RTCPeerConnection');
  Object.defineProperty(RTC.prototype, 'constructor', { value: Patched, writable: true, configurable: true });
  window.RTCPeerConnection = Patched;
  if ('webkitRTCPeerConnection' in window) { window.webkitRTCPeerConnection = Patched; }
  if (customIps && window.RTCIceCandidate) {
    const Original = window.RTCIceCandidate;
    const PatchedCandidate = new Proxy(Original, {
      construct: function (target, args, newTarget) {
        let init = args[0];
        if (typeof init === 'string') { init = { candidate: init, sdpMid: '', sdpMLineIndex: 0 }; }
        if (publicIp && init && typeof init.candidate === 'string') {
          const parts = init.candidate.split(' ');
          if (parts.length >= 5) { parts[4] = publicIp; }
          init = Object.assign({}, init, { candidate: parts.join(' ') });
        }
        return Reflect.construct(target, [init].concat(args.slice(1)), newTarget);
      }
    });
    sb.ctor(PatchedCandidate, 'RTCIceCandidate');
    Object.defineProperty(Original.prototype, 'constructor', { value: PatchedCandidate, writable: true, configurable: true });
    window.RTCIceCandidate = PatchedCandidate;
  }
})();"##,
            &[
                ("PUBLIC_IP", &quoted(&public_ip)),
                ("LOCAL_IP", &quoted(&local_ip)),
            ],
        ))
    }
}

/// Spoofs the Battery Status API.
#[derive(Clone, Copy, Debug, Default)]
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
        Some(render(
            r##"(function () {
  @SB@
  const info = { charging: @CHARGING@, chargingTime: @CHARGING_TIME@, dischargingTime: @DISCHARGING_TIME@, level: @LEVEL@,
    addEventListener: function () {}, removeEventListener: function () {}, dispatchEvent: function () { return true; },
    onchargingchange: null, onchargingtimechange: null, ondischargingtimechange: null, onlevelchange: null };
  const getBattery = sb.native(function getBattery() { return Promise.resolve(info); }, 'getBattery');
  try { Object.defineProperty(Object.getPrototypeOf(navigator), 'getBattery', { value: getBattery, configurable: true }); } catch (e) {}
})();"##,
            &[
                ("CHARGING", if b.charging { "true" } else { "false" }),
                ("CHARGING_TIME", &EvasionContext::number(b.charging_time)),
                (
                    "DISCHARGING_TIME",
                    &EvasionContext::number(b.discharging_time),
                ),
                ("LEVEL", &EvasionContext::number(b.level)),
            ],
        ))
    }
}

/// Spoofs the Network Information API (`navigator.connection`).
///
/// `navigator.connection` stays the browser's own `NetworkInformation`; only
/// the four values Chrome reports (`effectiveType`, `rtt`, `downlink` and
/// `saveData`) are replaced, on the prototype that owns them. Chrome has no
/// `mozConnection` or `webkitConnection`, so none is added.
#[derive(Clone, Copy, Debug, Default)]
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
        Some(render(
            r##"(function () {
  @SB@
  if (typeof NetworkInformation === 'undefined') { return; }
  const proto = NetworkInformation.prototype;
  const values = { effectiveType: @EFFECTIVE_TYPE@, rtt: @RTT@, downlink: @DOWNLINK@, saveData: @SAVE_DATA@ };
  Object.keys(values).forEach(function (prop) {
    if (prop in proto) {
      try { sb.getter(proto, prop, function () { return values[prop]; }); } catch (e) {}
    }
  });
})();"##,
            &[
                ("EFFECTIVE_TYPE", &quoted(&c.effective_type)),
                ("RTT", &c.rtt.to_string()),
                ("DOWNLINK", &EvasionContext::number(c.downlink)),
                ("SAVE_DATA", if c.save_data { "true" } else { "false" }),
            ],
        ))
    }
}

/// Spoofs `navigator.mediaDevices.enumerateDevices`.
#[derive(Clone, Copy, Debug, Default)]
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
        let devices = if fp.media_devices.is_empty() {
            let audio_in = fp.audio_inputs.unwrap_or(1);
            let audio_out = fp.audio_outputs.unwrap_or(1);
            let video_in = fp.video_inputs.unwrap_or(1);
            render(
                r##"(function () {
    const labels = ['Default', 'Communications', 'Microphone', 'Camera', 'Speaker'];
    const out = [];
    for (let i = 0; i < @AUDIO_IN@; i++) out.push({ deviceId: 'audioinput-' + i, groupId: 'grp-' + i, kind: 'audioinput', label: labels[i % labels.length] });
    for (let i = 0; i < @AUDIO_OUT@; i++) out.push({ deviceId: 'audiooutput-' + i, groupId: 'grp-' + i, kind: 'audiooutput', label: labels[i % labels.length] });
    for (let i = 0; i < @VIDEO_IN@; i++) out.push({ deviceId: 'videoinput-' + i, groupId: 'grp-' + i, kind: 'videoinput', label: 'Camera ' + i });
    return out;
  })()"##,
                &[
                    ("AUDIO_IN", &audio_in.to_string()),
                    ("AUDIO_OUT", &audio_out.to_string()),
                    ("VIDEO_IN", &video_in.to_string()),
                ],
            )
        } else {
            let entries = fp
                .media_devices
                .iter()
                .map(|d| {
                    format!(
                        "{{ deviceId: {}, groupId: {}, kind: {}, label: {} }}",
                        quoted(&d.device_id),
                        quoted(&d.group_id),
                        quoted(&d.kind),
                        quoted(&d.label),
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{entries}]")
        };
        Some(render(
            r##"(function () {
  @SB@
  if (typeof MediaDevices === 'undefined' || !MediaDevices.prototype.enumerateDevices) { return; }
  const original = MediaDevices.prototype.enumerateDevices;
  const devices = @DEVICES@;
  MediaDevices.prototype.enumerateDevices = sb.native(function enumerateDevices() {
    return Promise.resolve(devices.map(function (d) {
      return Object.assign({}, d, { toJSON: function () { return this; } });
    }));
  }, 'enumerateDevices', original);
})();"##,
            &[("DEVICES", &devices)],
        ))
    }
}

/// Restricts font enumeration and loading to the configured font list.
///
/// `document.fonts.check` and `load` are replaced on `FontFaceSet.prototype`.
/// A font spec is read the way CSS reads the `font` shorthand: the family is
/// what follows the size (`bold 14px "Segoe UI", sans-serif` names
/// `Segoe UI`), and a bare family name is taken as it is. A spec that names an
/// allowed font, or only a generic family such as `sans-serif`, is available;
/// any other family is not.
#[derive(Clone, Copy, Debug, Default)]
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
        let set = ctx
            .fingerprint
            .fonts
            .iter()
            .map(|f| quoted(f))
            .collect::<Vec<_>>()
            .join(", ");
        Some(render(
            r##"(function () {
  @SB@
  const allowed = new Set([@FONTS@]);
  const generic = new Set(['serif', 'sans-serif', 'monospace', 'cursive', 'fantasy', 'system-ui',
    'ui-serif', 'ui-sans-serif', 'ui-monospace', 'ui-rounded', 'math', 'emoji', 'fangsong']);
  const size = /^(?:(?:\d+(?:\.\d+)?|\.\d+)(?:px|pt|pc|em|ex|ch|rem|vw|vh|vmin|vmax|cm|mm|in|q|%)|(?:xx-small|x-small|small|medium|large|x-large|xx-large|xxx-large|smaller|larger))(?:\/\S*)?$/i;
  // The first family of a font spec: after the size in a shorthand, else the whole text.
  function parseFamily(spec) {
    if (typeof spec !== 'string') { return ''; }
    const first = spec.split(',')[0].replace(/['"]/g, '').trim();
    const tokens = first.split(/\s+/);
    for (let i = 0; i < tokens.length - 1; i++) {
      if (size.test(tokens[i])) { return tokens.slice(i + 1).join(' '); }
    }
    return first;
  }
  function available(family) { return allowed.has(family) || generic.has(family.toLowerCase()); }
  function makeFace(family) {
    return { family: family, status: 'loaded', load: function () { return Promise.resolve(this); } };
  }
  if (window.FontFaceSet) {
    const originalLoad = FontFaceSet.prototype.load;
    const originalCheck = FontFaceSet.prototype.check;
    FontFaceSet.prototype.load = sb.native(function load(spec, text) {
      const family = parseFamily(spec);
      if (allowed.has(family)) { return Promise.resolve([makeFace(family)]); }
      return originalLoad.apply(this, arguments);
    }, 'load', originalLoad);
    FontFaceSet.prototype.check = sb.native(function check(spec, text) {
      return available(parseFamily(spec));
    }, 'check', originalCheck);
  }
  if (window.FontFace) {
    const Original = window.FontFace;
    const Patched = new Proxy(Original, {
      construct: function (target, args, newTarget) {
        if (!allowed.has(parseFamily(args[0]))) {
          args = ['sans-serif', 'url(data:application/font-woff2;base64,)', args[2]];
        }
        return Reflect.construct(target, args, newTarget);
      }
    });
    sb.ctor(Patched, 'FontFace');
    Object.defineProperty(Original.prototype, 'constructor', { value: Patched, writable: true, configurable: true });
    window.FontFace = Patched;
  }
})();"##,
            &[("FONTS", &set)],
        ))
    }
}

/// Spoofs `speechSynthesis.getVoices()`.
#[derive(Clone, Copy, Debug, Default)]
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
                    "{{ name: {}, lang: {}, default: {}, localService: {}, voiceURI: {} }}",
                    quoted(&v.name),
                    quoted(&v.lang),
                    v.default,
                    v.local_service,
                    quoted(&v.voice_uri),
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        Some(render(
            r##"(function () {
  @SB@
  if (typeof SpeechSynthesis === 'undefined' || !SpeechSynthesis.prototype.getVoices) { return; }
  const original = SpeechSynthesis.prototype.getVoices;
  const voices = [@VOICES@];
  SpeechSynthesis.prototype.getVoices = sb.native(function getVoices() { return voices.slice(); }, 'getVoices', original);
})();"##,
            &[("VOICES", &entries)],
        ))
    }
}

/// Provides a minimal Web Bluetooth stub.
#[derive(Clone, Copy, Debug, Default)]
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
        Some(render(
            r##"(function () {
  @SB@
  if (navigator.bluetooth) { return; }
  const bt = {
    getAvailability: function () { return Promise.resolve(false); },
    requestDevice: function () { return Promise.reject(new DOMException('User cancelled', 'NotFoundError')); },
    addEventListener: function () {}, removeEventListener: function () {},
  };
  try { sb.getter(Object.getPrototypeOf(navigator), 'bluetooth', function () { return bt; }); } catch (e) {}
})();"##,
            &[],
        ))
    }
}

/// Fixes headless-mode signals: `matchMedia` answers and page visibility.
///
/// It does not touch `window.outerWidth` or `outerHeight`; the
/// [window geometry](WindowGeometryProvider) provider owns those.
#[derive(Clone, Copy, Debug, Default)]
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
        Some(render(
            r##"(function () {
  @SB@
  const original = window.matchMedia;
  if (original) {
    const answer = function (query, matches) {
      return { matches: matches, media: query, onchange: null, addListener: function () {}, removeListener: function () {},
        addEventListener: function () {}, removeEventListener: function () {}, dispatchEvent: function () { return false; } };
    };
    window.matchMedia = sb.native(function matchMedia(query) {
      const list = original.apply(this, arguments);
      if (query === '(prefers-reduced-motion: reduce)' || query === '(prefers-color-scheme: dark)') { return answer(query, false); }
      if (query === '(any-pointer: fine)') { return answer(query, true); }
      return list;
    }, 'matchMedia', original);
  }
  if (typeof Document !== 'undefined') {
    try { sb.getter(Document.prototype, 'hidden', function () { return false; }); } catch (e) {}
    try { sb.getter(Document.prototype, 'visibilityState', function () { return 'visible'; }); } catch (e) {}
  }
})();"##,
            &[],
        ))
    }
}

/// Reserved for hiding `Error.prepareStackTrace`; contributes nothing.
///
/// This provider used to define an accessor for `Error.prepareStackTrace`
/// that stored and returned whatever was assigned, which changes nothing about
/// how stack traces are formatted. It also added an own accessor Chrome does
/// not have (`Object.getOwnPropertyDescriptor(Error, 'prepareStackTrace')` is
/// `undefined` in Chrome), so it made the page easier to spot and no harder to
/// detect as automated. The detection it was meant for, a script that reads a
/// stack getter while the DevTools protocol has `Runtime` enabled, cannot be
/// answered from page JavaScript. The provider stays registered so the name is
/// stable, but it never applies and emits no script.
#[derive(Clone, Copy, Debug, Default)]
pub struct PrepareStackTraceProvider;

impl EvasionProvider for PrepareStackTraceProvider {
    fn name(&self) -> &str {
        "prepare_stack_trace"
    }
    fn priority(&self) -> i32 {
        87
    }
    fn applies(&self, _fp: &Fingerprint) -> bool {
        false
    }
    fn script(&self, _ctx: &EvasionContext) -> Option<String> {
        None
    }
}

/// Makes `HTMLMediaElement.canPlayType` answer for the codecs Chrome plays.
///
/// A Chromium build without proprietary codecs says nothing can play H.264 or
/// AAC, which Turnstile/DataDome-style scripts treat as a tell. When the
/// browser itself says it cannot play an MP4/AAC/MP3 type, this answers the way
/// Chrome with those codecs does: `maybe` for a bare container, `probably` when
/// every listed codec is a known one, and an empty string otherwise. When the
/// browser already says it can, its answer stands, and WebM and everything else
/// are always the browser's own.
#[derive(Clone, Copy, Debug, Default)]
pub struct MediaCodecsProvider;

impl EvasionProvider for MediaCodecsProvider {
    fn name(&self) -> &str {
        "media_codecs"
    }
    fn priority(&self) -> i32 {
        89
    }
    fn script(&self, _ctx: &EvasionContext) -> Option<String> {
        Some(render(
            r##"(function () {
  @SB@
  if (typeof HTMLMediaElement === 'undefined' || !HTMLMediaElement.prototype.canPlayType) { return; }
  const original = HTMLMediaElement.prototype.canPlayType;
  const known = /^(?:avc[13](?:\.[0-9a-f]+)*|mp4a\.40\.(?:2|5|29)|mp4a\.6b|mp3|flac|opus|vp9|vp09(?:\.\d+)*|av01(?:\.[0-9a-z]+)*)$/i;
  const container = /^\s*(video\/mp4|audio\/mp4|audio\/x-m4a|audio\/aac|audio\/mpeg|audio\/mp3)\s*(?:;\s*codecs\s*=\s*"?([^"]*)"?\s*)?$/i;
  HTMLMediaElement.prototype.canPlayType = sb.native(function canPlayType(type) {
    const answer = original.apply(this, arguments);
    if (answer !== '') { return answer; }
    const match = container.exec(String(type));
    if (!match) { return answer; }
    if (match[2] === undefined) { return /^audio\/(aac|mpeg|mp3)$/i.test(match[1]) ? 'probably' : 'maybe'; }
    const codecs = match[2].split(',').map(function (c) { return c.trim(); }).filter(Boolean);
    return codecs.length > 0 && codecs.every(function (c) { return known.test(c); }) ? 'probably' : '';
  }, 'canPlayType', original);
})();"##,
            &[],
        ))
    }
}

/// Overrides `navigator.userAgentData` (Client Hints).
#[derive(Clone, Copy, Debug, Default)]
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
                        "{{ brand: {}, version: {} }}",
                        quoted(&b.brand),
                        quoted(&b.version)
                    )
                })
                .collect::<Vec<_>>()
                .join(", ")
        };
        Some(render(
            r##"(function () {
  @SB@
  const brands = [@BRANDS@];
  const fullList = [@FULL@];
  const highEntropy = {
    architecture: @ARCH@, bitness: @BITNESS@, model: @MODEL@,
    platform: @PLATFORM@, platformVersion: @PLATFORM_VERSION@,
    uaFullVersion: @UA_FULL@, fullVersionList: fullList, brands: brands, mobile: @MOBILE@,
  };
  const uaData = {
    brands: brands, mobile: @MOBILE@, platform: @PLATFORM@,
    getHighEntropyValues: function (hints) {
      const res = { brands: brands, mobile: @MOBILE@, platform: @PLATFORM@ };
      (hints || []).forEach(function (h) { if (h in highEntropy) res[h] = highEntropy[h]; });
      return Promise.resolve(res);
    },
    toJSON: function () { return { brands: brands, mobile: @MOBILE@, platform: @PLATFORM@ }; },
  };
  try { sb.getter(Object.getPrototypeOf(navigator), 'userAgentData', function () { return uaData; }); } catch (e) {}
})();"##,
            &[
                ("BRANDS", &brand_json(&ch.brands)),
                ("FULL", &brand_json(&ch.full_version_list)),
                ("ARCH", &quoted(&ch.architecture)),
                ("BITNESS", &quoted(&ch.bitness)),
                ("MODEL", &quoted(&ch.model)),
                ("PLATFORM", &quoted(&ch.platform)),
                ("PLATFORM_VERSION", &quoted(&ch.platform_version)),
                ("UA_FULL", &quoted(&ch.ua_full_version)),
                ("MOBILE", if ch.mobile { "true" } else { "false" }),
            ],
        ))
    }
}

/// Spoofs the time zone seen by `Intl.DateTimeFormat` and by `Date`.
///
/// The two are made to agree. `Intl.DateTimeFormat` gets the zone as its
/// default, unless a caller names another one, and `new Date(...)`'s local-time
/// methods report that zone: `getTimezoneOffset`, `getHours` and the other
/// local getters, `toString`, `toDateString`, `toTimeString` and the
/// `toLocale*String` family, daylight saving time included.
///
/// Not covered: the `Date` setters and constructor forms that take local
/// components (`new Date(2024, 0, 15)`), `Date.parse` of a string without an
/// offset, and a bare `Date()` call, which still use the machine's zone. When
/// the fingerprint is applied over the DevTools protocol,
/// `Emulation.setTimezoneOverride` already changes all of them in the browser
/// itself and this provider is not needed (see
/// [`StealthFlags::native_spoofing`](crate::stealth::fingerprint::StealthFlags::native_spoofing)).
#[derive(Clone, Copy, Debug, Default)]
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
        Some(render(
            r##"(function () {
  @SB@
  const zone = @ZONE@;
  const Original = Intl.DateTimeFormat;
  // An unknown zone name cannot be honoured; leave the browser's own.
  try { new Original('en-US', { timeZone: zone }); } catch (e) { return; }

  function withZone(args) {
    const options = args[1];
    if (options === undefined) {
      args[1] = { timeZone: zone };
    } else if (options !== null && typeof options === 'object' && options.timeZone === undefined) {
      args[1] = Object.assign({}, options, { timeZone: zone });
    }
    return args;
  }
  const Patched = new Proxy(Original, {
    apply: function (target, thisArg, args) { return Reflect.apply(target, thisArg, withZone(Array.from(args))); },
    construct: function (target, args, newTarget) { return Reflect.construct(target, withZone(Array.from(args)), newTarget); }
  });
  sb.ctor(Patched, 'DateTimeFormat');
  Object.defineProperty(Original.prototype, 'constructor', { value: Patched, writable: true, configurable: true });
  Intl.DateTimeFormat = Patched;

  // Date, in the same zone.
  const parts = new Original('en-US', { timeZone: zone, hourCycle: 'h23', year: 'numeric', month: 'numeric',
    day: 'numeric', hour: 'numeric', minute: 'numeric', second: 'numeric' });
  const zoneNames = new Original('en-US', { timeZone: zone, timeZoneName: 'long' });
  const proto = Date.prototype;
  const time = proto.getTime;
  const DAYS = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
  const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];

  // The local fields of a time, in the zone, and the zone's offset from UTC in minutes.
  function local(ms) {
    const p = {};
    parts.formatToParts(ms).forEach(function (x) { p[x.type] = x.value; });
    const f = { year: +p.year, month: +p.month - 1, day: +p.day, hour: +p.hour % 24, minute: +p.minute, second: +p.second };
    f.weekday = new Date(Date.UTC(f.year, f.month, f.day)).getUTCDay();
    f.offset = (Date.UTC(f.year, f.month, f.day, f.hour, f.minute, f.second) - Math.floor(ms / 1000) * 1000) / 60000;
    return f;
  }
  function pad(n, width) {
    let s = String(Math.abs(n));
    while (s.length < width) { s = '0' + s; }
    return s;
  }
  function gmt(offset) {
    const a = Math.abs(offset);
    return 'GMT' + (offset < 0 ? '-' : '+') + pad(Math.floor(a / 60), 2) + pad(a % 60, 2);
  }
  function zoneName(ms) {
    const part = zoneNames.formatToParts(ms).find(function (x) { return x.type === 'timeZoneName'; });
    return part ? part.value : zone;
  }
  function datePart(f) { return DAYS[f.weekday] + ' ' + MONTHS[f.month] + ' ' + pad(f.day, 2) + ' ' + pad(f.year, 4); }
  function timePart(f, ms) {
    return pad(f.hour, 2) + ':' + pad(f.minute, 2) + ':' + pad(f.second, 2) + ' ' + gmt(f.offset) + ' (' + zoneName(ms) + ')';
  }

  function replace(name, build) {
    const original = proto[name];
    proto[name] = sb.native(function () {
      const ms = time.call(this);
      if (ms !== ms) { return original.apply(this, arguments); }
      return build(local(ms), ms, arguments);
    }, name, original);
  }
  replace('getTimezoneOffset', function (f) { return 0 - f.offset; });
  replace('getFullYear', function (f) { return f.year; });
  replace('getMonth', function (f) { return f.month; });
  replace('getDate', function (f) { return f.day; });
  replace('getDay', function (f) { return f.weekday; });
  replace('getHours', function (f) { return f.hour; });
  replace('getMinutes', function (f) { return f.minute; });
  replace('getSeconds', function (f) { return f.second; });
  replace('toString', function (f, ms) { return datePart(f) + ' ' + timePart(f, ms); });
  replace('toDateString', function (f) { return datePart(f); });
  replace('toTimeString', function (f, ms) { return timePart(f, ms); });

  ['toLocaleString', 'toLocaleDateString', 'toLocaleTimeString'].forEach(function (name) {
    const original = proto[name];
    proto[name] = sb.native(function (locales, options) {
      return original.call(this, locales, withZone([locales, options])[1]);
    }, name, original);
  });
})();"##,
            &[("ZONE", &quoted(zone))],
        ))
    }
}

/// Spoofs localization-related values.
///
/// `navigator.languages` is one frozen array, returned on every read, as in
/// Chrome. Quality values in the fingerprint's list (`en;q=0.9`) are dropped.
#[derive(Clone, Copy, Debug, Default)]
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
        let mut list: Vec<&str> = langs
            .split(',')
            .map(|s| s.split(';').next().unwrap_or(s).trim())
            .filter(|s| !s.is_empty())
            .collect();
        if list.is_empty() {
            list.push("en-US");
        }
        let primary = list[0];
        let array = list
            .iter()
            .map(|s| quoted(s))
            .collect::<Vec<_>>()
            .join(", ");
        Some(render(
            r##"(function () {
  @SB@
  const languages = Object.freeze([@LANGUAGES@]);
  const proto = Object.getPrototypeOf(navigator);
  try { sb.getter(proto, 'language', function () { return @PRIMARY@; }); } catch (e) {}
  try { sb.getter(proto, 'languages', function () { return languages; }); } catch (e) {}
})();"##,
            &[("LANGUAGES", &array), ("PRIMARY", &quoted(primary))],
        ))
    }
}

/// Spoofs `navigator.geolocation.getCurrentPosition` / `watchPosition` (CDP
/// fallback).
///
/// The methods are replaced on `Geolocation.prototype`. When the popup mode is
/// not `Allow`, they are the browser's own.
#[derive(Clone, Copy, Debug, Default)]
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
        let acc = EvasionContext::number(fp.accuracy.unwrap_or(100.0));
        let allow = matches!(fp.flags.geolocation_popup, PopupMode::Allow);
        Some(render(
            r##"(function () {
  @SB@
  if (typeof Geolocation === 'undefined') { return; }
  const coords = { latitude: @LAT@, longitude: @LON@, altitude: @ALT@, accuracy: @ACC@, altitudeAccuracy: @ACC@, heading: null, speed: null };
  const allow = @ALLOW@;
  const originalGet = Geolocation.prototype.getCurrentPosition;
  const originalWatch = Geolocation.prototype.watchPosition;
  let nextWatch = 1;
  function answer(success) {
    setTimeout(function () { if (typeof success === 'function') { success({ coords: coords, timestamp: Date.now() }); } }, 0);
  }
  Geolocation.prototype.getCurrentPosition = sb.native(function getCurrentPosition(success, error, options) {
    if (!allow) { return originalGet.apply(this, arguments); }
    answer(success);
  }, 'getCurrentPosition', originalGet);
  Geolocation.prototype.watchPosition = sb.native(function watchPosition(success, error, options) {
    if (!allow) { return originalWatch.apply(this, arguments); }
    answer(success);
    return nextWatch++;
  }, 'watchPosition', originalWatch);
})();"##,
            &[
                ("LAT", &EvasionContext::number(fp.latitude.unwrap_or(0.0))),
                ("LON", &EvasionContext::number(fp.longitude.unwrap_or(0.0))),
                ("ALT", &EvasionContext::number(fp.altitude.unwrap_or(0.0))),
                ("ACC", &acc),
                ("ALLOW", if allow { "true" } else { "false" }),
            ],
        ))
    }
}

/// Fixes the Modernizr hairline headless detection.
#[derive(Clone, Copy, Debug, Default)]
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
        Some(render(
            r##"(function () {
  @SB@
  const current = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'offsetHeight');
  if (!current || !current.get) { return; }
  sb.getter(HTMLElement.prototype, 'offsetHeight', function () {
    if (this instanceof HTMLElement && this.id === 'modernizr') { return 1; }
    return current.get.call(this);
  });
})();"##,
            &[],
        ))
    }
}

/// Reserved for keeping iframe `contentWindow` references consistent;
/// contributes nothing.
///
/// This provider used to wrap `document.createElement` and, for an `iframe`,
/// patch `contentWindow.self` and `window`. A new iframe has no
/// `contentWindow` until it is inserted, so the patch never did anything,
/// and the wrapper made `createElement` an own property of `document`, which
/// Chrome's is not. Scripts that run before any page script, as the bootstrap
/// does, already run in every frame, srcdoc frames included. The provider
/// stays registered so the name is stable, but it never applies and emits no
/// script.
#[derive(Clone, Copy, Debug, Default)]
pub struct IframeProvider;

impl EvasionProvider for IframeProvider {
    fn name(&self) -> &str {
        "iframe"
    }
    fn priority(&self) -> i32 {
        120
    }
    fn applies(&self, _fp: &Fingerprint) -> bool {
        false
    }
    fn script(&self, _ctx: &EvasionContext) -> Option<String> {
        None
    }
}

/// Forces all `Element.attachShadow` calls to use `mode: 'open'`.
///
/// Closed shadow roots make automation difficult and can be a signal that the
/// page is running in a controllable environment. This provider does not
/// change the return value's API surface; it only ensures the shadow root is
/// reachable via `element.shadowRoot`.
#[derive(Clone, Copy, Debug, Default)]
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
        Some(render(
            r##"(function () {
  @SB@
  if (!Element.prototype.attachShadow) { return; }
  const original = Element.prototype.attachShadow;
  Element.prototype.attachShadow = sb.native(function attachShadow(options) {
    const opts = (options && typeof options === 'object') ? options : {};
    return original.call(this, { ...opts, mode: 'open' });
  }, 'attachShadow', original);
})();"##,
            &[],
        ))
    }
}

/// Blocks requests to tracker and fingerprint-script hosts from page script.
///
/// The guard is page-side and covers `fetch` (a string, a `URL` or a
/// `Request`), `XMLHttpRequest` and `navigator.sendBeacon`; a blocked request
/// fails the way an unreachable host does (`TypeError: Failed to fetch`, an
/// `error` event on the request, `false` from `sendBeacon`). A host matches
/// when it is a listed host or a subdomain of one, never because the name
/// appears somewhere in the URL. Scripts, images and frames loaded by markup are
/// not covered; block those with a network interceptor. The list lives in the
/// snippet's own scope, not on `window`.
#[derive(Clone, Copy, Debug, Default)]
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
            .map(|h| quoted(&h.to_ascii_lowercase()))
            .collect::<Vec<_>>()
            .join(", ");
        Some(render(
            r##"(function () {
  @SB@
  const blocked = [@HOSTS@];
  function isBlocked(target) {
    let host;
    try { host = new URL(String(target), location.href).hostname.toLowerCase(); } catch (e) { return false; }
    return blocked.some(function (h) { return host === h || host.endsWith('.' + h); });
  }
  if (window.fetch) {
    const originalFetch = window.fetch;
    window.fetch = sb.native(function fetch(input, init) {
      const target = (typeof Request !== 'undefined' && input instanceof Request) ? input.url : input;
      if (isBlocked(target)) { return Promise.reject(new TypeError('Failed to fetch')); }
      return originalFetch.apply(this, arguments);
    }, 'fetch', originalFetch);
  }
  if (window.XMLHttpRequest) {
    const originalOpen = XMLHttpRequest.prototype.open;
    // A blocked request is opened on a URL no request can succeed on, so it
    // fails with the browser's own error events and status 0.
    XMLHttpRequest.prototype.open = sb.native(function open(method, url) {
      if (isBlocked(url)) {
        const rest = Array.prototype.slice.call(arguments, 2);
        return originalOpen.apply(this, [method, 'about:blank'].concat(rest));
      }
      return originalOpen.apply(this, arguments);
    }, 'open', originalOpen);
  }
  if (navigator.sendBeacon) {
    const originalBeacon = navigator.sendBeacon;
    Navigator.prototype.sendBeacon = sb.native(function sendBeacon(url) {
      if (isBlocked(url)) { return false; }
      return originalBeacon.apply(this, arguments);
    }, 'sendBeacon', originalBeacon);
  }
})();"##,
            &[("HOSTS", &array)],
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stealth::fingerprint::{BatteryProfile, BrandVersion, ClientHints, Fingerprint};

    fn ctx_for(fp: &Fingerprint) -> EvasionContext<'_> {
        EvasionContext::new(fp)
    }

    /// A fingerprint that makes every provider apply.
    fn everything() -> Fingerprint {
        let mut fp = Fingerprint::windows_desktop();
        fp.flags.audio_masking = MaskingMode::Mask;
        fp.flags.media_devices_masking = MaskingMode::Mask;
        fp.flags.block_trackers = true;
        fp.fonts = vec!["Arial".to_owned()];
        fp.latitude = Some(1.0);
        fp.longitude = Some(2.0);
        fp.webgl_vendor_id = Some("0x10de".to_owned());
        fp.webgl_renderer_id = Some("0x1f91".to_owned());
        fp.webrtc_public_ip = Some("203.0.113.9".to_owned());
        fp.webrtc_local_ip = Some("10.0.0.2".to_owned());
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
            ..ClientHints::default()
        });
        fp
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
    fn webgl_gpu_ids_are_not_exposed_to_pages() {
        let mut fp = Fingerprint::windows_desktop();
        fp.webgl_vendor_id = Some("0x10de".into());
        fp.webgl_renderer_id = Some("0x1f91".into());
        let script = WebglProvider.script(&ctx_for(&fp)).unwrap();
        assert!(!script.contains("__sbVendorId") && !script.contains("__sbRendererId"));
        assert!(!script.contains("0x10de"));
        assert!(!script.contains("0x1f91"));
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
    fn noise_does_not_depend_on_a_running_generator() {
        // The old noise advanced a shared generator on every read; a stateless
        // hash is what makes two reads agree.
        let fp = Fingerprint::windows_desktop();
        for provider in [
            &CanvasNoiseProvider as &dyn EvasionProvider,
            &AudioNoiseProvider,
        ] {
            let script = provider.script(&ctx_for(&fp)).unwrap();
            assert!(script.contains("Math.imul"), "{}", provider.name());
            assert!(!script.contains("1664525"), "{}", provider.name());
        }
    }

    #[test]
    fn window_geometry_includes_screen_width() {
        let fp = Fingerprint::windows_desktop();
        let script = WindowGeometryProvider.script(&ctx_for(&fp)).unwrap();
        assert!(script.contains("1920"));
    }

    #[test]
    fn headless_leaves_the_outer_window_size_to_window_geometry() {
        let fp = Fingerprint::windows_desktop();
        let script = HeadlessProvider.script(&ctx_for(&fp)).unwrap();
        assert!(!script.contains("outerWidth") && !script.contains("outerHeight"));
        let script = WindowGeometryProvider.script(&ctx_for(&fp)).unwrap();
        assert!(script.contains("outerWidth") && script.contains("outerHeight"));
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
        let fp = everything();
        assert!(ClientHintsProvider.applies(&fp));
        let script = ClientHintsProvider.script(&ctx_for(&fp)).unwrap();
        assert!(script.contains("userAgentData"));
        assert!(script.contains("Chromium"));

        let fp = Fingerprint::windows_desktop();
        assert!(!ClientHintsProvider.applies(&fp));
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

    #[test]
    fn no_snippet_names_a_page_visible_global() {
        let fp = everything();
        for provider in all() {
            let Some(script) = provider.script(&ctx_for(&fp)) else {
                continue;
            };
            for forbidden in [
                "window.__sb",
                "__sbNative",
                "__sbVendorId",
                "__sbRenderer",
                "__sbBlocked",
            ] {
                assert!(
                    !script.contains(forbidden),
                    "{} leaves {forbidden} on the page",
                    provider.name()
                );
            }
        }
    }

    #[test]
    fn no_marker_is_left_unfilled() {
        let marker = regex::Regex::new(r"@[A-Z][A-Z_]*@").unwrap();
        let fp = everything();
        for provider in all() {
            let Some(script) = provider.script(&ctx_for(&fp)) else {
                continue;
            };
            assert!(
                !marker.is_match(&script),
                "{} has an unfilled marker: {:?}",
                provider.name(),
                marker.find(&script).map(|m| m.as_str())
            );
        }
    }

    #[test]
    fn providers_that_cannot_work_emit_nothing() {
        let fp = everything();
        for provider in [
            &PrepareStackTraceProvider as &dyn EvasionProvider,
            &IframeProvider,
        ] {
            assert!(!provider.applies(&fp), "{}", provider.name());
            assert!(
                provider.script(&ctx_for(&fp)).is_none(),
                "{}",
                provider.name()
            );
        }
    }

    #[test]
    fn a_number_that_rust_prints_as_inf_reaches_javascript_as_infinity() {
        let mut fp = Fingerprint::windows_desktop();
        fp.battery = Some(BatteryProfile {
            charging: false,
            level: f64::INFINITY,
            charging_time: f64::NEG_INFINITY,
            discharging_time: f64::NAN,
        });
        fp.connection = Some(crate::stealth::fingerprint::ConnectionProfile {
            effective_type: "4g".to_owned(),
            rtt: 1,
            downlink: f64::INFINITY,
            save_data: false,
        });
        fp.device_memory = Some(f64::INFINITY);
        fp.pixel_ratio = Some(f64::INFINITY);
        fp.latitude = Some(f64::NAN);
        let ident = regex::Regex::new(r"\binf\b|\bnan\b").unwrap();
        for provider in [
            &BatteryProvider as &dyn EvasionProvider,
            &ConnectionProvider,
            &NavigatorPropsProvider,
            &WindowGeometryProvider,
            &GeolocationProvider,
        ] {
            let script = provider.script(&ctx_for(&fp)).unwrap();
            assert!(
                !ident.is_match(&script),
                "{} prints a Rust float",
                provider.name()
            );
        }
        let battery = BatteryProvider.script(&ctx_for(&fp)).unwrap();
        assert!(battery.contains("level: Infinity"));
        assert!(battery.contains("chargingTime: -Infinity"));
        assert!(battery.contains("dischargingTime: NaN"));
    }

    #[test]
    fn text_from_a_fingerprint_is_escaped() {
        let mut fp = Fingerprint::windows_desktop();
        fp.user_agent = Some("a'; evil(); '\\".to_owned());
        let script = NavigatorPropsProvider.script(&ctx_for(&fp)).unwrap();
        assert!(script.contains(r"'a\'; evil(); \'\\'"));
    }

    #[test]
    fn render_fills_each_marker_once_and_never_searches_what_it_filled() {
        let out = render(
            "a @ONE@ b @TWO@ c @UNKNOWN@ d @ e",
            &[("ONE", "@TWO@"), ("TWO", "2")],
        );
        assert_eq!(out, "a @TWO@ b 2 c @UNKNOWN@ d @ e");
        assert_eq!(render("no markers", &[]), "no markers");
        assert_eq!(render("trailing @", &[]), "trailing @");
        assert!(render("@SB@", &[]).starts_with("const sb ="));
    }
}
