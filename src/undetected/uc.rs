use serde_json::json;

use crate::browser::devtools::CdpClient;
use crate::error::SeleniumBaseError;

/// Minimal undetected-chrome (UC) script injected into every new document.
///
/// Removes `cdc_` chromedriver markers from `window` and `document` prototypes
/// and sets `navigator.webdriver` to `false` on `Navigator.prototype`.
pub const UC_STEALTH_SCRIPT: &str = r#"
(function() {
  'use strict';

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

  // 2. Only override navigator.webdriver when not already false (preserves native getter
  //    when --disable-blink-features=AutomationControlled is active).
  try {
    if (navigator.webdriver !== false) {
      try {
        delete navigator.webdriver;
      } catch (err) {}
      const proto = Navigator.prototype || Object.getPrototypeOf(navigator);
      if (proto) {
        Object.defineProperty(proto, 'webdriver', {
          get: () => false,
          configurable: true,
          enumerable: true
        });
      }
    }
  } catch(err) {}
})();
"#;

/// JavaScript snippet executed via CDP `Runtime.evaluate` to remove `cdc_`
/// properties on an already-loaded document.
pub const CLEAR_CDC_SCRIPT: &str = r#"
(function() {
  const cdcRegex = /^[a-z]{3}_[a-zA-Z0-9]{22}_.*/i;
  for (const prop of Object.getOwnPropertyNames(window)) {
    if (cdcRegex.test(prop) || /^\$?cdc_/i.test(prop)) {
      try { delete window[prop]; } catch (e) {}
    }
  }
  if (typeof document !== 'undefined') {
    for (const prop of Object.getOwnPropertyNames(document)) {
      if (cdcRegex.test(prop) || /^\$?cdc_/i.test(prop)) {
        try { delete document[prop]; } catch (e) {}
      }
    }
  }
})();
"#;

/// Injects [`UC_STEALTH_SCRIPT`] on every new document using `Page.addScriptToEvaluateOnNewDocument`.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::CdpDriver`] if the CDP command fails.
pub async fn apply_uc_stealth(cdp: &CdpClient) -> Result<(), SeleniumBaseError> {
    cdp.execute(
        "Page.addScriptToEvaluateOnNewDocument",
        json!({ "source": UC_STEALTH_SCRIPT }),
    )
    .await?;
    Ok(())
}

/// Immediately clears `cdc_` properties on the current document.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::CdpDriver`] if `Runtime.evaluate` fails.
pub async fn clear_cdc_properties(cdp: &CdpClient) -> Result<(), SeleniumBaseError> {
    cdp.execute(
        "Runtime.evaluate",
        json!({
            "expression": CLEAR_CDC_SCRIPT,
            "returnByValue": false,
        }),
    )
    .await?;
    Ok(())
}

/// Overrides the `User-Agent` header and `navigator.userAgent` via CDP `Network.setUserAgentOverride`.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::CdpDriver`] if the CDP command fails.
pub async fn override_user_agent(
    cdp: &CdpClient,
    user_agent: &str,
    platform: Option<&str>,
    accept_language: Option<&str>,
) -> Result<(), SeleniumBaseError> {
    let mut params = json!({ "userAgent": user_agent });
    if let Some(p) = platform {
        params["platform"] = json!(p);
    }
    if let Some(lang) = accept_language {
        params["acceptLanguage"] = json!(lang);
    }
    cdp.execute("Network.setUserAgentOverride", params).await?;
    Ok(())
}

/// Overrides the browser timezone via CDP `Emulation.setTimezoneOverride`.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::CdpDriver`] if the CDP command fails.
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

/// Overrides the browser geolocation via CDP `Emulation.setGeolocationOverride`.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::CdpDriver`] if the CDP command fails.
pub async fn override_geolocation(
    cdp: &CdpClient,
    latitude: f64,
    longitude: f64,
    accuracy: Option<f64>,
) -> Result<(), SeleniumBaseError> {
    cdp.execute(
        "Emulation.setGeolocationOverride",
        json!({
            "latitude": latitude,
            "longitude": longitude,
            "accuracy": accuracy.unwrap_or(100.0),
        }),
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uc_stealth_script_is_nonempty_and_well_formed() {
        assert!(UC_STEALTH_SCRIPT.contains("cdcRegex"));
        assert!(UC_STEALTH_SCRIPT.contains("Navigator.prototype"));
        assert!(UC_STEALTH_SCRIPT.contains("'webdriver'"));
    }

    #[test]
    fn clear_cdc_script_targets_window_and_document() {
        assert!(CLEAR_CDC_SCRIPT.contains("window"));
        assert!(CLEAR_CDC_SCRIPT.contains("document"));
        assert!(CLEAR_CDC_SCRIPT.contains("cdc_"));
    }
}
