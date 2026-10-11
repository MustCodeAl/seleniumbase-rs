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
    use super::webdriver_support::{MockWebDriver, Reply};
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

    /// A driver that answers every command with `null`.
    fn quiet_driver() -> MockWebDriver {
        MockWebDriver::start(json!({}), |_| Reply::Value(serde_json::Value::Null))
    }

    /// The last script the driver was asked to run.
    fn last_script(driver: &MockWebDriver) -> String {
        driver
            .requests("/execute/sync")
            .last()
            .and_then(|request| request.body["script"].as_str().map(str::to_owned))
            .expect("a script was sent")
    }

    /// Checks that `script` is `skeleton` with exactly `literals` in its
    /// string literals, so nothing the caller passed became code.
    fn assert_inert(script: &str, skeleton: &str, literals: &[&str]) {
        let read = ReadScript::read(script).unwrap_or_else(|why| panic!("{why}: {script:?}"));
        assert_eq!(read.skeleton, skeleton, "script: {script:?}");
        assert_eq!(read.literals, literals, "script: {script:?}");
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
    async fn js_click_keeps_the_selector_inside_its_string() {
        let driver = quiet_driver();
        let mut sb = driver.base_case().await;
        for css in HOSTILE {
            sb.js_click(css).await.unwrap();
            assert_inert(
                &last_script(&driver),
                "document.querySelector(S).click();",
                &[css],
            );
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
}
