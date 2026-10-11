// JavaScript library injection helpers.

// jQuery, loaded from its own CDN by `BaseCase::activate_jquery`, and the
// Messenger files, which the pure CDP engine loads from the same places.
use crate::sb_cdp::messenger::{JQUERY_URL, MESSENGER_CSS_URL, MESSENGER_JS_URL};

const JQUERY_CONFIRM_CSS_URL: &str =
    "https://cdnjs.cloudflare.com/ajax/libs/jquery-confirm/3.3.4/jquery-confirm.min.css";
const JQUERY_CONFIRM_JS_URL: &str =
    "https://cdnjs.cloudflare.com/ajax/libs/jquery-confirm/3.3.4/jquery-confirm.min.js";

/// Longest CSS value `css_value` accepts.
const MAX_CSS_VALUE_LEN: usize = 64;

/// What a loaded resource is.
#[derive(Clone, Copy, Debug)]
enum PageResource {
    Script,
    Stylesheet,
}

/// An async-script body (for `execute_async_script`) that adds `url` to the
/// page and calls back with `true` once it has loaded, or `false` if the
/// browser could not load it.
///
/// If `already_loaded` (a JavaScript expression) is true, nothing is added and
/// the callback gets `true` at once, so that activating a library twice is
/// harmless.
fn load_resource_script(kind: PageResource, url: &str, already_loaded: Option<&str>) -> String {
    let setup = match kind {
        PageResource::Script => format!(
            "var el = document.createElement('script'); el.src = {};",
            quote(url)
        ),
        PageResource::Stylesheet => format!(
            "var el = document.createElement('link'); el.rel = 'stylesheet'; el.href = {};",
            quote(url)
        ),
    };
    let guard = already_loaded.map_or_else(String::new, |check| {
        format!("if ({check}) {{ done(true); return; }} ")
    });
    format!(
        "var done = arguments[arguments.length - 1]; {guard}{setup} \
         el.onload = function () {{ done(true); }}; \
         el.onerror = function () {{ done(false); }}; \
         (document.head || document.documentElement).appendChild(el);"
    )
}

/// `value` if it is safe to place inside a CSS declaration.
///
/// Letters, digits, spaces and `# % . , ( ) + - _` are enough for any colour
/// or length. Everything else, including `;`, `{`, `}`, quotes, `\` and `/`,
/// could end the declaration or open a new rule, so it is refused instead of
/// escaped.
fn css_value<'a>(what: &str, value: &'a str) -> Result<&'a str, SeleniumBaseError> {
    let allowed = |c: char| c.is_ascii_alphanumeric() || " #%.,()+-_".contains(c);
    if value.trim().is_empty() || value.len() > MAX_CSS_VALUE_LEN || !value.chars().all(allowed) {
        return Err(SeleniumBaseError::InvalidConfig(format!(
            "{what} must be a plain CSS value such as a colour (got {value:?})"
        )));
    }
    Ok(value.trim())
}

/// A script that stops the page from asking whether to leave it.
///
/// It clears `window.onbeforeunload` and makes it ignore later assignments,
/// and it drops `beforeunload` listeners added afterwards. Listeners the page
/// registered before the script ran stay in place.
const DISABLE_BEFOREUNLOAD_SCRIPT: &str = "\
    window.onbeforeunload = null; \
    try { Object.defineProperty(window, 'onbeforeunload', { \
        configurable: true, get: function () { return null; }, set: function () {} }); } \
    catch (e) {} \
    if (!window.__sbBeforeUnloadOff) { \
        window.__sbBeforeUnloadOff = true; \
        var addEventListener = window.addEventListener; \
        window.addEventListener = function (type) { \
            if (type === 'beforeunload') { return; } \
            return addEventListener.apply(this, arguments); }; } \
    return true;";

impl BaseCase {
    /// Loads `url` into the page and returns once the browser has loaded it.
    async fn load_page_resource(
        &self,
        kind: PageResource,
        url: &str,
        already_loaded: Option<&str>,
    ) -> Result<(), SeleniumBaseError> {
        let script = load_resource_script(kind, url, already_loaded);
        if self.execute_async_script(&script).await?.as_bool() == Some(true) {
            return Ok(());
        }
        Err(SeleniumBaseError::download(
            url,
            "the page could not load this resource",
        ))
    }

    /// Injects jQuery from a CDN into the current page.
    ///
    /// Returns once jQuery has loaded, and does nothing if the page already has
    /// it.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Download`] if the page cannot load jQuery,
    /// for example when it is offline.
    pub async fn activate_jquery(&self) -> Result<(), SeleniumBaseError> {
        self.load_page_resource(
            PageResource::Script,
            JQUERY_URL,
            Some("typeof window.jQuery !== 'undefined'"),
        )
        .await
    }

    /// Injects the jquery-confirm CSS and JS from a CDN, after jQuery, which it
    /// needs.
    ///
    /// Returns once everything has loaded.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Download`] if the page cannot load a part.
    pub async fn activate_jquery_confirm(&self) -> Result<(), SeleniumBaseError> {
        self.activate_jquery().await?;
        self.load_page_resource(PageResource::Stylesheet, JQUERY_CONFIRM_CSS_URL, None)
            .await?;
        self.load_page_resource(
            PageResource::Script,
            JQUERY_CONFIRM_JS_URL,
            Some("typeof window.jconfirm !== 'undefined'"),
        )
        .await
    }

    /// Injects the HubSpot Messenger CSS and JS from a CDN, after jQuery, which
    /// it needs.
    ///
    /// Returns once everything has loaded.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Download`] if the page cannot load a part.
    pub async fn activate_messenger(&self) -> Result<(), SeleniumBaseError> {
        self.activate_jquery().await?;
        self.load_page_resource(PageResource::Stylesheet, MESSENGER_CSS_URL, None)
            .await?;
        self.load_page_resource(
            PageResource::Script,
            MESSENGER_JS_URL,
            Some("typeof window.Messenger !== 'undefined'"),
        )
        .await
    }

    /// Sets the theme jquery-confirm dialogs use (`modern`, `material`,
    /// `bootstrap`, `supervan`, `light`, `dark`, `seamless`, ...).
    ///
    /// Call [`BaseCase::activate_jquery_confirm`] first.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] if `theme` is not a plain
    /// name, or if jquery-confirm is not loaded in the page.
    pub async fn set_jqc_theme(&self, theme: &str) -> Result<(), SeleniumBaseError> {
        self.set_jqc_default_theme(theme).await
    }

    /// Restores jquery-confirm's default theme (`light`).
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] if jquery-confirm is not
    /// loaded in the page.
    pub async fn reset_jqc_theme(&self) -> Result<(), SeleniumBaseError> {
        self.set_jqc_default_theme("light").await
    }

    async fn set_jqc_default_theme(&self, theme: &str) -> Result<(), SeleniumBaseError> {
        let valid = !theme.is_empty()
            && theme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        if !valid {
            return Err(SeleniumBaseError::InvalidConfig(format!(
                "jquery-confirm theme must be a plain name such as 'modern' (got {theme:?})"
            )));
        }
        let script = format!(
            "if (typeof window.jconfirm === 'undefined') {{ return false; }} \
             window.jconfirm.defaults = window.jconfirm.defaults || {{}}; \
             window.jconfirm.defaults.theme = {}; return true;",
            quote(theme)
        );
        if self.execute_script(&script).await?.as_bool() == Some(true) {
            return Ok(());
        }
        Err(SeleniumBaseError::InvalidConfig(
            "jquery-confirm is not loaded in this page; call activate_jquery_confirm first".to_owned(),
        ))
    }

    /// Injects a small CSS snippet to customize IntroJS highlight colors.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] if a colour is not a plain
    /// CSS value (for example `#336699` or `rgb(10, 20, 30)`); one that could
    /// end the declaration is refused rather than injected.
    pub async fn set_introjs_colors(&self, background: &str, border: &str) -> Result<(), SeleniumBaseError> {
        let css = format!(
            ".introjs-helperLayer {{ background: {} !important; border-color: {} !important; }}",
            css_value("the IntroJS background colour", background)?,
            css_value("the IntroJS border colour", border)?
        );
        self.add_css_style(&css).await
    }

    /// Recolors Messenger messages: sets their background to the CSS colour
    /// `color`.
    ///
    /// This is **not** Python's `set_messenger_theme(theme, location,
    /// max_messages)`, which picks one of Messenger's themes (`flat`,
    /// `future`, `block`, `air`, `ice`), where messages appear and how many
    /// show at once. The pure-CDP engine has that:
    /// [`Page::set_messenger_theme`](crate::sb_cdp::Page::set_messenger_theme).
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] if `color` is not a plain
    /// CSS value (for example `#336699` or `rgb(10, 20, 30)`).
    pub async fn set_messenger_theme(&self, color: &str) -> Result<(), SeleniumBaseError> {
        let css = format!(
            ".messenger-message {{ background: {} !important; }}",
            css_value("the Messenger colour", color)?
        );
        self.add_css_style(&css).await
    }

    /// Disables the `beforeunload` confirmation dialog.
    ///
    /// Clears `window.onbeforeunload`, makes it ignore later assignments, and
    /// drops `beforeunload` listeners the page adds from now on. Listeners
    /// that were already registered with `addEventListener` cannot be removed
    /// from outside, so they keep working.
    pub async fn disable_beforeunload(&self) -> Result<(), SeleniumBaseError> {
        self.execute_script(DISABLE_BEFOREUNLOAD_SCRIPT).await?;
        Ok(())
    }

    /// Removes the advertising elements of upstream SeleniumBase's
    /// `AD_BLOCK_LIST` (ad iframes and scripts from known ad networks, and
    /// elements Google's ad tags mark) from the current page. It removes what
    /// is in the page now; it blocks no later request. Corresponds to
    /// Python's `ad_block`.
    pub async fn ad_block(&self) -> Result<(), SeleniumBaseError> {
        self.execute_script(&page_scripts::ad_block_script()).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests_jslibs {
    use super::*;
    use super::chrome_support::{open, serve};

    #[test]
    fn css_value_accepts_colours_and_lengths() {
        for value in ["red", "#336699", "rgb(10, 20, 30)", "rgba(0,0,0,.5)", "hsl(120 50% 50%)", "12px", "-1.5em"] {
            assert_eq!(css_value("colour", value).unwrap(), value);
        }
        assert_eq!(css_value("colour", " blue ").unwrap(), "blue");
    }

    #[test]
    fn css_value_refuses_anything_that_could_escape_the_declaration() {
        let too_long = "a".repeat(MAX_CSS_VALUE_LEN + 1);
        for value in [
            "red; } body { display: none",
            "red}",
            "url(javascript:alert(1))",
            "red /* x */",
            "expression(alert(1))\\",
            "a\"b",
            "a'b",
            "<style>",
            "",
            "   ",
            too_long.as_str(),
        ] {
            assert!(
                matches!(css_value("colour", value), Err(SeleniumBaseError::InvalidConfig(_))),
                "{value:?} should be refused"
            );
        }
    }

    #[test]
    fn load_script_names_the_url_as_data_not_code() {
        let url = "https://x/a.js?q=\"'</script>";
        let script = load_resource_script(PageResource::Script, url, None);
        // The quote and the markup are escaped: the URL cannot end the literal or the element.
        assert!(script.contains(&format!("el.src = {};", quote(url))), "{script}");
        assert!(script.contains(r#"?q=\"'"#), "{script}");
        assert!(!script.contains("</script"), "{script}");
        let sheet = load_resource_script(PageResource::Stylesheet, "https://x/a.css", Some("window.a"));
        assert!(sheet.contains("el.rel = 'stylesheet'"), "{sheet}");
        assert!(sheet.starts_with("var done = arguments[arguments.length - 1]; if (window.a) { done(true); return; }"));
    }

    /// Runs `script` the way WebDriver's `executeAsyncScript` does, and returns what it
    /// passes to its callback.
    fn as_async_script(script: &str) -> String {
        format!("new Promise(function (resolve) {{ (function () {{ {script} }}).apply(null, [resolve]); }})")
    }

    #[tokio::test]
    #[ignore = "needs Chrome"]
    async fn a_resource_reports_when_it_has_loaded_and_when_it_has_not() {
        let base = serve(vec![
            ("/page", 200, String::new(), "<!doctype html><title>t</title>".to_owned()),
            ("/lib.js", 200, "Content-Type: text/javascript\r\n".to_owned(), "window.libLoaded = true;".to_owned()),
            ("/lib.css", 200, "Content-Type: text/css\r\n".to_owned(), "body{}".to_owned()),
        ]);
        let (browser, page) = open(&format!("{base}/page")).await;

        let loaded = load_resource_script(PageResource::Script, &format!("{base}/lib.js"), None);
        assert_eq!(page.evaluate(as_async_script(&loaded)).await.unwrap(), true);
        // Loaded means the code has run, not merely that the tag exists.
        assert_eq!(page.evaluate("window.libLoaded === true").await.unwrap(), true);

        let sheet = load_resource_script(PageResource::Stylesheet, &format!("{base}/lib.css"), None);
        assert_eq!(page.evaluate(as_async_script(&sheet)).await.unwrap(), true);

        let missing = load_resource_script(PageResource::Script, &format!("{base}/nope.js"), None);
        assert_eq!(page.evaluate(as_async_script(&missing)).await.unwrap(), false);

        let guarded = load_resource_script(PageResource::Script, &format!("{base}/nope.js"), Some("window.libLoaded"));
        assert_eq!(
            page.evaluate(as_async_script(&guarded)).await.unwrap(),
            true,
            "a library that is already there is not fetched again"
        );
        browser.close().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "needs Chrome"]
    async fn disabling_beforeunload_lets_the_page_navigate() {
        let other = serve(vec![("/", 200, String::new(), "<!doctype html><title>other</title>".to_owned())]);
        let base = serve(vec![(
            "/",
            200,
            String::new(),
            "<!doctype html><button id=b>x</button><script>\
             window.onbeforeunload = function (e) { e.preventDefault(); return 'sure?'; };\
             </script>"
                .to_owned(),
        )]);
        let (browser, page) = open(&format!("{base}/")).await;
        // Chrome only asks when the user has interacted with the page.
        page.locator("#b").click().await.unwrap();
        page.evaluate(format!("(function () {{ {DISABLE_BEFOREUNLOAD_SCRIPT} }})()")).await.unwrap();
        // A handler the page installs afterwards is dropped as well.
        page.evaluate("window.addEventListener('beforeunload', function (e) { e.preventDefault(); e.returnValue = ''; }); 1")
            .await
            .unwrap();
        page.evaluate("window.onbeforeunload = function () { return 'again'; }; 1").await.unwrap();

        let page = page.with_timeout(std::time::Duration::from_secs(5));
        page.goto(format!("{other}/")).await.expect("no beforeunload prompt blocks the navigation");
        assert_eq!(page.evaluate("document.title").await.unwrap(), "other");
        browser.close().await.unwrap();
    }
}
