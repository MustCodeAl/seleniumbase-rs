// Tests of `generate_referral`, `generate_traffic` and their chains against the
// mock WebDriver in `webdriver_support` (no browser needed): what the driver was
// asked, and in which order. The test that a real Chrome sends a `Referer`
// header is `tests/generate_referral_chrome.rs`.
// Included (for tests only) at the end of `base_case.rs` like the files beside it.

mod tests_traffic {
    use super::webdriver_support::{element, MockWebDriver, Reply};
    use super::*;
    use serde_json::{json, Value};

    /// The script `generate_referral` sends, with every string literal as `S`.
    const LINK_SKELETON: &str = "var link = document.createElement(S); link.className = S; \
        link.setAttribute(S, S); link.setAttribute(S, S); link.textContent = S; \
        var body = document.createElement(S); body.appendChild(link); \
        document.documentElement.replaceChild(body, document.body);";

    /// The style Python gives the link.
    const LINK_STYLE: &str = "font-family: Arial,sans-serif; font-size: 30px; color: #18a2cd";

    /// Text a URL or a page could hold, each of which breaks a script that
    /// pastes it between quotes.
    const HOSTILE: &[&str] = &[
        "http://x.test/a'b",
        "http://x.test/\"; window.pwned = 1; //",
        "http://x.test/</script><!-- &amp; -->",
        "line one\nline two\r\n\ttabbed",
        "separators \u{2028} and \u{2029} inside",
        "",
    ];

    /// One thing the driver was asked that matters to these tests.
    #[derive(Debug, PartialEq)]
    enum Step {
        /// A navigation.
        Open(String),
        /// A look at `document.readyState`.
        WaitForLoad,
        /// The script that builds the referral link, with the URL it links to.
        Link(String),
        /// A lookup of an element by selector.
        Find(String),
        /// A click on the element that was found.
        Click,
    }

    /// A driver whose page is loaded and which finds, shows, enables and
    /// clicks one element for any selector.
    fn driver() -> MockWebDriver {
        MockWebDriver::start(json!({}), |request| {
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/execute/sync") => Reply::Value(json!("complete")),
                ("POST", "/element") => Reply::Value(element("e1")),
                ("POST", "/elements") => Reply::Value(json!([element("e1")])),
                ("GET", "/element/e1/displayed" | "/element/e1/enabled") => {
                    Reply::Value(json!(true))
                }
                _ => Reply::Value(Value::Null),
            }
        })
    }

    /// What the driver was asked, in order, with a step that repeats the one
    /// before it (a poll that looked twice) counted once.
    fn steps(driver: &MockWebDriver) -> Vec<Step> {
        let mut steps = Vec::new();
        for request in driver.all_requests() {
            let text = |value: &Value| value.as_str().unwrap_or_default().to_owned();
            let step = match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/url") => Step::Open(text(&request.body["url"])),
                ("POST", "/element") => Step::Find(text(&request.body["value"])),
                ("POST", "/element/e1/click") => Step::Click,
                ("POST", "/execute/sync") => {
                    let script = text(&request.body["script"]);
                    if script == "return document.readyState;" {
                        Step::WaitForLoad
                    } else {
                        let read = ReadScript::read(&script)
                            .unwrap_or_else(|why| panic!("{why}: {script:?}"));
                        assert_eq!(
                            read.skeleton.split_whitespace().collect::<Vec<_>>(),
                            LINK_SKELETON.split_whitespace().collect::<Vec<_>>(),
                            "a script that is not the referral link: {script:?}"
                        );
                        Step::Link(read.literals[3].clone())
                    }
                }
                _ => continue,
            };
            if steps.last() != Some(&step) {
                steps.push(step);
            }
        }
        steps
    }

    fn open(url: &str) -> Step {
        Step::Open(url.to_owned())
    }

    fn link(url: &str) -> Step {
        Step::Link(url.to_owned())
    }

    fn find(css: &str) -> Step {
        Step::Find(css.to_owned())
    }

    /// The steps of one referral from the page the browser is on: build the
    /// link, find it, click it and wait for the page it leads to.
    fn hop(to: &str) -> Vec<Step> {
        vec![
            link(to),
            find("a.analytics.referral.test"),
            Step::Click,
            Step::WaitForLoad,
        ]
    }

    #[test]
    fn the_link_script_keeps_the_destination_inside_its_string() {
        for destination in HOSTILE {
            let script = referral_link_script(destination);
            let read = ReadScript::read(&script).unwrap_or_else(|why| panic!("{why}: {script:?}"));
            assert_eq!(
                read.skeleton.split_whitespace().collect::<Vec<_>>(),
                LINK_SKELETON.split_whitespace().collect::<Vec<_>>(),
                "script: {script:?}"
            );
            assert_eq!(
                read.literals,
                [
                    "a",
                    "analytics referral test",
                    "href",
                    destination,
                    "style",
                    LINK_STYLE,
                    "Magic Link Button",
                    "body"
                ],
                "script: {script:?}"
            );
        }
    }

    #[tokio::test]
    async fn a_referral_opens_the_start_page_then_clicks_a_link_to_the_destination() {
        let driver = driver();
        let mut sb = driver.base_case().await;

        sb.generate_referral("https://start.test/home", "https://dest.test/landing", None)
            .await
            .unwrap();

        let mut expected = vec![open("https://start.test/home"), Step::WaitForLoad];
        expected.extend(hop("https://dest.test/landing"));
        assert_eq!(steps(&driver), expected);
    }

    #[tokio::test]
    async fn a_referral_with_no_start_page_links_from_the_page_the_browser_is_on() {
        let driver = driver();
        let mut sb = driver.base_case().await;

        sb.generate_referral("", "https://dest.test/landing", None)
            .await
            .unwrap();

        assert_eq!(steps(&driver), hop("https://dest.test/landing"));
    }

    #[tokio::test]
    async fn a_referral_with_a_selector_clicks_it_on_the_destination_page() {
        let driver = driver();
        let mut sb = driver.base_case().await;

        sb.generate_referral("", "https://dest.test/landing", Some("#read-more"))
            .await
            .unwrap();

        let mut expected = hop("https://dest.test/landing");
        expected.extend([find("#read-more"), Step::Click]);
        assert_eq!(steps(&driver), expected);
    }

    #[tokio::test]
    async fn a_referral_to_something_that_is_not_a_url_sends_nothing() {
        let driver = driver();
        let mut sb = driver.base_case().await;

        for destination in ["not a url", "javascript:alert(1)", ""] {
            let error = sb
                .generate_referral("https://start.test/home", destination, None)
                .await
                .expect_err("the destination is not a URL");
            assert!(
                matches!(&error, SeleniumBaseError::Parse { input, .. } if input == destination),
                "{error:?}"
            );
        }
        let error = sb
            .generate_referral("not a url", "https://dest.test/landing", None)
            .await
            .expect_err("the start page is not a URL");
        assert!(
            matches!(&error, SeleniumBaseError::Parse { input, .. } if input == "not a url"),
            "{error:?}"
        );

        assert_eq!(steps(&driver), []);
    }

    #[tokio::test]
    async fn traffic_repeats_the_referral_once_per_loop() {
        let driver = driver();
        let mut sb = driver.base_case().await;

        sb.generate_traffic("https://start.test/home", "https://dest.test/landing", 2, None)
            .await
            .unwrap();

        let mut once = vec![open("https://start.test/home"), Step::WaitForLoad];
        once.extend(hop("https://dest.test/landing"));
        let expected: Vec<Step> = once.into_iter().chain(
            [open("https://start.test/home"), Step::WaitForLoad]
                .into_iter()
                .chain(hop("https://dest.test/landing")),
        )
        .collect();
        assert_eq!(steps(&driver), expected);
    }

    #[tokio::test]
    async fn traffic_with_no_loops_does_nothing() {
        let driver = driver();
        let mut sb = driver.base_case().await;

        sb.generate_traffic("https://start.test/home", "https://dest.test/landing", 0, None)
            .await
            .unwrap();

        assert_eq!(steps(&driver), []);
    }

    #[tokio::test]
    async fn a_referral_chain_clicks_through_each_page_from_the_one_before() {
        let driver = driver();
        let mut sb = driver.base_case().await;

        sb.generate_referral_chain(&["https://a.test/one", "https://b.test/two", "https://c.test/three"])
            .await
            .unwrap();

        let expected: Vec<Step> = ["https://a.test/one", "https://b.test/two", "https://c.test/three"]
            .iter()
            .flat_map(|page| hop(page))
            .collect();
        assert_eq!(steps(&driver), expected);
    }

    #[tokio::test]
    async fn a_chain_needs_two_pages_and_checks_them_all_before_it_starts() {
        let driver = driver();
        let mut sb = driver.base_case().await;

        let error = sb
            .generate_referral_chain(&["https://a.test/one"])
            .await
            .expect_err("one page is not a chain");
        assert!(
            matches!(&error, SeleniumBaseError::InvalidConfig(message) if message.contains("two")),
            "{error:?}"
        );
        let error = sb
            .generate_referral_chain(&["https://a.test/one", "https://b.test/two", "nope"])
            .await
            .expect_err("the last page is not a URL");
        assert!(
            matches!(&error, SeleniumBaseError::Parse { input, .. } if input == "nope"),
            "{error:?}"
        );
        let error = sb
            .generate_traffic_chain(&["https://a.test/one"], 3)
            .await
            .expect_err("one page is not a chain");
        assert!(matches!(error, SeleniumBaseError::InvalidConfig(_)), "{error:?}");

        assert_eq!(steps(&driver), [], "nothing was clicked before the pages were checked");
    }

    #[tokio::test]
    async fn a_traffic_chain_repeats_the_chain_once_per_loop() {
        let driver = driver();
        let mut sb = driver.base_case().await;

        sb.generate_traffic_chain(&["https://a.test/one", "https://b.test/two"], 2)
            .await
            .unwrap();

        let expected: Vec<Step> = ["https://a.test/one", "https://b.test/two"]
            .iter()
            .cycle()
            .take(4)
            .flat_map(|page| hop(page))
            .collect();
        assert_eq!(steps(&driver), expected);
    }
}
