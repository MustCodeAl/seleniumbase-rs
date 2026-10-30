// Tests of the scripts `BaseCase` sends to a page and of how its waits end,
// against the mock WebDriver in `webdriver_support` (no browser needed).
// Included (for tests only) at the end of `base_case.rs` like the files beside it.

/// A script cut into its code and its string literals, the way a JavaScript
/// engine reads it, so a test can see whether text that came from a caller
/// stayed *data*.
#[derive(Debug, PartialEq)]
struct ReadScript {
    /// The code with every string literal replaced by `S`.
    skeleton: String,
    /// What each string literal evaluates to, in order.
    literals: Vec<String>,
}

impl ReadScript {
    /// Reads `script`, or says why an engine would not: a literal that never
    /// ends, or one with a raw line terminator in it (which ends the literal
    /// in engines older than ES2019, or is a syntax error for `\n` and `\r`).
    fn read(script: &str) -> Result<Self, String> {
        fn hex(chars: &mut std::str::Chars<'_>, digits: usize) -> Result<char, String> {
            let text: String = chars.by_ref().take(digits).collect();
            u32::from_str_radix(&text, 16)
                .ok()
                .and_then(char::from_u32)
                .ok_or_else(|| format!("bad escape \\x{text}"))
        }

        let mut skeleton = String::new();
        let mut literals = Vec::new();
        let mut chars = script.chars();
        while let Some(c) = chars.next() {
            if c != '\'' && c != '"' {
                skeleton.push(c);
                continue;
            }
            let mut value = String::new();
            loop {
                match chars.next().ok_or("a string literal never ends")? {
                    end if end == c => break,
                    '\\' => match chars.next().ok_or("a backslash ends the script")? {
                        'n' => value.push('\n'),
                        'r' => value.push('\r'),
                        't' => value.push('\t'),
                        'b' => value.push('\u{8}'),
                        'f' => value.push('\u{c}'),
                        'v' => value.push('\u{b}'),
                        '0' => value.push('\0'),
                        'x' => value.push(hex(&mut chars, 2)?),
                        'u' => value.push(hex(&mut chars, 4)?),
                        '\n' | '\r' | '\u{2028}' | '\u{2029}' => {
                            return Err("a line continuation inside a string literal".to_owned())
                        }
                        other => value.push(other),
                    },
                    '\n' | '\r' | '\u{2028}' | '\u{2029}' => {
                        return Err("a raw line terminator inside a string literal".to_owned())
                    }
                    other => value.push(other),
                }
            }
            skeleton.push('S');
            literals.push(value);
        }
        Ok(Self { skeleton, literals })
    }
}

mod tests_page_scripts {
    use super::webdriver_support::{element, MockWebDriver, Reply};
    use super::*;
    use serde_json::json;

    /// Text a test case or a page could hold, each of which breaks a script
    /// that pastes it between quotes or lets it end the literal.
    const HOSTILE: &[&str] = &[
        "a[title='x']",
        "x'); alert(1); ('",
        r#"say "hi" and C:\dir\new"#,
        "line one\nline two\r\n\ttabbed",
        "separators \u{2028} and \u{2029} inside",
        "</script><!-- &amp; -->",
        "emoji 🦀 and 日本語",
        "",
    ];

    /// A driver that finds one element for any selector and answers every
    /// other command with `null`.
    fn quiet_driver() -> MockWebDriver {
        MockWebDriver::start(json!({}), |request| {
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/element") => Reply::Value(element("e1")),
                ("POST", "/elements") => Reply::Value(json!([element("e1")])),
                _ => Reply::Value(serde_json::Value::Null),
            }
        })
    }

    /// The last script the driver was asked to run.
    fn last_script(driver: &MockWebDriver) -> String {
        driver
            .requests("/execute/sync")
            .last()
            .and_then(|request| request.body["script"].as_str().map(str::to_owned))
            .expect("a script was sent")
    }

    /// `text` with every run of white space as one space.
    fn squash(text: &str) -> String {
        text.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    /// Checks that `script` is `skeleton` (white space aside) with exactly
    /// `literals` in its string literals, so nothing the caller passed became
    /// code.
    fn assert_inert(script: &str, skeleton: &str, literals: &[&str]) {
        let read = ReadScript::read(script).unwrap_or_else(|why| panic!("{why}: {script:?}"));
        assert_eq!(
            squash(&read.skeleton),
            squash(skeleton),
            "script: {script:?}"
        );
        assert_eq!(read.literals, literals, "script: {script:?}");
    }

    /// Calls `$call` once for each text in `HOSTILE` (named `$text` for the
    /// call and the literals) and checks the script it sent with
    /// `assert_inert`. The call may fail; the script is what is checked.
    macro_rules! assert_script {
        ($driver:expr, $text:ident, $call:expr, $skeleton:expr, $literals:expr) => {
            for $text in HOSTILE {
                let _ = $call.await;
                assert_inert(&last_script(&$driver), $skeleton, &$literals);
            }
        };
    }

    #[test]
    fn the_reader_sees_through_quotes_and_escapes() {
        let read = ReadScript::read(r#"f('it\'s', "a\"b\n\u003c", 'x');"#).unwrap();
        assert_eq!(read.skeleton, "f(S, S, S);");
        assert_eq!(read.literals, ["it's", "a\"b\n<", "x"]);
    }

    #[test]
    fn the_reader_rejects_what_an_engine_would_reject() {
        assert!(ReadScript::read("f('x');").is_ok());
        assert!(
            ReadScript::read("f('it's');").is_err(),
            "an apostrophe ends the literal"
        );
        assert!(ReadScript::read("f('a\nb');").is_err(), "a raw newline");
        assert!(
            ReadScript::read("f(\"a\u{2028}b\");").is_err(),
            "a raw line separator"
        );
        assert!(ReadScript::read("f(\"open").is_err(), "no end");
    }

    #[tokio::test]
    async fn js_click_keeps_the_selector_out_of_its_script() {
        let driver = quiet_driver();
        let mut sb = driver.base_case().await;
        for css in HOSTILE {
            if css.is_empty() {
                            // An empty selector is refused before a script is sent.
                            sb.js_click(css).await.unwrap_err();
                            continue;
                        }
                        sb.js_click(css).await.unwrap();
            assert_inert(&last_script(&driver), "arguments[0].click();", &[]);
        }
    }

    #[tokio::test]
    async fn js_type_keeps_the_selector_and_the_text_inside_their_strings() {
        let driver = quiet_driver();
        let mut sb = driver.base_case().await;
        for text in HOSTILE {
            let css = format!("input[name='{text}']");
            sb.js_type(&css, text).await.unwrap();
            assert_inert(
                &last_script(&driver),
                "document.querySelector(S).value = S;",
                &[&css, text],
            );
        }
    }

    #[tokio::test]
    async fn set_attribute_keeps_every_argument_inside_its_string() {
        let driver = quiet_driver();
        let mut sb = driver.base_case().await;
        for value in HOSTILE {
            let css = format!("a[title='{value}']");
            let attribute = format!("data-{value}");
            sb.set_attribute(&css, &attribute, value).await.unwrap();
            assert_inert(
                &last_script(&driver),
                "document.querySelector(S).setAttribute(S, S);",
                &[&css, &attribute, value],
            );
        }
    }

    #[tokio::test]
    async fn remove_attribute_keeps_every_argument_inside_its_string() {
        let driver = quiet_driver();
        let mut sb = driver.base_case().await;
        for name in HOSTILE {
            let css = format!("a[title='{name}']");
            sb.remove_attribute(&css, name).await.unwrap();
            assert_inert(
                &last_script(&driver),
                "document.querySelector(S).removeAttribute(S);",
                &[&css, name],
            );
        }
    }

    #[tokio::test]
    async fn scroll_to_keeps_the_selector_inside_its_string() {
        let driver = quiet_driver();
        let sb = driver.base_case().await;
        for css in HOSTILE {
            sb.scroll_to(css).await.unwrap();
            assert_inert(
                &last_script(&driver),
                "document.querySelector(S).scrollIntoView();",
                &[css],
            );
        }
    }

    #[tokio::test]
    async fn local_storage_keys_and_values_stay_inside_their_strings() {
        let driver = quiet_driver();
        let sb = driver.base_case().await;
        for text in HOSTILE {
            let key = format!("key '{text}'");
            sb.set_local_storage_item(&key, text).await.unwrap();
            assert_inert(
                &last_script(&driver),
                "window.localStorage.setItem(S, S);",
                &[&key, text],
            );
            sb.get_local_storage_item(&key).await.unwrap();
            assert_inert(
                &last_script(&driver),
                "return window.localStorage.getItem(S);",
                &[&key],
            );
            sb.remove_local_storage_item(&key).await.unwrap();
            assert_inert(
                &last_script(&driver),
                "window.localStorage.removeItem(S);",
                &[&key],
            );
        }
    }

    /// Checks that `script` reads as complete string literals and that each of
    /// `values` is one of them. For scripts too long to spell out in full.
    fn assert_carries(script: &str, values: &[&str]) {
        let read = ReadScript::read(script).unwrap_or_else(|why| panic!("{why}: {script:?}"));
        for value in values {
            assert!(
                read.literals.iter().any(|literal| literal == value),
                "{value:?} is not a string literal of {script:?}"
            );
        }
    }

    #[tokio::test]
    async fn the_element_helpers_keep_their_arguments_inside_strings() {
        let driver = quiet_driver();
        let mut sb = driver.base_case().await;
        assert_script!(
            driver,
            css,
            sb.focus(css),
            "document.querySelector(S).focus();",
            [css]
        );
        assert_script!(
            driver,
            css,
            sb.js_click_all(css),
            "document.querySelectorAll(S).forEach(e => e.click());",
            [css]
        );
        assert_script!(
            driver,
            css,
            sb.jquery_click(css),
            "(function(){ if (window.jQuery && jQuery(S).length) { jQuery(S)[0].click(); } \
             else { document.querySelector(S).click(); } })();",
            [css, css, css]
        );
        assert_script!(
            driver,
            css,
            sb.hide_element(css),
            "document.querySelector(S).style.display=S;",
            [css, "none"]
        );
        assert_script!(
            driver,
            css,
            sb.show_element(css),
            "document.querySelector(S).style.display=S;",
            [css, "block"]
        );
        assert_script!(
            driver,
            css,
            sb.remove_element(css),
            "var e=document.querySelector(S); if(e) e.parentNode.removeChild(e);",
            [css]
        );
        assert_script!(
            driver,
            value,
            sb.set_value(&format!("input[value='{value}']"), value),
            "document.querySelector(S).value = S;",
            [&format!("input[value='{value}']"), value]
        );
        for css in [HOSTILE[0], HOSTILE[4]] {
            sb.smooth_scroll_to(css).await.unwrap();
            assert_inert(
                &last_script(&driver),
                "document.querySelector(S).scrollIntoView({behavior: S, block: S});",
                &[css, "smooth", "center"],
            );
        }
    }

    #[tokio::test]
    async fn session_storage_keys_and_values_stay_inside_their_strings() {
        let driver = quiet_driver();
        let sb = driver.base_case().await;
        for text in HOSTILE {
            let key = format!("key '{text}'");
            sb.set_session_storage_item(&key, text).await.unwrap();
            assert_inert(
                &last_script(&driver),
                "window.sessionStorage.setItem(S, S);",
                &[&key, text],
            );
            sb.get_session_storage_item(&key).await.unwrap();
            assert_inert(
                &last_script(&driver),
                "return window.sessionStorage.getItem(S);",
                &[&key],
            );
            sb.remove_session_storage_item(&key).await.unwrap();
            assert_inert(
                &last_script(&driver),
                "window.sessionStorage.removeItem(S);",
                &[&key],
            );
        }
    }

    #[tokio::test]
    async fn a_posted_message_stays_inside_its_string() {
        // Messenger answers true to the post script, so the message is posted
        // at once and the last script is the one that carries it.
        let driver = MockWebDriver::start(json!({}), |request| match request.path.as_str() {
            "/execute/sync" => Reply::Value(json!(true)),
            _ => Reply::Value(serde_json::Value::Null),
        });
        let sb = driver.base_case().await;
        for message in HOSTILE {
            sb.post_message(message)
                .duration(Duration::from_millis(1))
                .pause(false)
                .await
                .unwrap();
            assert_carries(&last_script(&driver), &[message]);
        }
    }

    #[tokio::test]
    async fn the_dom_helpers_keep_their_arguments_inside_strings() {
        let driver = quiet_driver();
        let mut sb = driver.base_case().await;
        assert_script!(
            driver,
            html,
            sb.set_content(html),
            "document.body.innerHTML = S;",
            [html]
        );
        assert_script!(
            driver,
            css,
            sb.remove_elements(css),
            "document.querySelectorAll(S).forEach(el => el.remove());",
            [css]
        );
        assert_script!(
            driver,
            css,
            sb.hide_elements(css),
            "document.querySelectorAll(S).forEach(el => el.style.display = S);",
            [css, "none"]
        );
        assert_script!(
            driver,
            css,
            sb.show_elements(css),
            "document.querySelectorAll(S).forEach(el => el.style.display = S);",
            [css, ""]
        );
        assert_script!(
            driver,
            name,
            sb.remove_attributes(name, name),
            "document.querySelectorAll(S).forEach(el => el.removeAttribute(S));",
            [name, name]
        );
        assert_script!(
            driver,
            value,
            sb.set_attributes(value, value, value),
            "document.querySelectorAll(S).forEach(el => el.setAttribute(S, S));",
            [value, value, value]
        );
        assert_script!(
            driver,
            css,
            sb.get_parent(css),
            "return document.querySelector(S)?.parentElement;",
            [css]
        );
        assert_script!(
            driver,
            href,
            sb.add_css_link(href),
            "var l=document.createElement(S); l.rel=S; l.href=S; document.head.appendChild(l);",
            ["link", "stylesheet", href]
        );
        assert_script!(
            driver,
            src,
            sb.add_js_link(src),
            "var s=document.createElement(S); s.src=S; document.head.appendChild(s);",
            ["script", src]
        );
        assert_script!(
            driver,
            css,
            sb.add_css_style(css),
            "var s=document.createElement(S); s.textContent=S; document.head.appendChild(s);",
            ["style", css]
        );
        assert_script!(
            driver,
            js,
            sb.add_js_code(js),
            "var s=document.createElement(S); s.textContent=S; document.head.appendChild(s);",
            ["script", js]
        );
        assert_script!(
            driver,
            text,
            sb.add_meta_tag(text, text),
            "var m=document.createElement(S); m.name=S; m.content=S; \
             document.head.appendChild(m);",
            ["meta", text, text]
        );
    }

    #[tokio::test]
    async fn the_attribute_and_select_helpers_keep_their_arguments_inside_strings() {
        let driver = quiet_driver();
        let mut sb = driver.base_case().await;
        assert_script!(
            driver,
            text,
            sb.is_attribute_present(text, text),
            "var el = document.querySelector(S); return !!(el && el.hasAttribute(S));",
            [text, text]
        );
        assert_script!(
            driver,
            css,
            sb.scroll_into_view(css),
            "document.querySelector(S).scrollIntoView();",
            [css]
        );
        assert_script!(
            driver,
            css,
            sb.slow_scroll_to(css),
            "document.querySelector(S).scrollIntoView({behavior: S});",
            [css, "smooth"]
        );
        assert_script!(
            driver,
            css,
            sb.select_all_options(css),
            "var s = document.querySelector(S); if (s) { for (var i = 0; \
             i < s.options.length; i++) s.options[i].selected = true; \
             s.dispatchEvent(new Event(S)); }",
            [css, "change"]
        );
        assert_script!(
            driver,
            css,
            sb.unselect_all(css),
            "var s = document.querySelector(S); if (s) { for (var i = 0; \
             i < s.options.length; i++) s.options[i].selected = false; \
             s.dispatchEvent(new Event(S)); }",
            [css, "change"]
        );
    }

    #[tokio::test]
    async fn the_jquery_style_helpers_keep_their_arguments_inside_strings() {
        let driver = quiet_driver();
        let mut sb = driver.base_case().await;
        assert_script!(
            driver,
            css,
            sb.jquery_click_all(css),
            "document.querySelectorAll(S).forEach(el => el.click());",
            [css]
        );
        assert_script!(
            driver,
            text,
            sb.jquery_type(text, text),
            "document.querySelectorAll(S).forEach(el => { el.value = S; \
             el.dispatchEvent(new Event(S)); el.dispatchEvent(new Event(S)); });",
            [text, text, "input", "change"]
        );
        assert_script!(
            driver,
            text,
            sb.jquery_update_text(text, text),
            "document.querySelectorAll(S).forEach(el => el.textContent = S);",
            [text, text]
        );
        assert_script!(
            driver,
            text,
            sb.js_update_text(text, text),
            "document.querySelectorAll(S).forEach(el => { if (S in el) el.value = S; \
             el.dispatchEvent(new Event(S)); el.dispatchEvent(new Event(S)); });",
            [text, "value", text, "input", "change"]
        );
    }

    #[test]
    fn the_scripts_built_from_a_url_or_a_selector_keep_it_inside_its_string() {
        for text in HOSTILE {
            assert_carries(&element_in_iframe_script(text), &[text]);
            for kind in [PageResource::Script, PageResource::Stylesheet] {
                assert_carries(&load_resource_script(kind, text, None), &[text]);
            }
        }
    }
}

mod tests_wait_errors {
    use super::webdriver_support::{element, MockWebDriver, Reply};
    use super::*;
    use serde_json::{json, Value};

    /// A driver whose page has the given `title`, finds an element for every
    /// lookup when `found` (and none otherwise), and answers every script with
    /// `answer`.
    fn mock_page(title: &str, found: bool, answer: Value) -> MockWebDriver {
        let title = title.to_owned();
        MockWebDriver::start(json!({}), move |request| {
            match (request.method.as_str(), request.path.as_str()) {
                ("GET", "/title") => Reply::Value(json!(title)),
                ("POST", "/execute/sync") => Reply::Value(answer.clone()),
                ("POST", "/element") if found => Reply::Value(element("e1")),
                ("POST", "/element") => Reply::Error(404, "no such element"),
                ("POST", "/elements") if found => Reply::Value(json!([element("e1")])),
                ("POST", "/elements") => Reply::Value(json!([])),
                _ => Reply::Value(Value::Null),
            }
        })
    }

    /// The message of the `WaitTimeout` in `result`; panics with what came
    /// instead (an `AssertionFailed` is what these waits used to return).
    fn timed_out<T: std::fmt::Debug>(result: Result<T, SeleniumBaseError>) -> String {
        match result {
            Err(SeleniumBaseError::WaitTimeout(message)) => message,
            other => panic!("expected a WaitTimeout, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_title_that_never_matches_times_out_naming_the_title_it_saw() {
        let driver = mock_page("Welcome page", false, Value::Null);
        let mut sb = driver.base_case().await;
        let message = timed_out(sb.wait_for_title("Dashboard", 0).await);
        assert!(message.contains("Dashboard"), "{message}");
        assert!(message.contains("Welcome page"), "{message}");
    }

    #[tokio::test]
    async fn a_title_that_matches_ends_the_wait() {
        let driver = mock_page("My Dashboard", false, Value::Null);
        let mut sb = driver.base_case().await;
        sb.wait_for_title("Dashboard", 0).await.unwrap();
    }

    #[tokio::test]
    async fn an_attribute_that_never_appears_times_out() {
        let driver = mock_page("", false, json!(false));
        let mut sb = driver.base_case().await;
        let message = timed_out(sb.wait_for_attribute("#app", "data-ready").timeout(Duration::ZERO).await);
        assert!(
            message.contains("#app") && message.contains("data-ready"),
            "{message}"
        );

        let driver = driver_with_answer(true);
        let mut sb = driver.base_case().await;
        sb.wait_for_attribute("#app", "data-ready").timeout(Duration::ZERO)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn an_attribute_that_never_goes_away_times_out() {
        let driver = mock_page("", false, json!(true));
        let mut sb = driver.base_case().await;
        let message = timed_out(
            sb.wait_for_attribute_not_present("#app", "data-busy").timeout(Duration::ZERO)
                .await,
        );
        assert!(
            message.contains("#app") && message.contains("data-busy"),
            "{message}"
        );

        let driver = driver_with_answer(false);
        let mut sb = driver.base_case().await;
        sb.wait_for_attribute_not_present("#app", "data-busy").timeout(Duration::ZERO)
            .await
            .unwrap();
    }

    /// A driver that answers every script with `answer`.
    fn driver_with_answer(answer: bool) -> MockWebDriver {
        mock_page("", false, json!(answer))
    }

    #[tokio::test]
    async fn text_that_never_appears_times_out() {
        let driver = mock_page("", false, json!(false));
        let sb = driver.base_case().await;
        let message = timed_out(sb.wait_for_exact_text("All done", "#status", 0).await);
        assert!(
            message.contains("All done") && message.contains("#status"),
            "{message}"
        );
    }

    #[tokio::test]
    async fn a_link_that_never_appears_times_out() {
        let driver = mock_page("", false, Value::Null);
        let sb = driver.base_case().await;
        let message = timed_out(sb.wait_for_link_text("Sign in", 0).await);
        assert!(message.contains("Sign in"), "{message}");
        let message = timed_out(sb.wait_for_partial_link_text("Sign", 0).await);
        assert!(message.contains("Sign"), "{message}");

        let driver = mock_page("", true, Value::Null);
        let sb = driver.base_case().await;
        sb.wait_for_link_text("Sign in", 0).await.unwrap();
        sb.wait_for_partial_link_text("Sign", 0).await.unwrap();
    }

    #[tokio::test]
    async fn none_of_the_elements_appearing_times_out_naming_them() {
        let driver = mock_page("", false, Value::Null);
        let sb = driver.base_case().await;
        let message = timed_out(sb.wait_for_any_of_elements_present(&["#a", "#b"], 0).await);
        assert!(
            message.contains("#a") && message.contains("#b"),
            "{message}"
        );

        let driver = mock_page("", true, Value::Null);
        let sb = driver.base_case().await;
        sb.wait_for_any_of_elements_present(&["#a", "#b"], 0)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn a_selector_that_cannot_match_is_reported_at_once() {
        let driver = mock_page("", false, Value::Null);
        let sb = driver.base_case().await;
        let waited = tokio::time::timeout(
            Duration::from_secs(5),
            sb.wait_for_any_of_elements_present(&["#a", ""], 30),
        )
        .await
        .expect("an empty selector is not worth waiting 30 seconds for");
        assert!(
            matches!(waited, Err(SeleniumBaseError::InvalidSelector(_))),
            "{waited:?}"
        );
    }

    #[tokio::test]
    async fn a_download_that_never_arrives_times_out_and_returns_to_the_first_tab() {
        let driver = MockWebDriver::start(json!({}), |request| {
            match (request.method.as_str(), request.path.as_str()) {
                ("GET", "/window") => Reply::Value(json!("first-tab")),
                ("POST", "/window/new") => {
                    Reply::Value(json!({ "handle": "download-tab", "type": "tab" }))
                }
                _ => Reply::Value(Value::Null),
            }
        });
        let mut sb = driver.base_case().await;
        let name = format!("sbrs-never-downloaded-{}.bin", std::process::id());

        let message = timed_out(
            sb.download_file_in_browser("https://example.test/file.bin", &name, 0)
                .await,
        );

        assert!(message.contains(&name), "{message}");
        let switches: Vec<String> = driver
            .requests("/window")
            .iter()
            .filter(|request| request.method == "POST")
            .filter_map(|request| request.body["handle"].as_str().map(str::to_owned))
            .collect();
        assert_eq!(switches, ["download-tab", "first-tab"]);
    }
}

mod tests_ready_state {
    use super::webdriver_support::{MockWebDriver, Reply};
    use super::*;
    use serde_json::{json, Value};
    use std::time::Instant;

    /// A driver whose page is in `state`.
    fn page_in(state: &'static str) -> MockWebDriver {
        MockWebDriver::start(json!({}), move |request| {
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/execute/sync") => Reply::Value(json!(state)),
                _ => Reply::Value(Value::Null),
            }
        })
    }

    /// Waits for the ready state of a page that stays `loading`, and says how
    /// long the wait lasted. Gives up on the wait itself after five seconds so
    /// a wait that ignores its limit fails the test instead of hanging it.
    async fn wait_on_a_loading_page(sb: &BaseCase) -> (Result<(), SeleniumBaseError>, Duration) {
        let started = Instant::now();
        let waited =
            tokio::time::timeout(Duration::from_secs(5), sb.wait_for_ready_state_complete())
                .await
                .expect("the wait ran past five seconds: it is not using the configured timeout");
        (waited, started.elapsed())
    }

    #[tokio::test]
    async fn a_complete_page_is_ready_at_once() {
        let driver = page_in("complete");
        let sb = driver.base_case().await;
        sb.wait_for_ready_state_complete().await.unwrap();
    }

    #[tokio::test]
    async fn the_wait_lasts_as_long_as_the_timeout_the_test_set() {
        let driver = page_in("loading");
        let mut sb = driver.base_case().await;
        sb.set_timeout(1).await.unwrap();

        let (waited, lasted) = wait_on_a_loading_page(&sb).await;

        let Err(SeleniumBaseError::WaitTimeout(message)) = waited else {
            panic!("expected a WaitTimeout, got {waited:?}");
        };
        assert!(message.contains("readyState"), "{message}");
        assert!(lasted >= Duration::from_secs(1), "gave up after {lasted:?}");
    }

    #[tokio::test]
    async fn the_time_limit_still_cuts_the_wait_short() {
        let driver = page_in("loading");
        let mut sb = driver.base_case().await;
        sb.set_timeout(30).await.unwrap();
        sb.set_time_limit(1).unwrap();

        let (waited, lasted) = wait_on_a_loading_page(&sb).await;

        assert!(
            matches!(waited, Err(SeleniumBaseError::WaitTimeout(_))),
            "{waited:?}"
        );
        assert!(lasted >= Duration::from_secs(1), "gave up after {lasted:?}");
    }

    #[tokio::test]
    async fn skipping_js_waits_returns_at_once_on_a_loading_page() {
        let driver = page_in("loading");
        let mut sb = driver.base_case().await;
        sb.set_run_settings(RunSettings::default().with_skip_js_waits(true));
        let (waited, lasted) = wait_on_a_loading_page(&sb).await;
        waited.unwrap();
        assert!(lasted < Duration::from_secs(1), "waited {lasted:?}");
    }

    #[tokio::test]
    async fn a_test_that_sets_nothing_waits_ten_seconds() {
        let sb = BaseCase::without_session(BrowserConfig::default());
        assert_eq!(sb.timeout_secs(), 10);
        assert_eq!(sb.default_wait_secs(), 10);
    }

    #[tokio::test]
    async fn the_default_wait_follows_the_default_timeout_and_is_cut_to_the_time_left() {
        let mut sb = BaseCase::without_session(BrowserConfig::default());
        sb.set_timeout(25).await.unwrap();
        assert_eq!(sb.default_wait_secs(), 25);
        sb.set_default_timeout(3);
        assert_eq!(sb.default_wait_secs(), 3);
        sb.reset_default_timeout();
        assert_eq!(sb.default_wait_secs(), 25);
        // A test with five seconds left does not wait longer for anything.
        sb.set_time_limit(5).unwrap();
        assert_eq!(sb.default_wait_secs(), 5);
        // A limit above the default does not stretch it.
        sb.set_time_limit(600).unwrap();
        assert_eq!(sb.default_wait_secs(), 25);
        sb.set_time_limit(0).unwrap();
        assert_eq!(sb.default_wait_secs(), 25);
    }
}

// The run settings that hook into a page load or a wait (`--check-js`,
// `--wfa`, `--disable-beforeunload`, `--skip-js-waits`, `--timeout-multiplier`),
// against the mock WebDriver.
mod tests_page_load_hooks {
    use super::webdriver_support::{element, MockWebDriver, Reply};
    use super::*;
    use crate::browser::config::Browser;
    use serde_json::{json, Value};
    use std::time::Instant;

    const LOCAL: &str = "http://localhost/page";

    /// A page that is `complete`, has one element, `#a`, and whose browser
    /// log holds `log`.
    fn page(log: Value) -> MockWebDriver {
        MockWebDriver::start(json!({}), move |request| {
            let script = request.body["script"].as_str().unwrap_or_default();
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/element") => Reply::Value(element("a")),
                ("POST", "/elements") => Reply::Value(json!([element("a")])),
                ("GET", "/element/a/displayed" | "/element/a/enabled") => {
                    Reply::Value(json!(true))
                }
                ("POST", "/log") => Reply::Value(log.clone()),
                ("GET", "/url") => Reply::Value(json!(LOCAL)),
                ("POST", "/execute/sync") if script.contains("readyState") => {
                    Reply::Value(json!("complete"))
                }
                ("POST", "/execute/sync") => Reply::Value(json!(true)),
                _ => Reply::Value(Value::Null),
            }
        })
    }

    async fn case(driver: &MockWebDriver, settings: RunSettings) -> BaseCase {
        let mut sb = driver.base_case().await;
        sb.set_run_settings(settings);
        sb
    }

    /// The settings of `--check-js`, `--wfa` and `--disable-beforeunload`.
    fn all_three() -> RunSettings {
        RunSettings::default()
            .with_check_js(true)
            .with_wait_for_angularjs(true)
            .with_disable_beforeunload(true)
    }

    fn log_reads(driver: &MockWebDriver) -> usize {
        driver.requests("/log").len()
    }

    /// Whether a script with `needle` in it ran.
    fn ran(driver: &MockWebDriver, needle: &str) -> bool {
        driver
            .requests("/execute/sync")
            .iter()
            .any(|request| request.body["script"].as_str().is_some_and(|s| s.contains(needle)))
    }

    /// The script of the AngularJS wait, and the one that switches off the
    /// `beforeunload` dialog.
    const ANGULAR: &str = "injector";
    const BEFOREUNLOAD: &str = "onbeforeunload";

    #[tokio::test]
    async fn a_ready_page_runs_no_hook_unless_asked() {
        let driver = page(json!([]));
        let sb = case(&driver, RunSettings::default()).await;
        sb.wait_for_ready_state_complete().await.unwrap();
        assert_eq!(log_reads(&driver), 0);
        assert!(!ran(&driver, ANGULAR) && !ran(&driver, BEFOREUNLOAD));
    }

    #[tokio::test]
    async fn check_js_reads_the_browser_log_when_the_page_is_ready() {
        let driver = page(json!([]));
        let sb = case(&driver, RunSettings::default().with_check_js(true)).await;
        sb.wait_for_ready_state_complete().await.unwrap();
        assert_eq!(log_reads(&driver), 1);
        assert!(!ran(&driver, ANGULAR) && !ran(&driver, BEFOREUNLOAD));
    }

    #[tokio::test]
    async fn check_js_fails_the_wait_on_a_severe_error() {
        let driver = page(json!([
            { "level": "SEVERE", "message": "http://localhost/app.js 1:1 Uncaught TypeError: x", "timestamp": 3 },
        ]));
        let sb = case(&driver, RunSettings::default().with_check_js(true)).await;
        let error = sb.wait_for_ready_state_complete().await.unwrap_err();
        let SeleniumBaseError::AssertionFailed(message) = error else {
            panic!("expected an assertion failure, got {error:?}");
        };
        assert!(message.contains("Uncaught TypeError"), "{message}");
    }

    #[tokio::test]
    async fn wait_for_angularjs_asks_the_page_about_its_angular_requests() {
        let driver = page(json!([]));
        let sb = case(&driver, RunSettings::default().with_wait_for_angularjs(true)).await;
        sb.wait_for_ready_state_complete().await.unwrap();
        assert!(ran(&driver, ANGULAR));
        assert_eq!(log_reads(&driver), 0);
        assert!(!ran(&driver, BEFOREUNLOAD));
    }

    #[tokio::test]
    async fn disable_beforeunload_runs_its_script_when_the_page_is_ready() {
        let driver = page(json!([]));
        let sb = case(&driver, RunSettings::default().with_disable_beforeunload(true)).await;
        sb.wait_for_ready_state_complete().await.unwrap();
        assert!(ran(&driver, BEFOREUNLOAD));
        assert_eq!(log_reads(&driver), 0);
        assert!(!ran(&driver, ANGULAR));
    }

    #[tokio::test]
    async fn skipping_the_js_waits_skips_all_three_hooks() {
        let driver = page(json!([]));
        let sb = case(&driver, all_three().with_skip_js_waits(true)).await;
        sb.wait_for_ready_state_complete().await.unwrap();
        assert!(driver.requests("/execute/sync").is_empty());
        assert_eq!(log_reads(&driver), 0);
    }

    #[tokio::test]
    async fn a_browser_that_is_not_chromium_has_no_log_and_no_dialog_to_switch_off() {
        let driver = page(json!([]));
        let mut config = BrowserConfig::default().with_webdriver_url(driver.url().to_owned());
        config.auto_start_driver = false;
        config.browser = Browser::Firefox;
        let mut sb = BaseCase::new(config).await.unwrap();
        sb.set_run_settings(all_three());
        sb.wait_for_ready_state_complete().await.unwrap();
        assert_eq!(log_reads(&driver), 0);
        assert!(!ran(&driver, BEFOREUNLOAD));
        assert!(ran(&driver, ANGULAR), "AngularJS is not a Chromium matter");
    }

    #[tokio::test]
    async fn opening_a_page_runs_the_hooks_after_the_navigation() {
        let driver = page(json!([]));
        let mut sb = case(&driver, all_three()).await;
        sb.open(LOCAL).await.unwrap();
        let requests = driver.all_requests();
        let position = |method: &str, path: &str| {
            requests
                .iter()
                .position(|request| request.method == method && request.path == path)
        };
        let navigated = position("POST", "/url").expect("the page was opened");
        let checked = position("POST", "/log").expect("the log was read");
        assert!(navigated < checked, "{requests:?}");
        assert!(ran(&driver, ANGULAR) && ran(&driver, BEFOREUNLOAD));
    }

    #[tokio::test]
    async fn opening_a_page_runs_no_hook_unless_asked() {
        let driver = page(json!([]));
        let mut sb = case(&driver, RunSettings::default()).await;
        sb.open(LOCAL).await.unwrap();
        assert!(driver.requests("/execute/sync").is_empty());
        assert_eq!(log_reads(&driver), 0);
    }

    #[tokio::test]
    async fn skipping_the_js_waits_skips_the_hooks_of_an_open_too() {
        let driver = page(json!([]));
        let mut sb = case(&driver, all_three().with_skip_js_waits(true)).await;
        sb.open(LOCAL).await.unwrap();
        assert!(driver.requests("/execute/sync").is_empty());
        assert_eq!(log_reads(&driver), 0);
    }

    #[tokio::test]
    async fn back_forward_and_refresh_run_the_hooks_too() {
        let driver = page(json!([]));
        let sb = case(&driver, RunSettings::default().with_check_js(true)).await;
        sb.go_back().await.unwrap();
        assert_eq!(log_reads(&driver), 1);
        sb.go_forward().await.unwrap();
        assert_eq!(log_reads(&driver), 2);
        sb.refresh().await.unwrap();
        assert_eq!(log_reads(&driver), 3);
    }

    #[tokio::test]
    async fn a_click_waits_for_angularjs_only_when_asked() {
        let driver = page(json!([]));
        let mut quiet = case(&driver, RunSettings::default()).await;
        quiet.click("#a").await.unwrap();
        assert!(!ran(&driver, ANGULAR));

        let mut waiting = case(&driver, RunSettings::default().with_wait_for_angularjs(true)).await;
        waiting.click("#a").await.unwrap();
        assert!(ran(&driver, ANGULAR));
    }

    #[tokio::test]
    async fn the_multiplier_scales_the_default_wait_but_not_the_time_a_call_names() {
        let mut sb = BaseCase::without_session(BrowserConfig::default());
        sb.set_run_settings(RunSettings::default().with_timeout_multiplier(1.5));
        assert_eq!(sb.default_wait_secs(), 15);
        sb.set_timeout(3).await.unwrap();
        assert_eq!(sb.default_wait_secs(), 5);
        assert_eq!(sb.timeout_secs(), 3, "the default itself is not changed");
        assert_eq!(sb.effective_timeout(7), 7, "a time a call names is kept");
    }

    #[tokio::test]
    async fn a_multiplier_below_half_waits_half_as_long() {
        let mut sb = BaseCase::without_session(BrowserConfig::default());
        sb.set_run_settings(RunSettings::default().with_timeout_multiplier(0.3));
        assert_eq!(sb.default_wait_secs(), 5);
    }

    #[tokio::test]
    async fn the_time_limit_still_cuts_a_scaled_wait() {
        let mut sb = BaseCase::without_session(BrowserConfig::default());
        sb.set_run_settings(RunSettings::default().with_timeout_multiplier(3.0));
        assert_eq!(sb.default_wait_secs(), 30);
        sb.set_time_limit(5).unwrap();
        assert_eq!(sb.default_wait_secs(), 5);
    }

    #[tokio::test]
    async fn the_multiplier_of_the_browser_config_reaches_the_case() {
        let config = BrowserConfig::default()
            .with_run_settings(RunSettings::default().with_timeout_multiplier(2.0));
        assert_eq!(BaseCase::without_session(config).default_wait_secs(), 20);
    }

    #[tokio::test]
    async fn a_wait_on_a_loading_page_lasts_the_scaled_time() {
        let driver = MockWebDriver::start(json!({}), |request| {
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/execute/sync") => Reply::Value(json!("loading")),
                _ => Reply::Value(Value::Null),
            }
        });
        let mut sb = driver.base_case().await;
        sb.set_timeout(1).await.unwrap();
        sb.set_run_settings(RunSettings::default().with_timeout_multiplier(2.0));
        let started = Instant::now();
        let waited = tokio::time::timeout(Duration::from_secs(10), sb.wait_for_ready_state_complete())
            .await
            .expect("the wait ran past ten seconds: it ignores the scaled timeout");
        assert!(matches!(waited, Err(SeleniumBaseError::WaitTimeout(_))), "{waited:?}");
        assert!(
            started.elapsed() >= Duration::from_secs(2),
            "gave up after {:?}",
            started.elapsed()
        );
    }
}

