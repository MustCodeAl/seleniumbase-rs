// JavaScript library injection helpers.

// jQuery, loaded from its own CDN by `BaseCase::activate_jquery`, and the
// scripts and settings of Messenger, which the pure CDP engine uses too. The
// messages `post_message` and its relatives show are built from them.
use crate::api::messages::{fallback_line, PostMessage};
use crate::sb_cdp::messenger::{
    activate_script, post_markup_script, theme_script, MessageOptions, MessageStyle,
    MessengerLocation, MessengerSettings, JQUERY_URL,
};

// The jquery-confirm dialogs of `get_jqc_*`; `JqcDefaults` is imported with
// the case that holds it.
use crate::api::jqc::{self, Answer, JqcButton, JqcButtonInput, JqcFormInputs, JqcTextInput};

/// How often a dialog is asked whether it is still open (Python: 0.05 s).
const JQC_POLL: Duration = Duration::from_millis(50);

/// How long a dialog is given to appear before its box is focused (Python:
/// 0.02 s).
const JQC_SETTLE: Duration = Duration::from_millis(20);


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

    /// Loads Messenger and what it needs into the page, as Python's
    /// `activate_messenger` does: jQuery, Underscore, the Messenger script, the
    /// style sheets of its five themes and its spinner, SeleniumBase's own
    /// style, and the scripts of the flat and future themes. Then it starts
    /// Messenger with Python's options: the future theme at the bottom right,
    /// eight messages at most.
    ///
    /// Something the page already has is not loaded again, so calling it twice
    /// is harmless (the options are set again, though). It returns once
    /// everything has loaded. A navigation drops Messenger with the page.
    ///
    /// # Errors
    ///
    /// Returns an error if the page cannot load one of the files, for example
    /// when it is offline or its policy blocks scripts from the CDN.
    pub async fn activate_messenger(&self) -> Result<(), SeleniumBaseError> {
        self.execute_script(&format!("return {};", activate_script()))
            .await?;
        Ok(())
    }

    /// Sets the theme, and the border colour and the width when they are
    /// given, that the next jquery-confirm dialogs use, as Python's
    /// `set_jqc_theme(theme, color=None, width=None)` does.
    ///
    /// The themes are `bootstrap`, `modern`, `material`, `supervan`, `light`,
    /// `dark` and `seamless`; the colours (of the dialog's border, not of its
    /// buttons) are `blue`, `default`, `green`, `red`, `purple`, `orange` and
    /// `dark`, in any case. The width is a percent or pixels: `"36.0%"` or
    /// `"450px"`, or a whole number, which is a percent up to 100 and pixels
    /// above. Nothing is written into the page: the dialogs of
    /// [`get_jqc_button_input`](Self::get_jqc_button_input),
    /// [`get_jqc_text_input`](Self::get_jqc_text_input) and
    /// [`get_jqc_form_inputs`](Self::get_jqc_form_inputs) are built from these
    /// settings, and a call's own `options` win for that call.
    ///
    /// The settings belong to this case, where Python keeps them for the whole
    /// process. They are all or nothing: a bad part changes none of them.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`], with Python's message, if
    /// the theme, the colour or the width is not valid.
    pub fn set_jqc_theme(
        &mut self,
        theme: &str,
        color: Option<&str>,
        width: Option<&str>,
    ) -> Result<(), SeleniumBaseError> {
        self.jqc = self.jqc.changed(theme, color, width)?;
        Ok(())
    }

    /// Resets the jquery-confirm theme, border colour and width to the factory
    /// defaults: `bootstrap`, `blue` and `38%` ([`JqcDefaults::default`]).
    /// Python's `reset_jqc_theme` never does this in 4.55.2; here it does.
    pub fn reset_jqc_theme(&mut self) {
        self.jqc = JqcDefaults::default();
    }

    /// The theme, border colour and width the next jquery-confirm dialogs use.
    #[must_use]
    pub fn jqc_defaults(&self) -> &JqcDefaults {
        &self.jqc
    }

    /// Pops up a jquery-confirm box with `buttons` and returns the text of the
    /// button that was clicked, as Python's `get_jqc_button_input(message,
    /// buttons, options=None)` does.
    ///
    /// `message` is HTML. A button is a text, a text and a colour name
    /// (`("Yes!", "green")`) or a [`JqcButton`]; the colours are `blue`,
    /// `green`, `red`, `orange`, `purple`, `default` and `dark`. With one
    /// button Enter, `y` and `1` click it; with several, `y` clicks `Yes`, `n`
    /// clicks `No` and the number keys click the buttons. The call waits for
    /// the person for as long as it takes, unless
    /// [`set_time_limit`](Self::set_time_limit) ends the test first. Python's
    /// `options` is the builder method `.options([("theme", "modern"),
    /// ("width", "450px")])`.
    ///
    /// In a headless browser, or on a virtual display, no dialog opens and the
    /// text of the last button comes back at once.
    ///
    /// # Errors
    ///
    /// Awaiting the call returns [`SeleniumBaseError::InvalidConfig`] if there
    /// is no button, a colour or an option is not valid, or the dialog closed
    /// without an answer because the page went away, and
    /// [`SeleniumBaseError::Download`] if the page cannot load jquery-confirm.
    pub fn get_jqc_button_input<'a, B: Into<JqcButton>>(
        &'a self,
        message: &'a str,
        buttons: impl IntoIterator<Item = B>,
    ) -> JqcButtonInput<'a> {
        let buttons = buttons.into_iter().map(Into::into).collect();
        JqcButtonInput::new(self, message, buttons, self.jqc.clone())
    }

    /// Pops up a jquery-confirm box with one input and returns the text
    /// submitted, as Python's `get_jqc_text_input(message, button=None,
    /// options=None)` does. Enter submits.
    ///
    /// Python's `button` is the builder method `.button(...)`, a text or a text
    /// and a colour; the default is a blue `Submit`. Python's `options` is
    /// `.options(...)`.
    ///
    /// In a headless browser, or on a virtual display, no dialog opens and `""`
    /// comes back at once.
    ///
    /// # Errors
    ///
    /// As for [`get_jqc_button_input`](Self::get_jqc_button_input), except that
    /// having no button is not an error.
    pub fn get_jqc_text_input<'a>(&'a self, message: &'a str) -> JqcTextInput<'a> {
        JqcTextInput::new(self, message, self.jqc.clone())
    }

    /// Pops up a jquery-confirm box with an input and buttons and returns the
    /// text typed and the text of the button clicked, as Python's
    /// `get_jqc_form_inputs(message, buttons, options=None)` does. With one
    /// button, Enter clicks it.
    ///
    /// In a headless browser, or on a virtual display, no dialog opens and
    /// `("", last button text)` comes back at once.
    ///
    /// # Errors
    ///
    /// As for [`get_jqc_button_input`](Self::get_jqc_button_input).
    pub fn get_jqc_form_inputs<'a, B: Into<JqcButton>>(
        &'a self,
        message: &'a str,
        buttons: impl IntoIterator<Item = B>,
    ) -> JqcFormInputs<'a> {
        let buttons = buttons.into_iter().map(Into::into).collect();
        JqcFormInputs::new(self, message, buttons, self.jqc.clone())
    }

    /// Opens the dialog `script` builds, focuses `focus`, and waits for the
    /// person to answer it.
    ///
    /// It polls every 50 ms whether a dialog is still open and has no time
    /// limit of its own, as in Python; the test's
    /// [time limit](Self::set_time_limit) ends the wait. When the dialog is gone
    /// it reads what the dialog stored.
    pub(crate) async fn run_jqc_dialog(
        &self,
        script: &str,
        focus: &str,
    ) -> Result<Answer, SeleniumBaseError> {
        self.activate_jquery_confirm().await?;
        self.execute_script(script).await?;
        tokio::time::sleep(JQC_SETTLE).await;
        // Python ignores a failure to focus: the box may be gone already.
        let _ = self.execute_script(focus).await;
        loop {
            tokio::time::sleep(JQC_POLL).await;
            self.check_time_limit()?;
            match self.execute_script(jqc::OPEN_COUNT_SCRIPT).await?.as_i64() {
                Some(0) => break,
                // jquery-confirm is gone: the page navigated.
                Some(open) if open < 0 => return Err(jqc::closed_without_answer()),
                _ => {}
            }
        }
        Answer::from_value(&self.execute_script(jqc::ANSWER_SCRIPT).await?)
    }


    /// Sets how Messenger draws its messages and where they appear, as
    /// Python's `set_messenger_theme(theme, location, max_messages)` does:
    /// one of the themes (`flat`, `future`, `block`, `air`, `ice`), one of six
    /// places on the screen and how many messages show at once. `settings` is
    /// a [`MessengerSettings`], or a [`MessengerTheme`] or a
    /// [`MessengerLocation`] alone, which keeps Python's defaults (future, bottom
    /// right, eight messages) for the rest. It loads Messenger first, through
    /// [`activate_messenger`](Self::activate_messenger), when the page lacks it.
    ///
    /// Messages posted afterwards with [`post_message`](Self::post_message) and
    /// its relatives, and with Messenger itself, follow the settings. To
    /// recolour Messenger's messages instead, see
    /// [`set_messenger_color`](Self::set_messenger_color).
    ///
    /// [`MessengerTheme`]: crate::sb_cdp::MessengerTheme
    /// [`MessengerLocation`]: crate::sb_cdp::MessengerLocation
    ///
    /// # Errors
    ///
    /// Returns an error if the page cannot load Messenger, and
    /// [`SeleniumBaseError::InvalidConfig`] if it is still not there
    /// afterwards.
    pub async fn set_messenger_theme(
        &self,
        settings: impl Into<MessengerSettings>,
    ) -> Result<(), SeleniumBaseError> {
        self.activate_messenger().await?;
        self.apply_messenger_settings(settings.into()).await
    }

    /// Gives the Messenger in the page `settings`, which must have been
    /// loaded.
    async fn apply_messenger_settings(
        &self,
        settings: MessengerSettings,
    ) -> Result<(), SeleniumBaseError> {
        let script = format!("return {};", theme_script(settings));
        if self.execute_script(&script).await?.as_bool() == Some(true) {
            return Ok(());
        }
        Err(SeleniumBaseError::InvalidConfig(
            "Messenger is not loaded in this page, so its theme cannot be set".to_owned(),
        ))
    }

    /// Recolors Messenger messages: sets their background to the CSS colour
    /// `color`. This is what `set_messenger_theme(color)` did before that
    /// method took Python's arguments.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] if `color` is not a plain
    /// CSS value (for example `#336699` or `rgb(10, 20, 30)`).
    pub async fn set_messenger_color(&self, color: &str) -> Result<(), SeleniumBaseError> {
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
