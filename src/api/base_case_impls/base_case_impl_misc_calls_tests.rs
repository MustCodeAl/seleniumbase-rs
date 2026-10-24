// Tests of the helpers that take Python's keyword arguments as call builders
// (`crate::api::misc_calls`), against the mock WebDriver in `webdriver_support`
// (no browser needed). Included (for tests only) at the end of `base_case.rs`
// like the files beside it.

mod tests_misc_calls {
    use super::webdriver_support::{element, MockWebDriver, Reply, Seen};
    use super::*;
    use serde_json::json;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    /// A page whose elements `a`, `b` and `c` match every selector; `b` is not
    /// displayed. A window 1000 x 800 big; every script answers "UA-test".
    fn page() -> MockWebDriver {
        MockWebDriver::start(json!({}), |request| {
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/element") => Reply::Value(element("a")),
                ("POST", "/elements") => {
                    Reply::Value(json!([element("a"), element("b"), element("c")]))
                }
                ("GET", "/element/b/displayed") => Reply::Value(json!(false)),
                ("GET", "/element/a/displayed" | "/element/c/displayed") => {
                    Reply::Value(json!(true))
                }
                ("GET", "/element/a/enabled") => Reply::Value(json!(true)),
                ("GET", "/window/rect") => {
                    Reply::Value(json!({ "x": 0, "y": 0, "width": 1000, "height": 800 }))
                }
                ("GET", "/window") => Reply::Value(json!("h1")),
                ("GET", "/window/handles") => Reply::Value(json!(["h1", "h2"])),
                ("POST", "/execute/sync") => Reply::Value(json!("UA-test")),
                _ => Reply::Value(Value::Null),
            }
        })
    }

    fn scripts(driver: &MockWebDriver) -> Vec<String> {
        driver
            .requests("/execute/sync")
            .iter()
            .filter_map(|request| request.body["script"].as_str().map(str::to_owned))
            .collect()
    }

    /// How many POST requests the driver got for `path`.
    fn posts(driver: &MockWebDriver, path: &str) -> usize {
        driver
            .requests(path)
            .iter()
            .filter(|request| request.method == "POST")
            .count()
    }

    #[test]
    fn an_offset_is_rounded_up_from_the_corner_and_cut_from_the_centre() {
        assert_eq!(whole_pixels(10.0, false), 10);
        assert_eq!(whole_pixels(10.2, false), 11);
        assert_eq!(whole_pixels(-0.5, false), 0);
        assert_eq!(whole_pixels(10.9, true), 10);
        assert_eq!(whole_pixels(-10.9, true), -10);
    }


    fn without_driver() -> BaseCase {
        BaseCase::without_session(BrowserConfig::default())
    }

    #[tokio::test]
    async fn click_nth_visible_element_counts_from_one_over_the_visible_ones() {
        let driver = page();
        let mut sb = driver.base_case().await;

        // `b` is hidden, so the second visible match is `c`.
        sb.click_nth_visible_element("p", 2).await.unwrap();

        assert_eq!(driver.requests("/element/c/click").len(), 1);
        assert!(driver.requests("/element/a/click").is_empty());
    }

    #[tokio::test]
    async fn click_nth_visible_element_refuses_zero_and_too_big_a_number() {
        let mut sb = without_driver();
        let error = sb.click_nth_visible_element("p", 0).await.unwrap_err();
        let SeleniumBaseError::InvalidConfig(message) = error else {
            panic!("expected InvalidConfig, got {error:?}");
        };
        assert!(message.contains("counts from 1"), "{message}");

        let driver = page();
        let mut sb = driver.base_case().await;
        let error = sb.click_nth_visible_element("p", 3).await.unwrap_err();
        let SeleniumBaseError::AssertionFailed(message) = error else {
            panic!("expected AssertionFailed, got {error:?}");
        };
        assert!(message.contains("click number 3"), "{message}");
        assert!(driver.requests("/element/c/click").is_empty());
    }

    #[tokio::test]
    async fn an_arrow_key_goes_to_body_as_often_as_times_says() {
        let driver = page();
        let sb = driver.base_case().await;

        sb.press_up_arrow().times(3).await.unwrap();

        let keys = driver.requests("/element/a/value");
        assert_eq!(keys.len(), 3);
        for request in keys {
            let sent = request.body["text"].as_str().unwrap_or_default();
            assert!(sent.contains('\u{e013}'), "{:?}", request.body);
        }
        let found: Vec<_> = driver
            .requests("/element")
            .iter()
            .map(|request: &Seen| request.body["value"].clone())
            .collect();
        assert!(found.contains(&json!("body")), "{found:?}");
    }

    #[tokio::test]
    async fn an_arrow_key_pressed_zero_times_sends_nothing() {
        let driver = page();
        let sb = driver.base_case().await;

        sb.press_left_arrow().times(0).await.unwrap();
        sb.press_down_arrow().selector("#x").await.unwrap();

        assert!(driver.requests("/element/a/value").len() == 1);
    }

    #[tokio::test]
    async fn a_scroll_is_a_percentage_of_the_window_height() {
        let driver = page();
        let sb = driver.base_case().await;

        sb.scroll_down().await.unwrap();
        sb.scroll_up().amount(50.0).await.unwrap();

        let scrolls: Vec<_> = scripts(&driver)
            .into_iter()
            .filter(|script| script.contains("scrollBy"))
            .collect();
        assert_eq!(scrolls, ["window.scrollBy(0, 200);", "window.scrollBy(0, -400);"]);
    }

    #[tokio::test]
    async fn a_scroll_by_nothing_a_number_can_be_is_refused() {
        let sb = without_driver();
        let error = sb.scroll_down().amount(f64::NAN).await.unwrap_err();
        assert!(matches!(error, SeleniumBaseError::InvalidConfig(_)), "{error:?}");
    }

    #[tokio::test]
    async fn hovering_then_double_clicking_takes_two_selectors() {
        let driver = page();
        let mut sb = driver.base_case().await;

        sb.hover_and_double_click("#menu", "#item")
            .timeout(Duration::from_secs(3))
            .await
            .unwrap();

        // One chain of actions for the hover, one for the double click.
        assert_eq!(posts(&driver, "/actions"), 2);
        let finds: Vec<_> = driver
            .requests("/element")
            .iter()
            .map(|request| request.body["value"].clone())
            .collect();
        assert!(finds.contains(&json!("#menu")), "{finds:?}");
        assert!(finds.contains(&json!("#item")), "{finds:?}");
    }

    #[tokio::test]
    async fn hovering_then_js_clicking_clicks_with_a_script() {
        let driver = page();
        let mut sb = driver.base_case().await;

        sb.hover_and_js_click("#menu", "#item").await.unwrap();

        assert_eq!(posts(&driver, "/actions"), 1);
        assert!(scripts(&driver).iter().any(|script| script.contains("click")));
    }

    #[tokio::test]
    async fn waiting_for_an_attribute_value_asks_the_page_for_that_value() {
        let driver = page();
        let mut sb = driver.base_case().await;

        // Every script answers a string, which is not `true`: no match.
        let waited = sb
            .wait_for_attribute("#app", "data-ready")
            .value("yes")
            .timeout(Duration::ZERO)
            .await;
        assert!(matches!(waited, Err(SeleniumBaseError::WaitTimeout(_))), "{waited:?}");
        let asked = scripts(&driver).join("\n");
        assert!(asked.contains("\"yes\"") && asked.contains("getAttribute"), "{asked}");

        let waited = sb
            .wait_for_attribute_not_present("#app", "data-ready")
            .value("yes")
            .timeout(Duration::ZERO)
            .await;
        assert!(waited.is_ok(), "{waited:?}");
    }

    #[tokio::test]
    async fn a_new_tab_is_switched_to_unless_switch_to_says_no() {
        let driver = page();
        let mut sb = driver.base_case().await;

        sb.open_new_tab().switch_to(false).await.unwrap();
        let switches = |driver: &MockWebDriver| {
            driver
                .requests("/window")
                .iter()
                .filter(|request| request.method == "POST")
                .count()
        };
        assert_eq!(switches(&driver), 0);
        assert!(scripts(&driver).iter().any(|script| script.contains("window.open")));

        sb.open_new_window().await.unwrap();
        assert_eq!(switches(&driver), 1);
    }

    #[tokio::test]
    async fn a_cookie_object_goes_in_without_its_expiry_unless_asked() {
        let driver = page();
        let sb = driver.base_case().await;
        let cookie = json!({ "name": "k", "value": "v", "path": "/", "expiry": 4_102_444_800_i64 });

        sb.add_cookie(&cookie).await.unwrap();
        sb.add_cookie(&cookie).expiry(true).await.unwrap();

        let sent: Vec<_> = driver
            .requests("/cookie")
            .iter()
            .map(|request| request.body["cookie"].clone())
            .collect();
        assert_eq!(sent.len(), 2);
        assert_eq!(sent[0]["name"], "k");
        assert_eq!(sent[0]["path"], "/");
        assert!(sent[0].get("expiry").is_none(), "{:?}", sent[0]);
        // A day from now, not the year 2100 the object came with.
        let day = sent[1]["expiry"].as_i64().unwrap();
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
        assert!((day - 86_400 - now).abs() < 120, "{day}");
    }

    #[tokio::test]
    async fn a_cookie_without_a_name_and_a_value_is_refused() {
        let driver = page();
        let sb = driver.base_case().await;

        for cookie in [json!({ "name": "k" }), json!({ "value": "v" }), json!("k=v")] {
            let error = sb.add_cookie(&cookie).await.unwrap_err();
            assert!(matches!(error, SeleniumBaseError::InvalidConfig(_)), "{error:?}");
        }
        assert!(driver.requests("/cookie").is_empty());
    }

    /// A server that answers `/report.csv` with a small file and anything else
    /// with 404, and keeps the head of each request it got.
    fn serve_files() -> (String, Arc<Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let heads = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&heads);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut buffer = [0_u8; 4096];
                let read = stream.read(&mut buffer).unwrap_or(0);
                let head = String::from_utf8_lossy(&buffer[..read]).into_owned();
                let found = head.starts_with("GET /report.csv");
                log.lock().unwrap().push(head);
                let (status, body) = if found { ("200 OK", "a,b\n1,2\n") } else { ("404 Not Found", "no") };
                let _ = stream.write_all(
                    format!(
                        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .as_bytes(),
                );
            }
        });
        (base, heads)
    }

    #[tokio::test]
    async fn download_file_and_save_file_as_fetch_the_url_into_a_folder() {
        let driver = page();
        let sb = driver.base_case().await;
        let (base, heads) = serve_files();
        let folder = tempfile::tempdir().unwrap();
        let url = format!("{base}/report.csv?token=1");

        let saved = sb
            .download_file(&url)
            .destination_folder(folder.path().join("made"))
            .await
            .unwrap();
        assert_eq!(saved, folder.path().join("made").join("report.csv"));
        assert_eq!(std::fs::read_to_string(&saved).unwrap(), "a,b\n1,2\n");

        let renamed = sb
            .save_file_as(&url, "renamed.txt")
            .destination_folder(folder.path())
            .await
            .unwrap();
        assert_eq!(renamed, folder.path().join("renamed.txt"));
        assert_eq!(std::fs::read_to_string(&renamed).unwrap(), "a,b\n1,2\n");

        // The browser's user agent goes with the request.
        let heads = heads.lock().unwrap();
        assert_eq!(heads.len(), 2);
        assert!(heads.iter().all(|head| head.contains("UA-test")), "{heads:?}");
    }

    #[tokio::test]
    async fn a_download_that_cannot_be_saved_says_why() {
        let driver = page();
        let sb = driver.base_case().await;
        let (base, _) = serve_files();
        let folder = tempfile::tempdir().unwrap();

        let missing = sb
            .download_file(&format!("{base}/missing.csv"))
            .destination_folder(folder.path())
            .await
            .unwrap_err();
        assert!(matches!(missing, SeleniumBaseError::Network { .. }), "{missing:?}");
        assert!(!folder.path().join("missing.csv").exists());

        let escaping = sb
            .save_file_as(&format!("{base}/report.csv"), "../outside.csv")
            .destination_folder(folder.path())
            .await;
        assert!(escaping.is_err());
        assert!(!folder.path().parent().unwrap().join("outside.csv").exists());
    }

    #[test]
    fn the_google_auth_password_is_the_mfa_code() {
        let sb = without_driver();
        // The code changes every 30 seconds; try again if a step fell between.
        let same = (0..3).any(|_| {
            sb.get_google_auth_password("JBSWY3DPEHPK3PXP").unwrap()
                == sb.get_mfa_code("JBSWY3DPEHPK3PXP").unwrap()
        });
        assert!(same);
    }
}
