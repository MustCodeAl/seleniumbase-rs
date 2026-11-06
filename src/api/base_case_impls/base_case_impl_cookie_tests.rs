// Tests of `get_cookie`, `get_cookies` and `add_cookie` against a mock
// WebDriver (no browser). Included (for tests only) at the end of
// `base_case.rs` like the files beside it.

mod tests_cookie_calls {
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

    #[tokio::test]
    async fn a_cookie_that_is_not_there_is_not_an_error() {
        let sb = page_with_sid().base_case().await;
        let found = sb
            .get_cookie("missing")
            .await
            .expect("a missing cookie is not an error");
        assert_eq!(format!("{found:?}"), "None");
    }
}
