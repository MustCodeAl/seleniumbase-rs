//! The WebDriver-side types and the JSON values, imported from `seleniumbase_rs` alone.
//!
//! The manifest of this crate lists neither `thirtyfour` nor `serde_json`. The assertions
//! also show that the types are the ones the API itself uses: a `By` made by the library's
//! own `Selector` is the `By` of `seleniumbase_rs::drivers`, and `json!` of
//! `seleniumbase_rs::json` makes a `Value` that `BaseCase::add_cookie` takes.

#![expect(
    deprecated,
    reason = "NetworkConditions is the type BaseCase::set_network_conditions takes"
)]

use seleniumbase_rs::drivers::{
    By, ChromiumLikeCapabilities, ConnectionType, DesiredCapabilities, NetworkConditions,
    WebDriverError,
};
use seleniumbase_rs::json::{json, Value};
use seleniumbase_rs::{Selector, SeleniumBaseError};

#[test]
fn a_selector_becomes_the_by_that_the_drivers_module_names() {
    let by: By = Selector::auto("#login").to_by().expect("a valid selector");
    assert_eq!(format!("{by:?}"), format!("{:?}", By::Css("#login")));
}

#[test]
fn capabilities_of_a_custom_launch_are_built_from_the_drivers_module() {
    let mut caps = DesiredCapabilities::chrome();
    caps.add_arg("--headless=new").expect("a Chrome argument");
    assert!(format!("{caps:?}").contains("--headless=new"));
}

#[test]
fn network_conditions_are_the_type_the_api_takes() {
    let slow = NetworkConditions {
        offline: false,
        latency: 400,
        download_throughput: 200_000,
        upload_throughput: 94_000,
        connection_type: Some(ConnectionType::Cellular3G),
    };
    assert_eq!(slow.latency, 400);
}

#[test]
fn a_webdriver_error_is_the_one_the_error_type_wraps() {
    let error = SeleniumBaseError::WebDriver(WebDriverError::FatalError("gone".to_owned()));
    assert!(error.to_string().contains("gone"));
}

#[test]
fn the_json_macro_makes_the_value_the_api_takes() {
    let cookie: Value = json!({ "name": "session", "value": "abc123" });
    assert_eq!(cookie["name"], "session");
    assert_eq!(cookie["value"], json!("abc123"));
}
