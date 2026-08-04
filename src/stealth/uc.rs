use serde_json::json;

use crate::error::SeleniumBaseError;
use crate::stealth::cdp::CdpClient;

/// Comprehensive undetected-chrome (UC) stealth script injected into every new document.
///
/// The script removes `cdc_` markers, rewrites `navigator.webdriver`, mocks Chrome-only
/// objects, patches WebGL/permissions/media devices, and masks common automation
/// fingerprints. It is designed to run via `Page.addScriptToEvaluateOnNewDocument` so
/// it executes before any page scripts.
const UC_STEALTH_SCRIPT: &str = r#"
(function() {
  'use strict';

  // 1. Remove known chromedriver markers from the window object.
  let objectToInspect = window;
  let cdc_props = [];
  while (objectToInspect !== null) {
    cdc_props = cdc_props.concat(Object.getOwnPropertyNames(objectToInspect));
    objectToInspect = Object.getPrototypeOf(objectToInspect);
  }
  cdc_props.filter(i => /^[a-z]{3}_[a-zA-Z0-9]{22}_.*/i.test(i)).forEach(p => {
    try { delete window[p]; } catch (e) {}
  });

  // 2. navigator.webdriver must be undefined/false, not a getter that returns undefined.
  try {
    Object.defineProperty(navigator, 'webdriver', {
      get: () => undefined,
      configurable: true,
      enumerable: true
    });
    // Defensive: some detectors check the descriptor itself.
    const proto = Navigator.prototype || Object.getPrototypeOf(navigator);
    if (proto) {
      Object.defineProperty(proto, 'webdriver', {
        get: () => undefined,
        configurable: true,
        enumerable: true
      });
    }
  } catch(err) {}

  // 3. Consistent language/locale/hardware profile.
  Object.defineProperty(navigator, 'languages', { get: () => ['en-US', 'en'] });
  Object.defineProperty(navigator, 'hardwareConcurrency', { get: () => 8 });
  Object.defineProperty(navigator, 'deviceMemory', { get: () => 8 });

  // 4. Plausible plugin list so automation fingerprinting does not see an empty array.
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
      item: function(idx) { return this[idx]; },
      namedItem: function(name) { return this[name]; },
      0: makeMimeType('application/x-google-chrome-pdf', 'pdf', 'Portable Document Format')
    },
    {
      name: 'Chrome PDF Viewer',
      filename: 'mhjfbmdgcfjbbpaeojofohoefgiehjai',
      description: 'Portable Document Format',
      version: undefined,
      length: 1,
      item: function(idx) { return this[idx]; },
      namedItem: function(name) { return this[name]; },
      0: makeMimeType('application/pdf', 'pdf', '')
    },
    {
      name: 'Native Client',
      filename: 'internal-nacl-plugin',
      description: '',
      version: undefined,
      length: 2,
      item: function(idx) { return this[idx]; },
      namedItem: function(name) { return this[name]; },
      0: makeMimeType('application/x-nacl', '', 'Native Client Executable'),
      1: makeMimeType('application/x-pnacl', '', 'Portable Native Client Executable')
    }
  ];
  Object.defineProperty(navigator, 'plugins', { get: () => plugins });

  // 5. Chrome-only globals expected by real Chrome profiles.
  if (!window.chrome) {
    window.chrome = {};
  }
  window.chrome.app = {
    isInstalled: false,
    getIsInstalled: function() { return false; },
    getDetails: function() { return null; },
    getDetailsForFrame: function() { return null; },
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
    getManifest: function() { return {}; },
    getURL: function() { return ''; }
  };
  window.chrome.csi = function() { return {}; };
  window.chrome.loadTimes = function() {
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
  };

  // 6. WebGL vendor/renderer spoofing.
  try {
    const getParameter = WebGLRenderingContext.prototype.getParameter;
    WebGLRenderingContext.prototype.getParameter = function(parameter) {
      if (parameter === 37445) return 'Intel Inc.';
      if (parameter === 37446) return 'Intel Iris OpenGL Engine';
      return getParameter.apply(this, [parameter]);
    };
    if (window.WebGL2RenderingContext) {
      const getParameter2 = WebGL2RenderingContext.prototype.getParameter;
      WebGL2RenderingContext.prototype.getParameter = function(parameter) {
        if (parameter === 37445) return 'Intel Inc.';
        if (parameter === 37446) return 'Intel Iris OpenGL Engine';
        return getParameter2.apply(this, [parameter]);
      };
    }
  } catch(err) {}

  // 7. Permissions spoofing so prompts do not expose automation.
  try {
    const originalQuery = window.navigator.permissions.query;
    window.navigator.permissions.query = function(parameters) {
      if (parameters && parameters.name === 'notifications') {
        return Promise.resolve({ state: Notification.permission, onchange: null });
      }
      return originalQuery.call(this, parameters);
    };
    window.navigator.permissions.request = function() {
      return Promise.resolve({ state: 'prompt' });
    };
  } catch(err) {}

  // 8. Hairline feature detection evasion.
  try {
    const elementDescriptor = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'offsetHeight');
    if (elementDescriptor && elementDescriptor.get) {
      Object.defineProperty(HTMLDivElement.prototype, 'offsetHeight', {
        ...elementDescriptor,
        get: function() {
          if (this.id === 'modernizr') return 1;
          return elementDescriptor.get.apply(this);
        }
      });
    }
  } catch(err) {}

  // 9. MediaDevices enumeration mock.
  try {
    Object.defineProperty(navigator.mediaDevices, 'enumerateDevices', {
      value: () => Promise.resolve([
        { kind: 'audioinput', deviceId: 'default', label: 'Microphone (built-in)', groupId: 'default' },
        { kind: 'videoinput', deviceId: 'default', label: 'Camera (built-in)', groupId: 'default' },
        { kind: 'audiooutput', deviceId: 'default', label: 'Speaker (built-in)', groupId: 'default' }
      ])
    });
  } catch(err) {}

  // 10. iframe contentWindow patch so nested automation contexts look natural.
  try {
    const originalCreateElement = Document.prototype.createElement;
    Document.prototype.createElement = function(tagName, options) {
      const element = originalCreateElement.call(this, tagName, options);
      if (tagName.toLowerCase() === 'iframe') {
        const contentWindowProp = Object.getOwnPropertyDescriptor(HTMLIFrameElement.prototype, 'contentWindow');
        if (contentWindowProp && contentWindowProp.get) {
          Object.defineProperty(element, 'contentWindow', {
            get: function() {
              const win = contentWindowProp.get.call(this);
              if (win) {
                try {
                  Object.defineProperty(win.navigator, 'webdriver', { get: () => undefined });
                } catch (e) {}
              }
              return win;
            }
          });
        }
      }
      return element;
    };
  } catch(err) {}

  // 11. Suppress console.debug leak vectors used by some detectors.
  try {
    window.console.debug = function() {};
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
    cdp.execute_with_params("Network.setUserAgentOverride", params)
        .await?;
    Ok(())
}

/// Overrides the browser timezone via CDP (useful for locale-matched UC profiles).
pub async fn override_timezone(
    cdp: &CdpClient,
    timezone_id: &str,
) -> Result<(), SeleniumBaseError> {
    cdp.execute_with_params(
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
    cdp.execute_with_params(
        "Emulation.setGeolocationOverride",
        json!({ "latitude": latitude, "longitude": longitude, "accuracy": accuracy }),
    )
    .await?;
    Ok(())
}

/// Masks the WebDriver `cdc_` property in the active page as an extra defense.
pub async fn clear_cdc_properties(cdp: &CdpClient) -> Result<(), SeleniumBaseError> {
    cdp.execute_with_params(
        "Runtime.evaluate",
        json!({
            "expression": "let o = window; while (o) { Object.getOwnPropertyNames(o).filter(p => /^[a-z]{3}_[a-zA-Z0-9]{22}_.*/i.test(p)).forEach(p => { try { delete o[p]; } catch(e) {} }); o = Object.getPrototypeOf(o); }",
            "returnByValue": true
        }),
    )
    .await?;
    Ok(())
}
