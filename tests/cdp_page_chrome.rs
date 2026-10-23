//! The `BaseCase::cdp_*` helpers (`api::cdp_page::CdpPage`) against a real
//! Chrome driven through chromedriver.
//!
//! The scripts these helpers send are checked here, where a JavaScript engine
//! runs them: a click on an element below the fold, the errors for what is not
//! there, and the dropdown selection. The test needs Chrome and chromedriver,
//! so it is ignored by default. Run it with:
//!
//! ```text
//! cargo test --test cdp_page_chrome -- --ignored
//! ```

use seleniumbase_rs::{BaseCase, BrowserConfig, DriverMode, SeleniumBaseError};

/// The `%` and `#` characters a data URL cannot hold are left out of it.
const PAGE: &str = "data:text/html,<html><head><title>CDP page</title></head>\
    <body style=\"height:3000px\">\
    <select id=planet><option value=earth>Earth</option><option value=mars>Mars</option></select>\
    <a id=link href=\"/x\">Link</a>\
    <div id=out>waiting</div>\
    <button id=low style=\"margin-top:2000px\" \
    onclick=\"document.getElementById('out').textContent='clicked'\">Low</button>\
    </body></html>";

#[tokio::test]
#[ignore = "requires a local Chrome and chromedriver"]
async fn the_cdp_helpers_click_read_select_and_report_what_is_missing() {
    let mut config = BrowserConfig::default()
        .with_headless(true)
        .with_mode(DriverMode::Cdp);
    config.auto_start_driver = true;
    let mut sb = BaseCase::new(config).await.expect("Chrome starts");
    sb.open(PAGE).await.unwrap();

    // The button is far below the fold: the click scrolls to it first.
    sb.cdp_click("#low").await.unwrap();
    assert_eq!(sb.cdp_get_text("#out").await.unwrap(), "clicked");

    // Reads.
    assert_eq!(
        sb.cdp_get_attribute("#planet", "id").await.unwrap(),
        Some("planet".to_owned())
    );
    assert_eq!(
        sb.cdp_get_attribute("#link", "download").await.unwrap(),
        None
    );
    assert!(matches!(
        sb.cdp_get_text("#nope").await,
        Err(SeleniumBaseError::ElementNotFound { .. })
    ));
    assert!(matches!(
        sb.cdp_get_attribute("#nope", "id").await,
        Err(SeleniumBaseError::ElementNotFound { .. })
    ));
    assert_eq!(
        sb.cdp_get_property("#planet", "value").await.unwrap(),
        Some("earth".to_owned())
    );

    // Dropdowns: a value that is there, one that is not, and no dropdown.
    sb.cdp_select_option_by_value("#planet", "mars")
        .await
        .unwrap();
    assert_eq!(
        sb.cdp_get_property("#planet", "value").await.unwrap(),
        Some("mars".to_owned())
    );
    assert!(matches!(
        sb.cdp_select_option_by_value("#planet", "venus").await,
        Err(SeleniumBaseError::InvalidSelector(_))
    ));
    assert_eq!(
        sb.cdp_get_property("#planet", "value").await.unwrap(),
        Some("mars".to_owned()),
        "a value the dropdown lacks must not change it"
    );
    assert!(matches!(
        sb.cdp_select_option_by_value("#nodropdown", "earth").await,
        Err(SeleniumBaseError::ElementNotFound { .. })
    ));
    sb.cdp_select_option_by_text("#planet", "Earth")
        .await
        .unwrap();
    sb.cdp_select_option_by_index("#planet", 1).await.unwrap();
    assert!(matches!(
        sb.cdp_select_option_by_index("#planet", 9).await,
        Err(SeleniumBaseError::InvalidSelector(_))
    ));

    // Scripts: a value comes back, a final `return` is dropped, a throw is an error.
    assert_eq!(sb.cdp_evaluate("1 + 1").await.unwrap(), 2);
    assert_eq!(
        sb.cdp_evaluate("const a = 20;\nreturn a + 1")
            .await
            .unwrap(),
        21
    );
    assert!(matches!(
        sb.cdp_evaluate("throw new Error('boom')").await,
        Err(SeleniumBaseError::CdpDriver(message)) if message.contains("boom")
    ));

    sb.quit().await.unwrap();
}

#[tokio::test]
#[ignore = "requires a local Chrome and chromedriver"]
async fn cdp_click_hits_the_button_on_a_page_scrolled_down() {
    let mut config = BrowserConfig::default()
        .with_headless(true)
        .with_mode(DriverMode::Cdp);
    config.auto_start_driver = true;
    let mut sb = BaseCase::new(config).await.expect("Chrome starts");
    sb.open(PAGE).await.unwrap();

    // The button is 2000 px down. With the page scrolled to it, its place in
    // the document (WebDriver's rectangle) is 1900 px below its place in the
    // viewport, which is where the click must go.
    sb.execute_script("window.scrollTo(0, 1900);")
        .await
        .unwrap();
    sb.cdp_click("#low").await.unwrap();
    assert_eq!(sb.cdp_get_text("#out").await.unwrap(), "clicked");

    sb.quit().await.unwrap();
}
