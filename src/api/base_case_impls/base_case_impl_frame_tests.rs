// Tests of `set_content_to_frame`, `set_content_to_default` and
// `set_content_to_parent` against a small fake browser behind the mock
// WebDriver in `webdriver_support`: windows with a URL, a source and the
// counters the page keeps, one iframe, and frame entry and exit. The test that a
// real Chrome swaps the page is `tests/set_content_to_frame_chrome.rs`.
// Included (for tests only) at the end of `base_case.rs` like the files beside it.

mod tests_frame_swap {
    use super::webdriver_support::{element, MockWebDriver, Reply, Seen};
    use super::*;
    use serde_json::{json, Value};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    const TOP_URL: &str = "https://site.test/page";
    const TOP_SOURCE: &str = "<html><body><h1>Top</h1><iframe id=\"f\"></iframe></body></html>";
    const FRAME_SOURCE: &str = "<html><head><style>p{color:red}</style></head>\
        <body><p>Frame \"quoted\" & <b>bold</b></p></body></html>";
    const SECOND_FRAME_SOURCE: &str = "<html><body><p>The frame inside the frame</p></body></html>";
    const FRAME_TAB_URL: &str = "https://frame.test/inner";

    /// Text that breaks a script which pastes it between quotes.
    const HOSTILE: &[&str] = &[
        "<p title='x'>it's</p>",
        "x'); alert(1); ('",
        "say \"hi\" and C:\\dir\\new",
        "line one\nline two\r\n\ttabbed",
        "separators \u{2028} and \u{2029} inside",
        "</script><!-- &amp; -->",
        "",
    ];

    /// One window of the fake browser.
    struct FakeWindow {
        url: String,
        /// What the page was when it loaded, which is what a refresh brings back.
        loaded: String,
        source: String,
        /// `document.cframe_swap` and `document.cframe_tab` in this window's page.
        swaps: u32,
        tabs: u32,
    }

    impl FakeWindow {
        fn at(url: &str, source: &str) -> Self {
            Self {
                url: url.to_owned(),
                loaded: source.to_owned(),
                source: source.to_owned(),
                swaps: 0,
                tabs: 0,
            }
        }
    }

    /// A browser with one window on [`TOP_URL`], whose page holds one iframe
    /// that `iframe#f` and `iframe[name="named"]` select.
    struct Fake {
        current: String,
        order: Vec<String>,
        windows: HashMap<String, FakeWindow>,
        in_frame: bool,
        frame_source: String,
        /// What the frame element's `src` property says.
        frame_src: String,
        refreshes: u32,
    }

    impl Fake {
        fn new(frame_src: &str) -> Self {
            Self {
                current: "tab-1".to_owned(),
                order: vec!["tab-1".to_owned()],
                windows: HashMap::from([("tab-1".to_owned(), FakeWindow::at(TOP_URL, TOP_SOURCE))]),
                in_frame: false,
                frame_source: FRAME_SOURCE.to_owned(),
                frame_src: frame_src.to_owned(),
                refreshes: 0,
            }
        }

        fn window(&mut self) -> &mut FakeWindow {
            self.windows.get_mut(&self.current).expect("the current window")
        }

        fn answer(&mut self, request: &Seen) -> Reply {
            let text = |value: &Value| value.as_str().unwrap_or_default().to_owned();
            match (request.method.as_str(), request.path.as_str()) {
                ("GET", "/url") => Reply::Value(json!(self.window().url)),
                ("GET", "/window") => Reply::Value(json!(self.current)),
                ("GET", "/window/handles") => Reply::Value(json!(self.order)),
                ("GET", "/source") if self.in_frame => Reply::Value(json!(self.frame_source)),
                ("GET", "/source") => Reply::Value(json!(self.window().source)),
                ("POST", path @ ("/element" | "/elements")) => {
                    let selector = text(&request.body["value"]);
                    let found = selector == "iframe#f" || selector == "iframe[name=\"named\"]";
                    match (path, found) {
                        ("/element", true) => Reply::Value(element("e1")),
                        ("/element", false) => Reply::Error(404, "no such element"),
                        (_, true) => Reply::Value(json!([element("e1")])),
                        (_, false) => Reply::Value(json!([])),
                    }
                }
                ("GET", "/element/e1/property/src") => Reply::Value(json!(self.frame_src)),
                ("GET", "/element/e1/displayed") => Reply::Value(json!(true)),

                ("POST", "/frame") => {
                    self.in_frame = !request.body["id"].is_null();
                    Reply::Value(Value::Null)
                }
                ("POST", "/frame/parent") => {
                    self.in_frame = false;
                    Reply::Value(Value::Null)
                }
                ("POST", "/window") => {
                    self.current = text(&request.body["handle"]);
                    Reply::Value(Value::Null)
                }
                ("POST", "/url") => {
                    let url = text(&request.body["url"]);
                    let page = format!("<html><body>{url}</body></html>");
                    *self.window() = FakeWindow::at(&url, &page);
                    Reply::Value(Value::Null)
                }
                ("POST", "/refresh") => {
                    self.refreshes += 1;
                    let window = self.window();
                    window.source = window.loaded.clone();
                    (window.swaps, window.tabs) = (0, 0);
                    Reply::Value(Value::Null)
                }
                ("POST", "/execute/sync") => self.run(&text(&request.body["script"])),
                _ => Reply::Value(Value::Null),
            }
        }

        fn run(&mut self, script: &str) -> Reply {
            if script == "return document.readyState;" {
                return Reply::Value(json!("complete"));
            }
            if script == FRAME_SWAP_STATE_SCRIPT {
                let window = self.window();
                return Reply::Value(json!([window.swaps, window.tabs]));
            }
            if script == FRAME_SWAP_UP_SCRIPT {
                self.window().swaps += 1;
            } else if script == FRAME_SWAP_DOWN_SCRIPT {
                let window = self.window();
                window.swaps = window.swaps.saturating_sub(1);
            } else if script == FRAME_SWAP_RESET_SCRIPT {
                self.window().swaps = 0;
            } else if script == FRAME_TAB_SCRIPT {
                self.window().tabs = 1;
            } else if script == "window.open('');" {
                let handle = format!("tab-{}", self.order.len() + 1);
                self.windows
                    .insert(handle.clone(), FakeWindow::at("about:blank", ""));
                self.order.push(handle);
            } else if let Ok(read) = ReadScript::read(script) {
                // The only other script is the one that replaces the document.
                if read.skeleton.starts_with("var next = new DOMParser()") {
                    self.window().source = read.literals[0].clone();
                }
            }
            Reply::Value(Value::Null)
        }
    }

    /// A driver backed by a [`Fake`] whose iframe has `frame_src` as its `src`,
    /// and the fake itself, so a test can look at the browser and change it.
    fn start(frame_src: &str) -> (MockWebDriver, Arc<Mutex<Fake>>) {
        let fake = Arc::new(Mutex::new(Fake::new(frame_src)));
        let shared = Arc::clone(&fake);
        let driver = MockWebDriver::start(json!({}), move |request| {
            shared.lock().expect("the fake browser").answer(request)
        });
        (driver, fake)
    }

    /// Every script the driver was asked to run, except the readiness polls.
    fn scripts(driver: &MockWebDriver) -> Vec<String> {
        driver
            .requests("/execute/sync")
            .iter()
            .filter_map(|request| request.body["script"].as_str())
            .filter(|script| *script != "return document.readyState;")
            .map(str::to_owned)
            .collect()
    }

    /// The source of the current window's page.
    fn page_source(fake: &Mutex<Fake>) -> String {
        fake.lock().expect("the fake browser").window().source.clone()
    }

    /// Where the frame was entered (`false`) and left (`true`), in order.
    fn frame_moves(driver: &MockWebDriver) -> Vec<bool> {
        driver
            .requests("/frame")
            .iter()
            .map(|request| request.body["id"].is_null())
            .collect()
    }

    #[test]
    fn the_replacing_script_keeps_the_html_inside_its_string() {
        for html in HOSTILE.iter().chain([&FRAME_SOURCE]) {
            let script = replace_document_script(html);
            let read = ReadScript::read(&script).unwrap_or_else(|why| panic!("{why}: {script:?}"));
            assert_eq!(
                read.skeleton.split_whitespace().collect::<Vec<_>>(),
                "var next = new DOMParser().parseFromString(S, S); \
                 document.head.innerHTML = next.head.innerHTML; \
                 document.body.innerHTML = next.body.innerHTML;"
                    .split_whitespace()
                    .collect::<Vec<_>>(),
                "script: {script:?}"
            );
            assert_eq!(read.literals, [*html, "text/html"], "script: {script:?}");
        }
    }

    #[tokio::test]
    async fn a_page_swapped_for_a_frame_comes_back_with_set_content_to_default() {
        let (driver, fake) = start("");
        let mut sb = driver.base_case().await;

        sb.set_content_to_frame("iframe#f").await.unwrap();

        assert_eq!(
            frame_moves(&driver),
            [false, true],
            "the frame is entered to read its source and left again"
        );
        assert_eq!(
            scripts(&driver),
            [
                replace_document_script(FRAME_SOURCE),
                FRAME_SWAP_UP_SCRIPT.to_owned()
            ]
        );
        assert_eq!(page_source(&fake), FRAME_SOURCE);

        sb.set_content_to_default().await.unwrap();

        assert_eq!(
            scripts(&driver)[2..],
            [
                FRAME_SWAP_STATE_SCRIPT.to_owned(),
                replace_document_script(TOP_SOURCE),
                FRAME_SWAP_RESET_SCRIPT.to_owned()
            ]
        );
        assert_eq!(page_source(&fake), TOP_SOURCE);
        assert_eq!(fake.lock().unwrap().refreshes, 0);
        assert_eq!(frame_moves(&driver), [false, true], "no further frame moves");
    }

    #[tokio::test]
    async fn a_frame_can_be_named_instead_of_selected() {
        let (driver, fake) = start("");
        let mut sb = driver.base_case().await;

        sb.set_content_to_frame("named").await.unwrap();

        let looked_up: Vec<String> = driver
            .requests("/elements")
            .iter()
            .map(|request| request.body["value"].as_str().unwrap_or_default().to_owned())
            .collect();
        assert_eq!(looked_up[..2], ["named", "iframe[name=\"named\"]"]);
        assert_eq!(page_source(&fake), FRAME_SOURCE);
    }

    #[tokio::test]
    async fn a_frame_that_is_not_there_times_out_and_leaves_the_page_alone() {
        let (driver, fake) = start("");
        let mut sb = driver.base_case().await;
        sb.set_timeout(1).await.unwrap();

        let error = sb
            .set_content_to_frame("iframe#missing")
            .await
            .expect_err("there is no such frame");

        assert!(
            matches!(&error, SeleniumBaseError::WaitTimeout(message) if message.contains("iframe#missing")),
            "{error:?}"
        );
        assert_eq!(scripts(&driver), Vec::<String>::new());
        assert_eq!(frame_moves(&driver), Vec::<bool>::new());
        assert_eq!(page_source(&fake), TOP_SOURCE);
    }

    #[tokio::test]
    async fn a_frame_with_an_http_src_opens_in_a_new_tab_and_default_returns_to_the_first() {
        let (driver, fake) = start(FRAME_TAB_URL);
        let mut sb = driver.base_case().await;

        sb.set_content_to_frame("iframe#f").await.unwrap();

        {
            let mut fake = fake.lock().unwrap();
            assert_eq!(fake.current, "tab-2");
            assert_eq!(fake.window().url, FRAME_TAB_URL);
            assert_eq!((fake.windows["tab-1"].tabs, fake.window().tabs), (1, 1));
        }
        assert_eq!(
            scripts(&driver),
            [
                FRAME_TAB_SCRIPT.to_owned(),
                "window.open('');".to_owned(),
                FRAME_TAB_SCRIPT.to_owned()
            ]
        );

        sb.set_content_to_default().await.unwrap();

        let mut fake = fake.lock().unwrap();
        assert_eq!(fake.current, "tab-1");
        assert_eq!(fake.order, ["tab-1", "tab-2"], "the new tab is left open, as in Python");
        assert_eq!(fake.window().source, TOP_SOURCE);
        assert_eq!(fake.refreshes, 0);
        drop(fake);
        assert_eq!(
            scripts(&driver)[3..],
            [
                FRAME_SWAP_STATE_SCRIPT.to_owned(),
                replace_document_script(TOP_SOURCE),
                FRAME_SWAP_RESET_SCRIPT.to_owned()
            ]
        );
    }

    #[tokio::test]
    async fn with_nothing_swapped_default_and_parent_only_leave_the_frame() {
        let (driver, fake) = start("");
        let mut sb = driver.base_case().await;

        sb.switch_to_frame("iframe#f").await.unwrap();
        assert!(fake.lock().unwrap().in_frame);
        sb.set_content_to_default().await.unwrap();
        assert!(!fake.lock().unwrap().in_frame);

        sb.switch_to_frame("iframe#f").await.unwrap();
        sb.set_content_to_parent().await.unwrap();

        assert!(!fake.lock().unwrap().in_frame);
        assert_eq!(driver.requests("/frame/parent").len(), 1);
        assert_eq!(fake.lock().unwrap().refreshes, 0);
        assert_eq!(scripts(&driver), Vec::<String>::new());
    }

    #[tokio::test]
    async fn a_swap_the_page_forgot_is_answered_with_a_refresh_once() {
        let (driver, fake) = start("");
        let mut sb = driver.base_case().await;
        sb.set_content_to_frame("iframe#f").await.unwrap();
        // The test navigates: the new document has no `cframe_swap`.
        fake.lock().unwrap().window().swaps = 0;

        sb.set_content_to_default().await.unwrap();

        assert_eq!(fake.lock().unwrap().refreshes, 1);
        assert_eq!(page_source(&fake), TOP_SOURCE, "a refresh brings the loaded page back");
        let before = scripts(&driver).len();

        sb.set_content_to_default().await.unwrap();

        assert_eq!(fake.lock().unwrap().refreshes, 1, "the forgotten swap was dropped");
        assert_eq!(scripts(&driver).len(), before, "and nothing was run for it");
    }

    #[tokio::test]
    async fn parent_undoes_one_swap_at_a_time_and_default_undoes_them_all() {
        let (driver, fake) = start("");
        let mut sb = driver.base_case().await;
        sb.set_content_to_frame("iframe#f").await.unwrap();
        fake.lock().unwrap().frame_source = SECOND_FRAME_SOURCE.to_owned();
        sb.set_content_to_frame("iframe#f").await.unwrap();
        assert_eq!(page_source(&fake), SECOND_FRAME_SOURCE);
        assert_eq!(fake.lock().unwrap().window().swaps, 2);
        let before = scripts(&driver).len();

        sb.set_content_to_parent().await.unwrap();

        assert_eq!(page_source(&fake), FRAME_SOURCE, "back to the page before the second swap");
        assert_eq!(fake.lock().unwrap().window().swaps, 1);
        assert_eq!(
            scripts(&driver)[before..],
            [
                FRAME_SWAP_STATE_SCRIPT.to_owned(),
                FRAME_SWAP_DOWN_SCRIPT.to_owned(),
                replace_document_script(FRAME_SOURCE)
            ]
        );

        sb.set_content_to_parent().await.unwrap();

        assert_eq!(page_source(&fake), TOP_SOURCE, "back to the page before the first");
        assert_eq!(fake.lock().unwrap().window().swaps, 0);

        // Swap twice again, then leave them all at once.
        sb.set_content_to_frame("iframe#f").await.unwrap();
        sb.set_content_to_frame("iframe#f").await.unwrap();
        sb.set_content_to_default().await.unwrap();
        assert_eq!(page_source(&fake), TOP_SOURCE);
        assert_eq!(fake.lock().unwrap().window().swaps, 0);
        assert_eq!(fake.lock().unwrap().refreshes, 0);
    }
}
