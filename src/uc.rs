use serde_json::json;

use crate::cdp::CdpClient;
use crate::error::SeleniumBaseError;

// A robust stealth patch set for UC mode.
const UC_STEALTH_SCRIPT: &str = r#"
// 1. Remove cdc_ properties if they exist
let objectToInspect = window;
let cdc_props = [];
while (objectToInspect !== null) {
  cdc_props = cdc_props.concat(Object.getOwnPropertyNames(objectToInspect));
  objectToInspect = Object.getPrototypeOf(objectToInspect);
}
cdc_props.filter(i => i.match(/^[a-z]{3}_[a-z]{22}_.*/i)).forEach(p => delete window[p]);

// 2. Mock navigator properties
Object.defineProperty(navigator, 'webdriver', { get: () => undefined });
Object.defineProperty(navigator, 'languages', { get: () => ['en-US', 'en'] });
Object.defineProperty(navigator, 'plugins', {
  get: () => [
    {
      0: {type: "application/x-google-chrome-pdf", suffixes: "pdf", description: "Portable Document Format"},
      description: "Portable Document Format",
      filename: "internal-pdf-viewer",
      length: 1,
      name: "Chrome PDF Plugin"
    },
    {
      0: {type: "application/pdf", suffixes: "pdf", description: ""},
      description: "",
      filename: "mhjfbmdgcfjbbpaeojofohoefgiehjai",
      length: 1,
      name: "Chrome PDF Viewer"
    },
    {
      0: {type: "application/x-nacl", suffixes: "", description: "Native Client Executable"},
      1: {type: "application/x-pnacl", suffixes: "", description: "Portable Native Client Executable"},
      description: "",
      filename: "internal-nacl-plugin",
      length: 2,
      name: "Native Client"
    }
  ]
});

// Mock hardware properties
Object.defineProperty(navigator, 'hardwareConcurrency', { get: () => 8 });
Object.defineProperty(navigator, 'deviceMemory', { get: () => 8 });

// 3. Mock Chrome app and runtime objects
if (!window.chrome) {
  window.chrome = {};
}
window.chrome.app = {
  isInstalled: false,
  InstallState: { DISABLED: 'disabled', INSTALLED: 'installed', NOT_INSTALLED: 'not_installed' },
  RunningState: { CANNOT_RUN: 'cannot_run', READY_TO_RUN: 'ready_to_run', RUNNING: 'running' }
};
window.chrome.runtime = {
  OnInstalledReason: { CHROME_UPDATE: 'chrome_update', INSTALL: 'install', SHARED_MODULE_UPDATE: 'shared_module_update', UPDATE: 'update' },
  OnRestartRequiredReason: { APP_UPDATE: 'app_update', OS_UPDATE: 'os_update', PERIODIC: 'periodic' },
  PlatformArch: { ARM: 'arm', ARM64: 'arm64', MIPS: 'mips', MIPS64: 'mips64', X86_32: 'x86-32', X86_64: 'x86-64' },
  PlatformNaclArch: { ARM: 'arm', MIPS: 'mips', MIPS64: 'mips64', X86_32: 'x86-32', X86_64: 'x86-64' },
  PlatformOs: { ANDROID: 'android', CROS: 'cros', LINUX: 'linux', MAC: 'mac', OPENBSD: 'openbsd', WIN: 'win' },
  RequestUpdateCheckStatus: { NO_UPDATE: 'no_update', THROTTLED: 'throttled', UPDATE_AVAILABLE: 'update_available' }
};

// 4. WebGL Vendor spoofing
try {
  const getParameter = WebGLRenderingContext.prototype.getParameter;
  WebGLRenderingContext.prototype.getParameter = function(parameter) {
    if (parameter === 37445) { // UNMASKED_VENDOR_WEBGL
      return 'Intel Inc.';
    }
    if (parameter === 37446) { // UNMASKED_RENDERER_WEBGL
      return 'Intel Iris OpenGL Engine';
    }
    return getParameter.apply(this, [parameter]);
  };
} catch(err) {}

// 5. Permissions spoofing
try {
  const originalQuery = window.navigator.permissions.query;
  window.navigator.permissions.query = (parameters) => (
    parameters.name === 'notifications' ?
      Promise.resolve({ state: Notification.permission }) :
      originalQuery(parameters)
  );
} catch(err) {}

// 6. Hairline Feature
try {
  const elementDescriptor = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'offsetHeight');
  Object.defineProperty(HTMLDivElement.prototype, 'offsetHeight', {
    ...elementDescriptor,
    get: function() {
      if (this.id === 'modernizr') {
          return 1;
      }
      return elementDescriptor.get.apply(this);
    },
  });
} catch(err) {}

// 7. MediaDevices mock
try {
  Object.defineProperty(navigator.mediaDevices, 'enumerateDevices', {
    value: () => Promise.resolve([
      { kind: 'audioinput', deviceId: 'default', label: 'Microphone (built-in)', groupId: 'a' },
      { kind: 'videoinput', deviceId: 'default', label: 'Camera (built-in)', groupId: 'b' },
      { kind: 'audiooutput', deviceId: 'default', label: 'Speaker (built-in)', groupId: 'c' }
    ])
  });
} catch(err) {}
"#;

pub async fn apply_uc_stealth(cdp: &CdpClient) -> Result<(), SeleniumBaseError> {
    cdp.add_init_script(UC_STEALTH_SCRIPT).await?;
    Ok(())
}

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
