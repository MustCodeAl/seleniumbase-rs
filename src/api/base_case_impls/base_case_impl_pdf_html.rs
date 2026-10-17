// Additional BaseCase methods for PDF, HTML parsing, and themed tours.

/// Browser log messages from these libraries are not page errors, so
/// `assert_no_js_errors` ignores them (Python SeleniumBase does the same).
const IGNORED_ERROR_SOURCES: [&str; 2] = [
    "//cdnjs.cloudflare.com/ajax/libs/messenger",
    "//cdnjs.cloudflare.com/ajax/libs/underscore",
];

/// The `SEVERE` browser log entries that count as JavaScript errors.
fn severe_js_errors(entries: &[thirtyfour::BrowserLogEntry]) -> Vec<&thirtyfour::BrowserLogEntry> {
    entries
        .iter()
        .filter(|entry| entry.level == "SEVERE")
        .filter(|entry| {
            !IGNORED_ERROR_SOURCES
                .iter()
                .any(|source| entry.message.contains(source))
        })
        .collect()
}

impl BaseCase {
    /// Captures the current page as a PDF using CDP `Page.printToPDF`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Unsupported`] if the session has no CDP
    /// connection or the response has no PDF data, and an I/O error if the file
    /// cannot be written.
    pub async fn print_to_pdf(&self, filename: &str) -> Result<(), SeleniumBaseError> {
        let response = self
            .session
            .execute_cdp_with_params(
                "Page.printToPDF",
                serde_json::json!({
                    "printBackground": true,
                    "preferCSSPageSize": true,
                }),
            )
            .await?;

        let data = response
            .get("data")
            .and_then(|v| v.as_str())
            .ok_or_else(|| SeleniumBaseError::Unsupported("PDF data missing from CDP response".to_owned()))?;

        let bytes = BASE64_STANDARD
            .decode(data)
            .map_err(|e| SeleniumBaseError::Unsupported(format!("Failed to decode PDF: {e}")))?;

        tokio::fs::write(filename, bytes).await?;
        Ok(())
    }

    /// Alias for `print_to_pdf`.
    pub async fn save_as_pdf(&self, filename: &str) -> Result<(), SeleniumBaseError> {
        self.print_to_pdf(filename).await
    }

    /// Extracts text from a PDF file on disk.
    ///
    /// Needs the `pdf` Cargo feature.
    #[cfg(feature = "pdf")]
    pub fn get_pdf_text(&self, filename: &str) -> Result<String, SeleniumBaseError> {
        pdf::extract_text_from_file(filename)
    }

    /// Asserts that `text` appears inside the given PDF.
    ///
    /// Needs the `pdf` Cargo feature.
    #[cfg(feature = "pdf")]
    pub fn assert_pdf_text(&self, filename: &str, text: &str) -> Result<(), SeleniumBaseError> {
        let content = self.get_pdf_text(filename)?;
        if !content.contains(text) {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "PDF '{}' did not contain text '{}'",
                filename, text
            )));
        }
        Ok(())
    }

    /// Returns a BeautifulSoup-style parser for the current page source.
    pub async fn get_beautiful_soup_object(&self) -> Result<BeautifulSoup, SeleniumBaseError> {
        let source = self.get_page_source().await?;
        Ok(BeautifulSoup::parse(&source))
    }

    /// Creates a tour and assigns a visual theme.
    pub async fn create_tour_with_theme(
        &mut self,
        name: &str,
        theme: TourTheme,
    ) -> Result<(), SeleniumBaseError> {
        self.tour = Some(crate::api::tour::Tour::new(name).with_theme(theme));
        Ok(())
    }

    /// Full-screens the browser window.
    pub async fn fullscreen_window(&self) -> Result<(), SeleniumBaseError> {
        self.session.driver().fullscreen_window().await?;
        Ok(())
    }

    /// Returns the screen width via JavaScript.
    pub async fn get_screen_width(&self) -> Result<i64, SeleniumBaseError> {
        let value = self.execute_script("return window.screen.width;").await?;
        value
            .as_i64()
            .ok_or_else(|| SeleniumBaseError::Unsupported("screen width not a number".to_owned()))
    }

    /// Returns the screen height via JavaScript.
    pub async fn get_screen_height(&self) -> Result<i64, SeleniumBaseError> {
        let value = self.execute_script("return window.screen.height;").await?;
        value
            .as_i64()
            .ok_or_else(|| SeleniumBaseError::Unsupported("screen height not a number".to_owned()))
    }

    /// Returns the locale the browser was configured with, if one was.
    ///
    /// `None` means no locale was set, so the browser uses its own default.
    /// Use [`BaseCase::get_locale_code`] to ask the browser what it is using.
    pub fn configured_locale(&self) -> Option<&str> {
        self.config.locale.as_deref()
    }

    /// Returns the configured browser locale, or `en-US` when none was
    /// configured.
    ///
    /// The `en-US` fallback is a guess, not what the browser reports: a browser
    /// started without a locale uses the operating system's. Call
    /// [`BaseCase::configured_locale`] to tell the two apart, or
    /// [`BaseCase::get_locale_code`] for the locale the browser is using.
    pub fn get_locale(&self) -> String {
        self.configured_locale().unwrap_or("en-US").to_owned()
    }

    /// Asserts that the browser reported no severe JavaScript errors.
    ///
    /// This reads (and so clears) the browser's log, like Python SeleniumBase's
    /// `assert_no_js_errors`, and counts only entries of level `SEVERE`:
    /// uncaught exceptions, failed resource loads and `console.error`. A message
    /// that merely contains the word "error" is not one.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::AssertionFailed`] listing the errors, or a
    /// WebDriver error if the browser has no log to read (only Chromium-based
    /// browsers do).
    pub async fn assert_no_js_errors(&self) -> Result<(), SeleniumBaseError> {
        let entries = self.session.driver().browser_log().await?;
        let errors = severe_js_errors(&entries);
        if errors.is_empty() {
            return Ok(());
        }
        let listed = errors
            .iter()
            .map(|entry| entry.message.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let url = match self.session.driver().current_url().await {
            Ok(url) => url.to_string(),
            Err(_) => "the page".to_owned(),
        };
        Err(SeleniumBaseError::AssertionFailed(format!(
            "JavaScript errors found on {url} =>\n{listed}"
        )))
    }

    /// Converts a CSS selector to an equivalent XPath expression.
    ///
    /// Supports type, `*`, `#id`, `.class`, attribute selectors (`[a]`, `[a=v]`
    /// and the `~=`, `|=`, `^=`, `$=`, `*=` forms), the descendant, `>`, `+` and
    /// `~` combinators, comma-separated lists, and the pseudo-classes
    /// `:first-child`, `:last-child`, `:only-child`, `:first-of-type`,
    /// `:last-of-type`, `:nth-child(n)`, `:nth-of-type(n)` (a number, `odd` or
    /// `even`), `:empty` and `:contains("text")`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidSelector`] for a selector it cannot
    /// convert, rather than an XPath expression that is not valid.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # fn demo(sb: &seleniumbase_rs::BaseCase) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    /// assert_eq!(sb.convert_css_to_xpath("form > input#user")?, "//form/input[@id='user']");
    /// # Ok(())
    /// # }
    /// ```
    pub fn convert_css_to_xpath(&self, css: &str) -> Result<String, SeleniumBaseError> {
        css_to_xpath(css)
    }
}

// -------------------------------------------------------------------------
// CSS selector -> XPath
// -------------------------------------------------------------------------

/// `s` as an XPath string literal, quoted so that any content is safe.
fn xpath_literal(s: &str) -> String {
    if !s.contains('\'') {
        format!("'{s}'")
    } else if !s.contains('"') {
        format!("\"{s}\"")
    } else {
        let parts: Vec<String> = s.split('\'').map(|part| format!("'{part}'")).collect();
        format!("concat({})", parts.join(", \"'\", "))
    }
}

/// How a CSS combinator relates a compound selector to the one before it.
#[derive(Clone, Copy, Debug, PartialEq)]
enum CssCombinator {
    Descendant,
    Child,
    NextSibling,
    LaterSibling,
}

/// One compound selector (`tag#id.class[attr]:pseudo`) as an XPath step.
struct CssStep {
    /// Element name, or `*`.
    name: String,
    /// XPath predicates the element must satisfy.
    predicates: Vec<String>,
}

impl CssStep {
    fn render(&self) -> String {
        self.predicates
            .iter()
            .fold(self.name.clone(), |step, predicate| format!("{step}[{predicate}]"))
    }
}

/// Reads one CSS selector list from a string.
struct CssParser<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
    source: &'a str,
}

impl<'a> CssParser<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            chars: source.chars().peekable(),
            source,
        }
    }

    fn error(&self, reason: &str) -> SeleniumBaseError {
        SeleniumBaseError::InvalidSelector(format!(
            "cannot convert CSS selector '{}' to XPath: {reason}",
            self.source
        ))
    }

    fn skip_whitespace(&mut self) -> bool {
        let mut skipped = false;
        while self.chars.next_if(|c| c.is_whitespace()).is_some() {
            skipped = true;
        }
        skipped
    }

    fn is_ident_char(c: char) -> bool {
        c.is_alphanumeric() || c == '-' || c == '_' || !c.is_ascii()
    }

    /// An identifier; a backslash makes the next character literal.
    fn ident(&mut self) -> String {
        let mut ident = String::new();
        loop {
            match self.chars.peek() {
                Some('\\') => {
                    self.chars.next();
                    if let Some(c) = self.chars.next() {
                        ident.push(c);
                    }
                }
                Some(&c) if Self::is_ident_char(c) => {
                    ident.push(c);
                    self.chars.next();
                }
                _ => return ident,
            }
        }
    }

    /// A quoted string or a bare identifier or number.
    fn value(&mut self) -> Result<String, SeleniumBaseError> {
        match self.chars.peek().copied() {
            Some(quote @ ('"' | '\'')) => {
                self.chars.next();
                let mut value = String::new();
                loop {
                    match self.chars.next() {
                        Some(c) if c == quote => return Ok(value),
                        Some('\\') => value.extend(self.chars.next()),
                        Some(c) => value.push(c),
                        None => return Err(self.error("unterminated string")),
                    }
                }
            }
            _ => {
                let value = self.ident();
                if value.is_empty() {
                    Err(self.error("expected a value"))
                } else {
                    Ok(value)
                }
            }
        }
    }

    fn expect(&mut self, expected: char) -> Result<(), SeleniumBaseError> {
        self.skip_whitespace();
        match self.chars.next() {
            Some(c) if c == expected => Ok(()),
            _ => Err(self.error(&format!("expected '{expected}'"))),
        }
    }

    /// `[name]`, `[name=value]` and the other attribute forms.
    fn attribute(&mut self) -> Result<String, SeleniumBaseError> {
        self.skip_whitespace();
        let name = self.ident();
        if name.is_empty() {
            return Err(self.error("expected an attribute name"));
        }
        self.skip_whitespace();
        let operator = match self.chars.next() {
            Some(']') => return Ok(format!("@{name}")),
            Some('=') => "=".to_owned(),
            Some(c @ ('~' | '|' | '^' | '$' | '*')) => {
                if self.chars.next() != Some('=') {
                    return Err(self.error("expected '=' in an attribute selector"));
                }
                format!("{c}=")
            }
            _ => return Err(self.error("malformed attribute selector")),
        };
        self.skip_whitespace();
        let value = self.value()?;
        self.skip_whitespace();
        self.expect(']')?;
        let attr = format!("@{name}");
        let literal = xpath_literal(&value);
        let empty_or_blank = value.is_empty();
        Ok(match operator.as_str() {
            "=" => format!("{attr}={literal}"),
            // The value of `~=` is a whole word, so it cannot be empty or hold a space.
            "~=" if empty_or_blank || value.contains(char::is_whitespace) => "false()".to_owned(),
            "~=" => format!(
                "contains(concat(' ', normalize-space({attr}), ' '), {})",
                xpath_literal(&format!(" {value} "))
            ),
            "|=" => format!(
                "({attr}={literal} or starts-with({attr}, {}))",
                xpath_literal(&format!("{value}-"))
            ),
            // These three match nothing when the value is empty.
            "^=" | "$=" | "*=" if empty_or_blank => "false()".to_owned(),
            "^=" => format!("starts-with({attr}, {literal})"),
            "$=" => format!(
                "substring({attr}, string-length({attr}) - string-length({literal}) + 1) = {literal}"
            ),
            _ => format!("contains({attr}, {literal})"),
        })
    }

    /// `:pseudo` and `:pseudo(argument)`.
    fn pseudo(&mut self, name: &str) -> Result<Vec<String>, SeleniumBaseError> {
        let pseudo = self.ident().to_ascii_lowercase();
        let argument = if self.chars.next_if_eq(&'(').is_some() {
            self.skip_whitespace();
            let argument = if matches!(self.chars.peek(), Some('"' | '\'')) {
                self.value()?
            } else {
                let mut raw = String::new();
                while let Some(c) = self.chars.next_if(|c| *c != ')') {
                    raw.push(c);
                }
                raw.trim().to_owned()
            };
            self.expect(')')?;
            Some(argument)
        } else {
            None
        };
        let same_name = |preceding: bool| -> Result<String, SeleniumBaseError> {
            if name == "*" {
                return Err(self.error(&format!(":{pseudo} needs an element name")));
            }
            let axis = if preceding { "preceding" } else { "following" };
            Ok(format!("not({axis}-sibling::{name})"))
        };
        let position = |argument: &str, counted: &str| -> Result<String, SeleniumBaseError> {
            let before = format!("count(preceding-sibling::{counted})");
            match argument.to_ascii_lowercase().as_str() {
                "odd" => Ok(format!("{before} mod 2 = 0")),
                "even" => Ok(format!("{before} mod 2 = 1")),
                number => match number.parse::<u32>() {
                    Ok(n) if n >= 1 => Ok(format!("{before} = {}", n - 1)),
                    _ => Err(self.error(&format!("unsupported position '{argument}'"))),
                },
            }
        };
        let predicate = match (pseudo.as_str(), argument.as_deref()) {
            ("first-child", None) => "not(preceding-sibling::*)".to_owned(),
            ("last-child", None) => "not(following-sibling::*)".to_owned(),
            ("only-child", None) => "not(preceding-sibling::*) and not(following-sibling::*)".to_owned(),
            ("first-of-type", None) => same_name(true)?,
            ("last-of-type", None) => same_name(false)?,
            ("empty", None) => "not(node())".to_owned(),
            ("nth-child", Some(n)) => position(n, "*")?,
            ("nth-of-type", Some(n)) if name != "*" => position(n, name)?,
            ("contains", Some(text)) => format!("contains(., {})", xpath_literal(text)),
            _ => return Err(self.error(&format!("unsupported pseudo-class ':{pseudo}'"))),
        };
        Ok(vec![predicate])
    }

    /// One compound selector.
    fn compound(&mut self) -> Result<CssStep, SeleniumBaseError> {
        let (name, named) = if self.chars.next_if_eq(&'*').is_some() {
            ("*".to_owned(), true)
        } else {
            let tag = self.ident().to_ascii_lowercase();
            if tag.is_empty() {
                ("*".to_owned(), false)
            } else {
                (tag, true)
            }
        };
        let mut predicates = Vec::new();
        loop {
            match self.chars.peek().copied() {
                Some('#') => {
                    self.chars.next();
                    let id = self.ident();
                    if id.is_empty() {
                        return Err(self.error("expected an id after '#'"));
                    }
                    predicates.push(format!("@id={}", xpath_literal(&id)));
                }
                Some('.') => {
                    self.chars.next();
                    let class = self.ident();
                    if class.is_empty() {
                        return Err(self.error("expected a class after '.'"));
                    }
                    predicates.push(format!(
                        "contains(concat(' ', normalize-space(@class), ' '), {})",
                        xpath_literal(&format!(" {class} "))
                    ));
                }
                Some('[') => {
                    self.chars.next();
                    predicates.push(self.attribute()?);
                }
                Some(':') => {
                    self.chars.next();
                    predicates.extend(self.pseudo(&name)?);
                }
                _ => break,
            }
        }
        if !named && predicates.is_empty() {
            return Err(self.error("expected a selector"));
        }
        Ok(CssStep { name, predicates })
    }

    /// One complex selector, as an absolute XPath path.
    fn complex(&mut self) -> Result<String, SeleniumBaseError> {
        let mut path = String::new();
        let mut combinator = CssCombinator::Descendant;
        loop {
            let step = self.compound()?;
            path.push_str(&match combinator {
                CssCombinator::Descendant => format!("//{}", step.render()),
                CssCombinator::Child => format!("/{}", step.render()),
                CssCombinator::NextSibling => {
                    let tail = CssStep {
                        name: "*".to_owned(),
                        predicates: Vec::new(),
                    };
                    let own = CssStep {
                        name: format!("self::{}", step.name),
                        predicates: step.predicates,
                    };
                    format!("/following-sibling::{}[1]/{}", tail.render(), own.render())
                }
                CssCombinator::LaterSibling => format!("/following-sibling::{}", step.render()),
            });
            let spaced = self.skip_whitespace();
            combinator = match self.chars.peek().copied() {
                None | Some(',') => return Ok(path),
                Some('>') => CssCombinator::Child,
                Some('+') => CssCombinator::NextSibling,
                Some('~') => CssCombinator::LaterSibling,
                Some(_) if spaced => CssCombinator::Descendant,
                Some(c) => return Err(self.error(&format!("unexpected '{c}'"))),
            };
            if combinator != CssCombinator::Descendant {
                self.chars.next();
                self.skip_whitespace();
            }
        }
    }

    /// A comma-separated list of complex selectors.
    fn list(&mut self) -> Result<String, SeleniumBaseError> {
        let mut paths = Vec::new();
        self.skip_whitespace();
        loop {
            paths.push(self.complex()?);
            if self.chars.next_if_eq(&',').is_none() {
                return Ok(paths.join(" | "));
            }
            self.skip_whitespace();
        }
    }
}

/// Converts the CSS selector `css` to an XPath expression.
pub(crate) fn css_to_xpath(css: &str) -> Result<String, SeleniumBaseError> {
    if css.trim().is_empty() {
        return Err(SeleniumBaseError::InvalidSelector(
            "cannot convert an empty CSS selector to XPath".to_owned(),
        ));
    }
    CssParser::new(css).list()
}

#[cfg(test)]
mod tests_pdf_html {
    use super::*;

    fn xpath(css: &str) -> String {
        css_to_xpath(css).unwrap_or_else(|e| panic!("{css}: {e}"))
    }

    #[test]
    fn xpath_literal_quotes_whatever_it_is_given() {
        assert_eq!(xpath_literal("plain"), "'plain'");
        assert_eq!(xpath_literal("it's"), "\"it's\"");
        assert_eq!(xpath_literal(r#"a'b"c"#), r#"concat('a', "'", 'b"c')"#);
        // Not HTML-escaped: `&` is just a character in an XPath string.
        assert_eq!(xpath_literal("a&b"), "'a&b'");
    }

    #[test]
    fn simple_selectors() {
        assert_eq!(xpath("div"), "//div");
        assert_eq!(xpath("*"), "//*");
        assert_eq!(xpath("#main"), "//*[@id='main']");
        assert_eq!(xpath("input#user"), "//input[@id='user']");
        assert_eq!(
            xpath("a.nav.big"),
            "//a[contains(concat(' ', normalize-space(@class), ' '), ' nav ')]\
             [contains(concat(' ', normalize-space(@class), ' '), ' big ')]"
        );
        assert_eq!(xpath("DIV"), "//div", "type selectors are case-insensitive in HTML");
    }

    #[test]
    fn combinators_become_path_steps() {
        assert_eq!(xpath("div a"), "//div//a");
        assert_eq!(xpath("div > a"), "//div/a");
        assert_eq!(xpath("form>input"), "//form/input");
        assert_eq!(xpath("h1 + p"), "//h1/following-sibling::*[1]/self::p");
        assert_eq!(xpath("h1 ~ p"), "//h1/following-sibling::p");
        assert_eq!(xpath("ul, ol"), "//ul | //ol");
    }

    #[test]
    fn attribute_selectors() {
        assert_eq!(xpath("[disabled]"), "//*[@disabled]");
        assert_eq!(xpath("input[name='q']"), "//input[@name='q']");
        assert_eq!(xpath("input[name=q]"), "//input[@name='q']");
        assert_eq!(xpath(r#"a[title="it's"]"#), r#"//a[@title="it's"]"#);
        assert_eq!(xpath("a[href^='/x']"), "//a[starts-with(@href, '/x')]");
        assert_eq!(xpath("a[href*=x]"), "//a[contains(@href, 'x')]");
        assert_eq!(xpath("a[href^='']"), "//a[false()]");
        assert_eq!(xpath("a[rel~='nofollow']"), "//a[contains(concat(' ', normalize-space(@rel), ' '), ' nofollow ')]");
        assert_eq!(
            xpath("a[lang|=en]"),
            "//a[(@lang='en' or starts-with(@lang, 'en-'))]"
        );
    }

    #[test]
    fn pseudo_classes() {
        assert_eq!(xpath("li:first-child"), "//li[not(preceding-sibling::*)]");
        assert_eq!(xpath("li:last-child"), "//li[not(following-sibling::*)]");
        assert_eq!(xpath("li:nth-child(2)"), "//li[count(preceding-sibling::*) = 1]");
        assert_eq!(xpath("li:nth-of-type(odd)"), "//li[count(preceding-sibling::li) mod 2 = 0]");
        assert_eq!(xpath("li:first-of-type"), "//li[not(preceding-sibling::li)]");
        assert_eq!(xpath("button:contains(\"Go on\")"), "//button[contains(., 'Go on')]");
        assert_eq!(xpath("td:empty"), "//td[not(node())]");
    }

    #[test]
    fn escaped_identifiers_are_taken_literally() {
        assert_eq!(xpath(r"#a\:b"), "//*[@id='a:b']");
    }

    #[test]
    fn what_cannot_be_converted_is_an_error_not_a_broken_xpath() {
        for css in ["", "   ", "a:hover", "a::before", "li:nth-child(2n+1)", "*:first-of-type", "[", "a[b=", "div >", "a[b!=c]", "#", ".", "a,"] {
            let result = css_to_xpath(css);
            assert!(
                matches!(result, Err(SeleniumBaseError::InvalidSelector(_))),
                "{css:?} should be rejected, got {result:?}"
            );
        }
    }

    fn entry(level: &str, message: &str) -> thirtyfour::BrowserLogEntry {
        serde_json::from_value(serde_json::json!({
            "level": level, "message": message, "timestamp": 0
        }))
        .unwrap()
    }

    #[test]
    fn only_severe_entries_are_js_errors() {
        let entries = vec![
            entry("INFO", "0 errors found"),
            entry("WARNING", "error-prone but only a warning"),
            entry("SEVERE", "https://x/app.js 1:1 Uncaught TypeError: x is not a function"),
            entry("SEVERE", "https://cdnjs.cloudflare.com/ajax/libs/messenger/1.5.0/js/m.js"),
            entry("SEVERE", "see //cdnjs.cloudflare.com/ajax/libs/underscore/1.9.1/u.js"),
        ];
        let errors = severe_js_errors(&entries);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("Uncaught TypeError"));
    }

    #[test]
    fn an_unconfigured_locale_is_distinguishable_from_the_fallback() {
        let config = BrowserConfig {
            locale: None,
            ..BrowserConfig::default()
        };
        let sb = BaseCase::with_session(config.clone(), BrowserSession::disconnected());
        assert_eq!(sb.configured_locale(), None);
        assert_eq!(sb.get_locale(), "en-US");
        let config = BrowserConfig {
            locale: Some("de-DE".to_owned()),
            ..config
        };
        let sb = BaseCase::with_session(config, BrowserSession::disconnected());
        assert_eq!(sb.configured_locale(), Some("de-DE"));
        assert_eq!(sb.get_locale(), "de-DE");
    }
}
