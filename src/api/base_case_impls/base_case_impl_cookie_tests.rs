// Tests of `get_cookie`, `get_cookies` and `add_cookie` against a mock
// WebDriver (no browser). Included (for tests only) at the end of
// `base_case.rs` like the files beside it.

mod tests_cookie_calls {
    use crate::browser::config::DriverMode;
    use crate::cookie::SameSite;
    use super::webdriver_support::{MockWebDriver, Reply};
    use super::*;
    use serde_json::json;

    /// A page whose only cookie is `sid`; the driver says "no such cookie"
    /// for any other name.
    fn page_with_sid() -> MockWebDriver {
        MockWebDriver::start(json!({}), |request| {
            match (request.method.as_str(), request.path.as_str()) {
                ("GET", "/cookie/sid") => Reply::Value(json!({
                    "name": "sid", "value": "abc", "domain": "www.example.test", "path": "/",
                    "secure": true, "httpOnly": true, "expiry": 4_102_444_800_i64,
                    "sameSite": "Lax"
                })),
                ("GET", "/cookie/missing") => Reply::Error(404, "no such cookie"),
                ("GET", "/cookie/broken") => Reply::Error(500, "unknown error"),
                _ => Reply::Value(Value::Null),
            }
        })
    }

    /// The cookie `sid` as the tests below expect to read it.
    fn sid() -> Cookie {
        Cookie::new("sid", "abc")
            .domain("www.example.test")
            .secure(true)
            .http_only(true)
            .expires(4_102_444_800.0)
            .same_site(SameSite::Lax)
    }

    /// The session cookie `theme`.
    fn theme() -> Cookie {
        Cookie::new("theme", "dark").domain("www.example.test")
    }

    /// The two cookies as WebDriver reports them.
    fn webdriver_cookies() -> Value {
        json!([
            {
                "name": "sid", "value": "abc", "domain": "www.example.test", "path": "/",
                "secure": true, "httpOnly": true, "expiry": 4_102_444_800_i64,
                "sameSite": "Lax"
            },
            {
                "name": "theme", "value": "dark", "domain": "www.example.test", "path": "/",
                "secure": false, "httpOnly": false
            }
        ])
    }

    /// The same two cookies as the DevTools Protocol reports them: a float `expires`, `-1`
    /// for a session cookie, and fields WebDriver has no name for.
    fn devtools_cookies() -> Value {
        json!([
            {
                "name": "sid", "value": "abc", "domain": "www.example.test", "path": "/",
                "expires": 4_102_444_800.0, "size": 6, "httpOnly": true, "secure": true,
                "session": false, "sameSite": "Lax", "priority": "Medium"
            },
            {
                "name": "theme", "value": "dark", "domain": "www.example.test", "path": "/",
                "expires": -1, "size": 9, "httpOnly": false, "secure": false,
                "session": true, "priority": "Medium"
            }
        ])
    }

    /// A page at `https://www.example.test/home` that holds the two cookies, whichever way it
    /// is asked for them.
    fn page_with_cookies() -> MockWebDriver {
        MockWebDriver::start(json!({}), |request| {
            match (request.method.as_str(), request.path.as_str()) {
                ("GET", "/cookie") => Reply::Value(webdriver_cookies()),
                ("GET", "/url") => Reply::Value(json!("https://www.example.test/home")),
                // `document.readyState`, which loading cookies waits for.
                ("POST", "/execute/sync") => Reply::Value(json!("complete")),
                ("POST", "/goog/cdp/execute") if request.body["cmd"] == "Network.getCookies" => {
                    Reply::Value(json!({ "cookies": devtools_cookies() }))
                }
                ("POST", "/goog/cdp/execute") => Reply::Value(json!({})),
                _ => Reply::Value(Value::Null),
            }
        })
    }

    /// A [`BaseCase`] in DevTools mode on `driver`.
    async fn devtools_case(driver: &MockWebDriver) -> BaseCase {
        let mut config = BrowserConfig::default()
            .with_webdriver_url(driver.url().to_owned())
            .with_mode(DriverMode::Cdp);
        config.auto_start_driver = false;
        BaseCase::new(config)
            .await
            .expect("the mock accepts a session")
    }

    /// The cookies the driver was asked to add, oldest first.
    fn added(driver: &MockWebDriver) -> Vec<Value> {
        driver
            .requests("/cookie")
            .into_iter()
            .filter(|request| request.method == "POST")
            .map(|request| request.body["cookie"].clone())
            .collect()
    }

    #[tokio::test]
    async fn a_cookie_that_is_not_there_is_not_an_error() {
        let sb = page_with_sid().base_case().await;
        let found = sb
            .get_cookie("missing")
            .await
            .expect("a missing cookie is not an error");
        assert!(found.is_none(), "{found:?}");
    }

    #[tokio::test]
    async fn any_other_driver_error_is_still_an_error() {
        let sb = page_with_sid().base_case().await;
        let error = sb.get_cookie("broken").await.unwrap_err();
        assert!(matches!(error, SeleniumBaseError::WebDriver(_)), "{error:?}");
    }

    #[tokio::test]
    async fn a_cookie_that_is_there_comes_back_field_by_field() {
        let sb = page_with_sid().base_case().await;
        assert_eq!(sb.get_cookie("sid").await.unwrap(), Some(sid()));
    }

    #[tokio::test]
    async fn the_cookies_of_a_plain_webdriver_session_are_ours() {
        let driver = page_with_cookies();
        let sb = driver.base_case().await;
        assert_eq!(sb.get_cookies().await.unwrap(), [sid(), theme()]);
        assert!(
            driver.requests("/goog/cdp/execute").is_empty(),
            "no DevTools in this mode"
        );
    }

    #[tokio::test]
    async fn the_cookies_of_a_devtools_session_are_ours_too() {
        let driver = page_with_cookies();
        let sb = devtools_case(&driver).await;
        assert_eq!(sb.get_cookies().await.unwrap(), [sid(), theme()]);
        assert!(
            driver.requests("/cookie").is_empty(),
            "not asked through WebDriver"
        );
        assert!(
            driver
                .requests("/goog/cdp/execute")
                .iter()
                .any(|request| request.body["cmd"] == "Network.getCookies"),
            "asked through DevTools"
        );
    }

    #[tokio::test]
    async fn both_paths_give_equal_cookies_for_the_same_cookies() {
        let over_webdriver = page_with_cookies()
            .base_case()
            .await
            .get_cookies()
            .await
            .unwrap();
        let over_devtools = devtools_case(&page_with_cookies())
            .await
            .get_cookies()
            .await
            .unwrap();
        assert_eq!(over_webdriver, over_devtools);
        // A session cookie has no expiry on both: DevTools says `-1`, WebDriver says nothing.
        assert_eq!(over_webdriver[1].expires, None);
        assert_eq!(over_devtools[1].expires, None);
    }

    #[tokio::test]
    async fn cookies_that_were_read_go_back_in_as_they_are() {
        let driver = page_with_cookies();
        let sb = driver.base_case().await;
        for cookie in sb.get_cookies().await.unwrap() {
            sb.add_cookie(&cookie)
                .expiry(CookieExpiry::Saved)
                .await
                .unwrap();
        }
        let expected = webdriver_cookies().as_array().unwrap().clone();
        assert_eq!(added(&driver), expected);
    }

    #[tokio::test]
    async fn a_file_saved_by_python_loads() {
        let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/cookies/python_saved_cookies.txt");
        let driver = page_with_cookies();
        let sb = driver.base_case().await;

        sb.load_cookies_from_path(&file, CookieExpiry::Saved)
            .await
            .unwrap();

        let sent = added(&driver);
        assert_eq!(sent.len(), 3, "{sent:?}");
        // The cookie of another domain takes the page's; the rest is as Python saved it.
        assert_eq!(sent[0]["name"], "sid");
        assert_eq!(sent[0]["domain"], "www.example.test");
        assert_eq!(sent[0]["httpOnly"], true);
        assert_eq!(sent[0]["expiry"], 4_102_444_800_i64);
        assert_eq!(sent[0]["sameSite"], "Lax");
        // No expiry and no policy in the file, none sent; the flags are as saved.
        assert_eq!(sent[1]["name"], "theme");
        assert_eq!(sent[1]["secure"], true);
        assert!(sent[1].get("expiry").is_none(), "{:?}", sent[1]);
        assert!(sent[1].get("sameSite").is_none(), "{:?}", sent[1]);
        assert_eq!(sent[2]["path"], "/docs");
        assert_eq!(sent[2]["expiry"], 1_893_456_000_i64);
        assert_eq!(sent[2]["sameSite"], "None");
    }

    #[tokio::test]
    async fn the_cookies_are_saved_in_the_format_python_reads() {
        // The file holds the driver's own cookie objects, which Python's `load_cookies`
        // takes as they are; it also reads back as our cookies.
        let driver = page_with_cookies();
        let sb = driver.base_case().await;
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("cookies.txt");

        sb.save_cookies_to_path(&file).await.unwrap();

        let text = std::fs::read_to_string(&file).unwrap();
        let written: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(written, webdriver_cookies(), "{text}");
        let saved: Vec<Cookie> = serde_json::from_str(&text).unwrap();
        assert_eq!(saved, [sid(), theme()]);
    }

    
}
