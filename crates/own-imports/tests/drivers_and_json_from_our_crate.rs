//! The WebDriver-side types and the JSON values, imported from `seleniumbase_rs` alone.
//!
//! The manifest of this crate lists neither `thirtyfour` nor `serde_json`. The assertions
//! also show that the types are the ones the API itself uses: the raw element behind an
//! `Element` is the `WebElement` of `seleniumbase_rs::drivers`, and `json!` of
//! `seleniumbase_rs::json` reads a W3C cookie object as the `Cookie` that `BaseCase::add_cookie`
//! takes.

#![expect(
    deprecated,
    reason = "NetworkConditions is the type BaseCase::set_network_conditions takes"
)]

use seleniumbase_rs::drivers::{
    ChromiumLikeCapabilities, ConnectionType, DesiredCapabilities, NetworkConditions,
    WebDriverError, WebElement,
};
use seleniumbase_rs::json::{from_value, json, Value};
use seleniumbase_rs::{
    BaseCase, Cookie, DriverErrorKind, Element, Key, SameSite, SelectBy, SeleniumBaseError,
};

#[test]
fn an_element_and_the_types_around_it_come_from_our_crate() {
    // Naming the function is enough: it compiles without `thirtyfour` in the manifest.
    let _what_a_test_writes = what_a_test_writes_with_an_element;
}

/// What a test of the WebDriver engine writes with an element: `Element`, `Key` and
/// `SelectBy` are at the crate root, the raw element is `drivers::WebElement`, and a
/// selector is a plain string.
async fn what_a_test_writes_with_an_element(element: &Element) -> seleniumbase_rs::Result<()> {
    element.find(".remove").await?.click().await?;
    element.press(Key::Enter).await?;
    element.select_option(SelectBy::Text("Two")).await?;
    let raw: &WebElement = element.as_webdriver_element();
    println!("{} is {}", element, raw.element_id());
    Ok(())
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
fn a_webdriver_error_becomes_the_error_type_and_its_kind_is_read_without_thirtyfour() {
    let error = SeleniumBaseError::from(WebDriverError::FatalError("gone".to_owned()));
    assert!(error.to_string().contains("gone"));
    // The kind is read from our own enum: no `thirtyfour` item is named.
    assert!(
        matches!(&error, SeleniumBaseError::WebDriver(driver) if driver.kind() == DriverErrorKind::Other),
        "{error:?}"
    );
}

#[test]
fn the_json_macro_makes_the_value_the_api_trades_in() {
    let object: Value = json!({ "name": "session", "value": "abc123" });
    assert_eq!(object["name"], "session");
    assert_eq!(object["value"], json!("abc123"));
}

#[test]
fn a_w3c_cookie_object_reads_as_the_cookie_the_api_takes() {
    let cookie: Cookie = from_value(json!({
        "name": "session", "value": "abc123", "expiry": 4_102_444_800_i64, "sameSite": "Lax"
    }))
    .expect("a W3C cookie object");
    assert_eq!(cookie.path, "/");
    assert_eq!(cookie.expires, Some(4_102_444_800.0));
    assert_eq!(cookie.same_site, Some(SameSite::Lax));
}

/// What a test writes with a cookie: the type is at the crate root, and the same one goes in and
/// comes out. Naming the function is enough: it compiles without `thirtyfour` in the manifest.
async fn what_a_test_writes_with_a_cookie(sb: &BaseCase) -> seleniumbase_rs::Result<()> {
    sb.add_cookie(&Cookie::new("session", "abc123").http_only(true))
        .expiry(true)
        .await?;
    if let Some(cookie) = sb.get_cookie("session").await? {
        println!("{cookie:?}");
    }
    for cookie in sb.get_cookies().await? {
        println!("{} expires {:?}", cookie.name, cookie.expires);
    }
    Ok(())
}

#[test]
fn the_cookie_calls_come_from_our_crate() {
    let _what_a_test_writes = what_a_test_writes_with_a_cookie;
}
