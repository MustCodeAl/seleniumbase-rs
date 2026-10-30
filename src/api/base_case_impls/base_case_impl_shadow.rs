// Shadow DOM piercing helpers for BaseCase.

use crate::utils::shadow::is_shadow_selector;


impl BaseCase {
    /// Returns true if the selector crosses a shadow root: it has the
    /// `::shadow` combinator followed by white space, as Python's
    /// `"::shadow " in selector` says.
    ///
    /// Every method that takes a selector reads such a selector, the way
    /// Python does: `sb.click("my-app::shadow button")` finds `my-app` in the
    /// page and `button` in its shadow root, and waits for it like any other
    /// click. A selector crosses one root for each `::shadow`, and must end on
    /// an element inside the last one.
    #[must_use]
    pub fn is_shadow_selector(&self, selector: &str) -> bool {
        is_shadow_selector(selector)
    }

    /// The element `selector` names, once it is `ready`, and not before the
    /// case's timeout is out.
    async fn shadow_element(
        &self,
        selector: &str,
        ready: Ready,
    ) -> Result<thirtyfour::WebElement, SeleniumBaseError> {
        self.await_ready(selector, ready).await?;
        self.session.find(Selector::auto(selector).to_by()?).await
    }

    /// Clicks an element inside one or more shadow roots, once it can be
    /// clicked. The same as [`click`](Self::click), which reads the same
    /// selectors, for a case that is only borrowed.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::WaitTimeout`] if the element is not clickable in
    /// the case's timeout.
    pub async fn shadow_click(&self, selector: &str) -> Result<(), SeleniumBaseError> {
        let element = self.shadow_element(selector, Ready::Clickable).await?;
        element.click().await?;
        Ok(())
    }

    /// Types text into an element inside one or more shadow roots, once it is
    /// displayed, after clearing it. The same as
    /// [`type_text`](Self::type_text), for a case that is only borrowed.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::WaitTimeout`] if the element is not displayed in
    /// the case's timeout.
    pub async fn shadow_type(
        &self,
        selector: &str,
        text: &str,
    ) -> Result<(), SeleniumBaseError> {
        let element = self.shadow_element(selector, Ready::Visible).await?;
        element.clear().await?;
        element.send_keys(text).await?;
        Ok(())
    }

    /// Returns the text of an element inside shadow roots, as the browser
    /// shows it, once the element is displayed. A container that only slots
    /// its content gives the text of the slotted nodes, as in Python.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::WaitTimeout`] if the element is not displayed in
    /// the case's timeout.
    pub async fn shadow_get_text(&self, selector: &str) -> Result<String, SeleniumBaseError> {
        let element = self.shadow_element(selector, Ready::Visible).await?;
        shown_text(&element).await
    }

    /// Returns an attribute value of an element inside shadow roots, once the
    /// element is in the page: the attribute if the element has it, else the
    /// DOM property of that name, as [`get_attribute`](Self::get_attribute)
    /// does.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::WaitTimeout`] if there is no such element in the
    /// case's timeout.
    pub async fn shadow_get_attribute(
        &self,
        selector: &str,
        attribute: &str,
    ) -> Result<Option<String>, SeleniumBaseError> {
        let element = self.shadow_element(selector, Ready::Present).await?;
        self.session.attribute_or_property(&element, attribute).await
    }

    /// Asserts that a shadow-pierced element exists, waiting up to the case's
    /// timeout for it. The same as
    /// [`assert_element_present`](Self::assert_element_present), which reads
    /// the same selectors.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::AssertionFailed`] if there is no such element
    /// when the timeout ends.
    pub async fn assert_shadow_element(&self, selector: &str) -> Result<(), SeleniumBaseError> {
        self.assert_element_present(selector).await
    }

    /// Asserts that text appears inside a shadow-pierced element, waiting up
    /// to the case's timeout for it. The text comes first and the selector
    /// second, like [`assert_text`](Self::assert_text), which this is.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::AssertionFailed`] if the element does not show
    /// the text when the timeout ends.
    pub async fn assert_shadow_text(
        &self,
        text: &str,
        selector: &str,
    ) -> Result<(), SeleniumBaseError> {
        self.assert_text(text, selector).await
    }
}

#[cfg(test)]
mod tests_shadow {
    use super::webdriver_support::{element, MockWebDriver, Reply, Seen};
    use super::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    /// The key W3C WebDriver gives a shadow root reference.
    const SHADOW_KEY: &str = "shadow-6066-11e4-a52e-4f735466cecf";

    /// A page with `<app-root>` whose shadow root holds `<h1>` (which says
    /// `says(n)` the n-th time its text is read, counting from 0) and
    /// `<button id=go>`. Anything else finds nothing, and `<p>` has no shadow
    /// root.
    fn page(says: fn(usize) -> &'static str) -> MockWebDriver {
        let reads = Arc::new(AtomicUsize::new(0));
        MockWebDriver::start(json!({}), move |request: &Seen| {
            let asked_for = request.body["value"].as_str().unwrap_or_default();
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/element") if asked_for == "app-root" => Reply::Value(element("host")),
                ("POST", "/element") if asked_for == "p" => Reply::Value(element("plain")),
                ("GET", "/element/host/shadow") => Reply::Value(json!({ SHADOW_KEY: "root" })),
                ("GET", "/element/plain/shadow") => Reply::Error(404, "no such shadow root"),

                ("POST", "/execute/sync") => Reply::Value(Value::Null),
                ("POST", "/shadow/root/element" | "/element/root/element") => match asked_for {
                    "h1" => Reply::Value(element("heading")),
                    "#go" => Reply::Value(element("go")),
                    _ => Reply::Error(404, "no such element"),
                },
                ("POST", "/shadow/root/elements" | "/element/root/elements") => match asked_for {
                    "h1" => Reply::Value(json!([element("heading")])),
                    _ => Reply::Value(json!([])),
                },
                ("GET", "/element/heading/text") => Reply::Value(json!(says(
                    reads.fetch_add(1, Ordering::SeqCst)
                ))),
                ("GET", "/element/heading/name") => Reply::Value(json!("h1")),
                ("GET", "/element/heading/displayed" | "/element/go/displayed") => {
                    Reply::Value(json!(true))
                }
                ("GET", "/element/go/enabled") => Reply::Value(json!(true)),
                _ => Reply::Value(Value::Null),
            }
        })
    }

    /// Asserts that the only scripts the driver ran are the ones that read a
    /// host's `shadowRoot` (how the WebDriver client reaches a root): the
    /// lookups, the text and the clicks are the driver's own commands.
    fn assert_only_root_scripts(driver: &MockWebDriver) {
        for request in driver.requests("/execute/sync") {
            let script = request.body["script"].as_str().unwrap_or_default();
            assert!(script.contains("shadowRoot"), "an unexpected script: {script}");
        }
    }


    /// The selectors the driver was asked to find, oldest first.
    fn looked_up(driver: &MockWebDriver) -> Vec<String> {
        driver
            .all_requests()
            .into_iter()
            .filter(|request| {
                request.method == "POST"
                    && (request.path.ends_with("/element") || request.path.ends_with("/elements"))
            })
            .map(|request| request.body["value"].as_str().unwrap_or_default().to_owned())
            .collect()
    }

    #[test]
    fn a_selector_is_a_shadow_selector_the_way_python_reads_it() {
        let sb = BaseCase::with_session(BrowserConfig::default(), BrowserSession::disconnected());
        assert!(sb.is_shadow_selector("app::shadow input"));
        assert!(sb.is_shadow_selector("app ::shadow input"));
        assert!(!sb.is_shadow_selector("app input"));
        assert!(!sb.is_shadow_selector("//a/parent::shadow-host"));
    }

    #[tokio::test]
    async fn the_regular_methods_look_in_the_shadow_root_of_the_host() {
        let driver = page(|_| "Welcome");
        let mut sb = driver.base_case().await;
        sb.set_timeout(1).await.unwrap();

        sb.click("app-root::shadow #go").await.expect("the button is clicked");
        assert_eq!(sb.get_text("app-root ::shadow h1").await.unwrap(), "Welcome");
        sb.assert_exact_text("Welcome", "app-root::shadow h1")
            .await
            .expect("the heading says Welcome");
        sb.assert_element("app-root::shadow h1").await.expect("the heading shows");
        assert!(sb.is_element_visible("app-root::shadow #go").await.unwrap());
        assert!(!sb.is_element_present("app-root::shadow #nope").await.unwrap());
        assert_eq!(sb.find_elements("app-root::shadow h1").await.unwrap().len(), 1);

        // The host is looked for in the page, the rest in its root: the driver
        // is never handed a selector with the combinator in it.
        let asked = looked_up(&driver);
        assert!(asked.contains(&"app-root".to_owned()), "{asked:?}");
        assert!(asked.contains(&"#go".to_owned()), "{asked:?}");
        assert!(
            asked.iter().all(|selector| !selector.contains("::shadow")),
            "{asked:?}"
        );
        assert!(
            !driver.requests("/element/go/click").is_empty(),
            "{:?}",
            driver.all_requests()
        );
        assert_only_root_scripts(&driver);
    }

    #[tokio::test]
    async fn a_host_without_a_shadow_root_is_not_found_and_does_not_hang() {
        let driver = page(|_| "Welcome");
        let mut sb = driver.base_case().await;
        sb.set_timeout(1).await.unwrap();

        assert!(!sb.is_element_present("p::shadow h1").await.unwrap());
        assert!(!sb.is_element_visible("p::shadow h1").await.unwrap());
        sb.click("p::shadow h1").await.expect_err("p has no shadow root");
        sb.get_text("nope::shadow h1").await.expect_err("nope is not in the page");
    }

    #[tokio::test]
    async fn a_selector_that_ends_on_a_shadow_root_is_refused_before_the_driver_is_asked() {
        let driver = page(|_| "Welcome");
        let mut sb = driver.base_case().await;

        for selector in ["app-root ::shadow", "app-root::shadow"] {
            let message = sb
                .get_text(selector)
                .await
                .expect_err("a selector must end inside a shadow root")
                .to_string();
            assert!(message.contains("cannot end on a shadow root"), "{message}");
            sb.click(selector).await.expect_err("the same for a click");
        }
        assert!(looked_up(&driver).is_empty(), "{:?}", looked_up(&driver));
    }

    #[tokio::test]
    async fn the_shadow_helpers_wait_for_the_text_that_arrives_late() {
        // The heading says "Loading" the first two times it is read.
        let driver = page(|read| if read < 2 { "Loading" } else { "Ready" });
        let mut sb = driver.base_case().await;
        sb.set_timeout(5).await.unwrap();

        sb.assert_shadow_text("Ready", "app-root ::shadow h1")
            .await
            .expect("the text shows within the timeout");
        assert_only_root_scripts(&driver);

        // It gives up with the case's timeout when the text never comes.
        sb.set_timeout(1).await.unwrap();
        let started = std::time::Instant::now();
        sb.assert_shadow_text("Never", "app-root ::shadow h1")
            .await
            .expect_err("the heading never says that");
        assert!(started.elapsed() >= std::time::Duration::from_millis(900));
        sb.assert_shadow_element("app-root ::shadow h1")
            .await
            .expect("the heading is there");
        sb.assert_shadow_element("app-root ::shadow nope")
            .await
            .expect_err("nothing is called nope");
    }

    #[tokio::test]
    async fn the_shadow_helpers_read_what_the_browser_shows() {
        let driver = page(|_| "Content Panel 1");
        let mut sb = driver.base_case().await;
        sb.set_timeout(1).await.unwrap();

        // The browser's text, not a script's textContent: a container that
        // only slots its content has none of its own.
        assert_eq!(
            sb.shadow_get_text("app-root ::shadow h1").await.unwrap(),
            "Content Panel 1"
        );
        sb.shadow_click("app-root ::shadow #go").await.unwrap();
        assert_only_root_scripts(&driver);
        assert_eq!(driver.requests("/element/go/click").len(), 1);
    }
}
