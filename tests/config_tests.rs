use seleniumbase_rs::config::{Browser, BrowserConfig, DriverMode};
use seleniumbase_rs::core::selectors::Selector;

#[test]
fn browser_config_defaults_are_safe() {
    let config = BrowserConfig::default();
    assert_eq!(config.webdriver_url, "http://localhost:4444");
    assert_eq!(config.browser, Browser::Chrome);
    assert!(config.headless);
    assert_eq!(config.mode, DriverMode::WebDriver);
    assert!(!config.is_cdp_enabled());
    assert!(!config.is_uc_enabled());
}

#[test]
fn selector_validation_rejects_empty_values() {
    let result = Selector::Css(" ").to_by();
    assert!(result.is_err());
}

#[test]
fn uc_mode_implies_cdp() {
    let config = BrowserConfig::default().with_mode(DriverMode::Uc);
    assert!(config.is_cdp_enabled());
    assert!(config.is_uc_enabled());
}
