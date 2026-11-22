use serde_json::json;

use crate::browser::devtools::CdpClient;
use crate::error::SeleniumBaseError;

/// Comprehensive undetected-chrome (UC) stealth script injected into every new document.
///
/// The script removes `cdc_` markers, rewrites `navigator.webdriver`, mocks Chrome-only
/// objects, patches WebGL/permissions/media devices, and masks common automation
/// fingerprints. It is designed to run via `Page.addScriptToEvaluateOnNewDocument` so
/// it executes before any page scripts.
pub const UC_STEALTH_SCRIPT: &str = r#"
(function() {
  'use strict';

  // Native toString disguise: guarantees all patched functions report `[native code]`.
  const nativeToString = Function.prototype.toString;
  const customNames = new WeakMap();
  const patchedToString = new Proxy(nativeToString, {
    apply: function(target, thisArg, args) {
      if (customNames.has(thisArg)) {
        return 'function ' + customNames.get(thisArg) + '() { [native code] }';
      }
      return Reflect.apply(target, thisArg, args);
    }
  });
  customNames.set(patchedToString, 'toString');
  try { Function.prototype.toString = patchedToString; } catch(e) {}
  function markNative(fn, name) {
    if (typeof fn === 'function') {
      customNames.set(fn, name || fn.name || '');
    }
    return fn;
  }

  // 1. Remove known chromedriver markers from window and document.
  const cdcRegex = /^[a-z]{3}_[a-zA-Z0-9]{22}_.*/i;
  const scrub = (obj) => {
    if (!obj) return;
    try {
      Object.getOwnPropertyNames(obj).forEach(p => {
        if (cdcRegex.test(p) || /^\$?cdc_/i.test(p)) {
          try { delete obj[p]; } catch (e) {}
        }
      });
    } catch (e) {}
  };
  let w = window;
  while (w) { scrub(w); w = Object.getPrototypeOf(w); }
  if (typeof document !== 'undefined') {
    let d = document;
    while (d) { scrub(d); d = Object.getPrototypeOf(d); }
  }

  // 2. navigator.webdriver must be false (not undefined) on Navigator.prototype, not an own property on navigator.
  try {
    delete navigator.webdriver;
  } catch (err) {}
  try {
    const proto = Navigator.prototype || Object.getPrototypeOf(navigator);
    if (proto) {
      const getter = markNative(function() { return false; }, 'get webdriver');
      Object.defineProperty(proto, 'webdriver', {
        get: getter,
        configurable: true,
        enumerable: true
      });
    }
  } catch(err) {}

  // 3. Consistent language/locale/hardware profile on prototype.
  try {
    const proto = Navigator.prototype || Object.getPrototypeOf(navigator);
    if (proto) {
      if (!navigator.languages || navigator.languages.length === 0) {
        Object.defineProperty(proto, 'languages', {
          get: markNative(() => ['en-US', 'en'], 'get languages'),
          configurable: true,
          enumerable: true
        });
      }
      if (!navigator.hardwareConcurrency) {
        Object.defineProperty(proto, 'hardwareConcurrency', {
          get: markNative(() => 8, 'get hardwareConcurrency'),
          configurable: true,
          enumerable: true
        });
      }
      if (!navigator.deviceMemory) {
        Object.defineProperty(proto, 'deviceMemory', {
          get: markNative(() => 8, 'get deviceMemory'),
          configurable: true,
          enumerable: true
        });
      }
    }
  } catch(err) {}

  // 4. Plausible plugin list: keep real Chrome PluginArray if valid, otherwise mock with PluginArray prototype.
  try {
    if (!navigator.plugins || navigator.plugins.length === 0 || !(navigator.plugins instanceof PluginArray)) {
      const makeMimeType = (type, suffixes, description) => ({
        type, suffixes, description, enabledPlugin: null
      });
      const plugins = [
        {
          name: 'Chrome PDF Plugin',
          filename: 'internal-pdf-viewer',
          description: 'Portable Document Format',
          version: undefined,
          length: 1,
          item: markNative(function(idx) { return this[idx]; }, 'item'),
          namedItem: markNative(function(name) { return this[name]; }, 'namedItem'),
          0: makeMimeType('application/x-google-chrome-pdf', 'pdf', 'Portable Document Format')
        },
        {
          name: 'Chrome PDF Viewer',
          filename: 'mhjfbmdgcfjbbpaeojofohoefgiehjai',
          description: 'Portable Document Format',
          version: undefined,
          length: 1,
          item: markNative(function(idx) { return this[idx]; }, 'item'),
          namedItem: markNative(function(name) { return this[name]; }, 'namedItem'),
          0: makeMimeType('application/pdf', 'pdf', '')
        },
        {
          name: 'Native Client',
          filename: 'internal-nacl-plugin',
          description: '',
          version: undefined,
          length: 2,
          item: markNative(function(idx) { return this[idx]; }, 'item'),
          namedItem: markNative(function(name) { return this[name]; }, 'namedItem'),
          0: makeMimeType('application/x-nacl', '', 'Native Client Executable'),
          1: makeMimeType('application/x-pnacl', '', 'Portable Native Client Executable')
        }
      ];
      try {
        Object.setPrototypeOf(plugins, PluginArray.prototype);
      } catch (e) {}
      const proto = Navigator.prototype || Object.getPrototypeOf(navigator);
      if (proto) {
        Object.defineProperty(proto, 'plugins', {
          get: markNative(() => plugins, 'get plugins'),
          configurable: true,
          enumerable: true
        });
      }
    }
  } catch(err) {}

  // 5. Chrome-only globals expected by real Chrome profiles.
  if (!window.chrome) {
    window.chrome = {};
  }
  window.chrome.app = {
    isInstalled: false,
    getIsInstalled: markNative(function() { return false; }, 'getIsInstalled'),
    getDetails: markNative(function() { return null; }, 'getDetails'),
    getDetailsForFrame: markNative(function() { return null; }, 'getDetailsForFrame'),
    InstallState: { DISABLED: 'disabled', INSTALLED: 'installed', NOT_INSTALLED: 'not_installed' },
    RunningState: { CANNOT_RUN: 'cannot_run', READY_TO_RUN: 'ready_to_run', RUNNING: 'running' }
  };
  window.chrome.runtime = {
    OnInstalledReason: { CHROME_UPDATE: 'chrome_update', INSTALL: 'install', SHARED_MODULE_UPDATE: 'shared_module_update', UPDATE: 'update' },
    OnRestartRequiredReason: { APP_UPDATE: 'app_update', OS_UPDATE: 'os_update', PERIODIC: 'periodic' },
    PlatformArch: { ARM: 'arm', ARM64: 'arm64', MIPS: 'mips', MIPS64: 'mips64', X86_32: 'x86-32', X86_64: 'x86-64' },
    PlatformNaclArch: { ARM: 'arm', MIPS: 'mips', MIPS64: 'mips64', X86_32: 'x86-32', X86_64: 'x86-64' },
    PlatformOs: { ANDROID: 'android', CROS: 'cros', LINUX: 'linux', MAC: 'mac', OPENBSD: 'openbsd', WIN: 'win' },
    RequestUpdateCheckStatus: { NO_UPDATE: 'no_update', THROTTLED: 'throttled', UPDATE_AVAILABLE: 'update_available' },
    getManifest: markNative(function() { return {}; }, 'getManifest'),
    getURL: markNative(function() { return ''; }, 'getURL')
  };
  window.chrome.csi = markNative(function() { return {}; }, 'csi');
  window.chrome.loadTimes = markNative(function() {
    return {
      requestTime: performance.now() / 1000,
      startLoadTime: performance.now() / 1000,
      commitLoadTime: performance.now() / 1000,
      finishDocumentLoadTime: performance.now() / 1000,
      finishLoadTime: performance.now() / 1000,
      firstPaintTime: performance.now() / 1000,
      firstPaintAfterLoadTime: performance.now() / 1000,
      navigationType: 'NavLink',
      wasFetchedViaSpdy: false,
      wasNpnNegotiated: false,
      npnNegotiatedProtocol: '',
      wasAlternateProtocolAvailable: false,
      connectionInfo: 'h2',
      effectiveConnectionType: '4g'
    };
  }, 'loadTimes');

  // 6. WebGL vendor/renderer spoofing.
  try {
    const getParameter = WebGLRenderingContext.prototype.getParameter;
    WebGLRenderingContext.prototype.getParameter = markNative(function(parameter) {
      if (parameter === 37445) return 'Intel Inc.';
      if (parameter === 37446) return 'Intel Iris OpenGL Engine';
      return getParameter.apply(this, [parameter]);
    }, 'getParameter');
    if (window.WebGL2RenderingContext) {
      const getParameter2 = WebGL2RenderingContext.prototype.getParameter;
      WebGL2RenderingContext.prototype.getParameter = markNative(function(parameter) {
        if (parameter === 37445) return 'Intel Inc.';
        if (parameter === 37446) return 'Intel Iris OpenGL Engine';
        return getParameter2.apply(this, [parameter]);
      }, 'getParameter');
    }
  } catch(err) {}

  // 7. Permissions spoofing so prompts do not expose automation.
  try {
    const originalQuery = window.navigator.permissions.query;
    window.navigator.permissions.query = markNative(function(parameters) {
      if (parameters && parameters.name === 'notifications') {
        return Promise.resolve({ state: Notification.permission, onchange: null });
      }
      return originalQuery.call(this, parameters);
    }, 'query');
    window.navigator.permissions.request = markNative(function() {
      return Promise.resolve({ state: 'prompt' });
    }, 'request');
  } catch(err) {}

  // 8. Hairline feature detection evasion.
  try {
    const elementDescriptor = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'offsetHeight');
    if (elementDescriptor && elementDescriptor.get) {
      Object.defineProperty(HTMLDivElement.prototype, 'offsetHeight', {
        ...elementDescriptor,
        get: markNative(function() {
          if (this.id === 'modernizr') return 1;
          return elementDescriptor.get.apply(this);
        }, 'get offsetHeight')
      });
    }
  } catch(err) {}

  // 9. MediaDevices enumeration mock.
  try {
    if (navigator.mediaDevices && navigator.mediaDevices.enumerateDevices) {
      navigator.mediaDevices.enumerateDevices = markNative(function() {
        return Promise.resolve([
          { kind: 'audioinput', deviceId: 'default', label: 'Microphone (built-in)', groupId: 'default' },
          { kind: 'videoinput', deviceId: 'default', label: 'Camera (built-in)', groupId: 'default' },
          { kind: 'audiooutput', deviceId: 'default', label: 'Speaker (built-in)', groupId: 'default' }
        ]);
      }, 'enumerateDevices');
    }
  } catch(err) {}

  // 10. iframe contentWindow patch so nested automation contexts look natural.
  try {
    const originalCreateElement = Document.prototype.createElement;
    Document.prototype.createElement = markNative(function(tagName, options) {
      const element = originalCreateElement.call(this, tagName, options);
      if (tagName && String(tagName).toLowerCase() === 'iframe') {
        const contentWindowProp = Object.getOwnPropertyDescriptor(HTMLIFrameElement.prototype, 'contentWindow');
        if (contentWindowProp && contentWindowProp.get) {
          Object.defineProperty(element, 'contentWindow', {
            get: markNative(function() {
              const win = contentWindowProp.get.call(this);
              if (win) {
                try {
                  delete win.navigator.webdriver;
                  const winProto = win.Navigator.prototype || Object.getPrototypeOf(win.navigator);
                  if (winProto) {
                    Object.defineProperty(winProto, 'webdriver', {
                      get: markNative(() => false, 'get webdriver'),
                      configurable: true
                    });
                  }
                } catch (e) {}
              }
              return win;
            }, 'get contentWindow')
          });
        }
      }
      return element;
    }, 'createElement');
  } catch(err) {}

  // 11. Suppress console.debug serialization leak probes while preserving native appearance.
  try {
    const origDebug = window.console.debug;
    window.console.debug = markNative(function debug(...args) {
      if (args.length === 1 && typeof args[0] === 'object' && args[0] !== null) {
        try {
          const keys = Object.getOwnPropertyNames(args[0]);
          for (let k of keys) {
            const desc = Object.getOwnPropertyDescriptor(args[0], k);
            if (desc && desc.get) return;
          }
        } catch(e) {}
      }
      return origDebug.apply(window.console, args);
    }, 'debug');
  } catch(err) {}
})();
"#;

/// Injects the UC stealth script into every document created in the current session.
pub async fn apply_uc_stealth(cdp: &CdpClient) -> Result<(), SeleniumBaseError> {
    cdp.add_init_script(UC_STEALTH_SCRIPT).await?;
    Ok(())
}

/// Overrides the browser user agent and optionally the accept-language header via CDP.
pub async fn override_user_agent(
    cdp: &CdpClient,
    user_agent: &str,
    locale: Option<&str>,
) -> Result<(), SeleniumBaseError> {
    let mut params = json!({ "userAgent": user_agent });
    if let Some(locale_value) = locale {
        params["acceptLanguage"] = json!(locale_value);
    }
    cdp.execute("Network.setUserAgentOverride", params).await?;
    Ok(())
}

/// Overrides the browser timezone via CDP (useful for locale-matched UC profiles).
pub async fn override_timezone(
    cdp: &CdpClient,
    timezone_id: &str,
) -> Result<(), SeleniumBaseError> {
    cdp.execute(
        "Emulation.setTimezoneOverride",
        json!({ "timezoneId": timezone_id }),
    )
    .await?;
    Ok(())
}

/// Overrides geolocation via CDP so sites see a fixed latitude/longitude.
pub async fn override_geolocation(
    cdp: &CdpClient,
    latitude: f64,
    longitude: f64,
    accuracy: f64,
) -> Result<(), SeleniumBaseError> {
    cdp.execute(
        "Emulation.setGeolocationOverride",
        json!({ "latitude": latitude, "longitude": longitude, "accuracy": accuracy }),
    )
    .await?;
    Ok(())
}

/// The script [`clear_cdc_properties`] runs: it deletes the `cdc_…` properties
/// ChromeDriver leaves on `window` and its prototypes.
///
/// It is a function expression so that running it again, or in a page that
/// already ran it, cannot fail on a re-declared variable.
pub const CLEAR_CDC_SCRIPT: &str = "(() => { let o = window; while (o) { Object.getOwnPropertyNames(o).filter(p => /^[a-z]{3}_[a-zA-Z0-9]{22}_.*/i.test(p)).forEach(p => { try { delete o[p]; } catch(e) {} }); o = Object.getPrototypeOf(o); } })()";

/// Masks the WebDriver `cdc_` property in the active page as an extra defense.
///
/// # Errors
///
/// Returns an error if the browser refuses the command or the script throws.
pub async fn clear_cdc_properties(cdp: &CdpClient) -> Result<(), SeleniumBaseError> {
    // Inside a function: a top-level `let` stays declared in the page, so the
    // second call would throw "Identifier 'o' has already been declared".
    let response = cdp
        .execute(
            "Runtime.evaluate",
            json!({ "expression": CLEAR_CDC_SCRIPT, "returnByValue": true }),
        )
        .await?;
    if let Some(details) = response.get("exceptionDetails") {
        return Err(SeleniumBaseError::Unsupported(format!(
            "clearing the cdc_ properties threw: {details}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uc_stealth_script_contains_all_eleven_sections() {
        for section_num in 1..=11 {
            let marker = format!("// {section_num}.");
            assert!(
                UC_STEALTH_SCRIPT.contains(&marker),
                "UC_STEALTH_SCRIPT must contain section {section_num} marker '{marker}'"
            );
        }
    }

    #[test]
    fn uc_stealth_script_masks_native_functions() {
        assert!(
            UC_STEALTH_SCRIPT.contains("markNative"),
            "UC_STEALTH_SCRIPT must define and use markNative helper"
        );
        assert!(
            UC_STEALTH_SCRIPT.contains("function () { [native code] }")
                || UC_STEALTH_SCRIPT.contains("[native code]"),
            "markNative must provide native code representation"
        );
    }

    #[test]
    fn uc_stealth_script_webdriver_is_prototype_false_not_own_property() {
        // Must delete any own property on navigator
        assert!(
            UC_STEALTH_SCRIPT.contains("delete navigator.webdriver"),
            "Must delete any existing own property on navigator"
        );
        // Must define getter on Navigator.prototype
        assert!(
            UC_STEALTH_SCRIPT.contains("Navigator.prototype"),
            "Must define webdriver getter on Navigator.prototype"
        );
        assert!(
            UC_STEALTH_SCRIPT.contains("return false"),
            "webdriver getter must return false"
        );
        // Must NOT define returning undefined (which leaked on BrowserScan)
        assert!(
            !UC_STEALTH_SCRIPT.contains("() => undefined"),
            "Must not return undefined for webdriver"
        );
        assert!(
            !UC_STEALTH_SCRIPT.contains("navigator.webdriver = "),
            "Must not set own property navigator.webdriver directly"
        );
    }

    #[test]
    fn uc_stealth_script_plugin_array_prototype_fidelity() {
        assert!(
            UC_STEALTH_SCRIPT.contains("PluginArray.prototype"),
            "Must reference PluginArray prototype"
        );
        assert!(
            UC_STEALTH_SCRIPT.contains("plugins instanceof PluginArray"),
            "Must preserve or ensure PluginArray instance"
        );
    }

    #[test]
    fn uc_stealth_script_protects_against_cdp_console_inspection_probe() {
        assert!(
            UC_STEALTH_SCRIPT.contains("console.debug"),
            "Must wrap console.debug for CDP inspection probe protection"
        );
    }

    #[test]
    fn clear_cdc_script_structure() {
        assert!(
            CLEAR_CDC_SCRIPT.contains("[a-z]{3}_[a-zA-Z0-9]{22}_"),
            "CLEAR_CDC_SCRIPT must target chromedriver marker pattern"
        );
        assert!(
            CLEAR_CDC_SCRIPT.contains("getPrototypeOf"),
            "CLEAR_CDC_SCRIPT must traverse prototype chain"
        );
    }
}
