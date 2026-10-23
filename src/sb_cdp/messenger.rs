//! Messages shown inside the page, in the style of the Messenger library.
//!
//! Python SeleniumBase's CDP Mode has `post_message`, `activate_messenger` and
//! `set_messenger_theme`. They load the [Messenger] library into the page and
//! show a short message in a corner of it, which is how a test run announces
//! what it is doing. Here they are [`Page::post_message`],
//! [`Page::activate_messenger`] and [`Page::set_messenger_theme`].
//!
//! The library comes from a public CDN, so the page must be able to reach it.
//! Every piece of text that reaches the page is escaped for where it lands: a
//! message is escaped as HTML, because Messenger shows it as HTML, and then
//! written as a JavaScript string literal.
//!
//! [Messenger]: https://github.hubspot.com/messenger/docs/welcome/
//!
//! # Examples
//!
//! ```no_run
//! use std::time::Duration;
//! use seleniumbase_rs::sb_cdp::{
//!     Browser, LaunchOptions, MessageOptions, MessageStyle, MessengerLocation, MessengerSettings,
//! };
//!
//! # async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
//! let browser = Browser::launch(LaunchOptions::default()).await?;
//! let page = browser.default_page().await?;
//! page.goto("https://example.com").await?;
//!
//! page.set_messenger_theme(MessengerSettings {
//!     location: MessengerLocation::TopLeft,
//!     ..MessengerSettings::default()
//! })
//! .await?;
//! page.post_message(
//!     "Logged in",
//!     &MessageOptions {
//!         style: MessageStyle::Success,
//!         duration: Duration::from_secs(3),
//!         ..MessageOptions::default()
//!     },
//! )
//! .await?;
//! # Ok(())
//! # }
//! ```

use std::fmt;
use std::str::FromStr;
use std::time::Duration;

use serde_json::{json, Value};

use super::page::Page;
use crate::error::SeleniumBaseError;

/// jQuery, which Messenger needs, loaded from its own CDN.
pub(crate) const JQUERY_URL: &str = "https://code.jquery.com/jquery-3.7.1.min.js";

/// The Messenger style sheet.
pub(crate) const MESSENGER_CSS_URL: &str =
    "https://cdnjs.cloudflare.com/ajax/libs/messenger/1.5.0/css/messenger.min.css";

/// The Messenger script.
pub(crate) const MESSENGER_JS_URL: &str =
    "https://cdnjs.cloudflare.com/ajax/libs/messenger/1.5.0/js/messenger.min.js";

/// The style sheets Python loads, in its order: Messenger's own, then one for
/// each of the five themes (flat, future, block, air, ice).
pub(crate) const STYLE_SHEET_URLS: [&str; 6] = [
    MESSENGER_CSS_URL,
    "https://cdnjs.cloudflare.com/ajax/libs/messenger/1.5.0/css/messenger-theme-flat.min.css",
    "https://cdnjs.cloudflare.com/ajax/libs/messenger/1.5.0/css/messenger-theme-future.min.css",
    "https://cdnjs.cloudflare.com/ajax/libs/messenger/1.5.0/css/messenger-theme-block.min.css",
    "https://cdnjs.cloudflare.com/ajax/libs/messenger/1.5.0/css/messenger-theme-air.min.css",
    "https://cdnjs.cloudflare.com/ajax/libs/messenger/1.5.0/css/messenger-theme-ice.min.css",
];

/// The style sheet of Messenger's spinner.
pub(crate) const SPINNER_CSS_URL: &str =
    "https://cdnjs.cloudflare.com/ajax/libs/messenger/1.5.0/css/messenger-spinner.min.css";

/// Underscore, which Messenger needs (the 1.13.6 Python loads).
pub(crate) const UNDERSCORE_URL: &str =
    "https://cdnjs.cloudflare.com/ajax/libs/underscore.js/1.13.6/underscore-min.js";

/// The scripts of the two themes Python loads once Messenger is in the page:
/// flat and future.
pub(crate) const THEME_SCRIPT_URLS: [&str; 2] = [
    "https://cdnjs.cloudflare.com/ajax/libs/messenger/1.5.0/js/messenger-theme-flat.min.js",
    "https://cdnjs.cloudflare.com/ajax/libs/messenger/1.5.0/js/messenger-theme-future.min.js",
];

/// SeleniumBase's own style for Messenger (Python's `get_messenger_style`): a
/// larger font, and the soft glow of the flat and future themes.
const MESSENGER_STYLE: &str = "\
.messenger-message-inner {
    font-family: \"open-sans\",Arial,sans-serif !important;
    font-size: 17px;
}
ul.messenger-theme-flat, ul.messenger-theme-future {
    box-shadow: 2px 2px 9px 4px rgba(32, 142, 120, 0.28),
    2px 2px 9px 4px rgba(200, 240, 80, 0.34) !important;
}
";

/// How long a message stays, when none is given: the 2.55 seconds Python
/// SeleniumBase uses.
const DEFAULT_DURATION: Duration = Duration::from_millis(2550);

/// How many messages the screen holds at once, when none is given.
const DEFAULT_MAX_MESSAGES: u16 = 8;

/// The kind of a message, which sets its colour and icon.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::sb_cdp::MessageStyle;
///
/// assert_eq!("error".parse::<MessageStyle>()?, MessageStyle::Error);
/// assert_eq!(MessageStyle::default().as_str(), "info");
/// assert!("loud".parse::<MessageStyle>().is_err());
/// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
/// ```
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum MessageStyle {
    /// A plain notice.
    #[default]
    Info,
    /// Something worked.
    Success,
    /// Something failed.
    Error,
}

impl MessageStyle {
    /// The name Messenger and Python SeleniumBase give it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Success => "success",
            Self::Error => "error",
        }
    }
}

impl FromStr for MessageStyle {
    type Err = SeleniumBaseError;

    /// Reads `info`, `success` or `error`; `default` is `info`.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text.trim().to_ascii_lowercase().as_str() {
            "info" | "default" => Ok(Self::Info),
            "success" => Ok(Self::Success),
            "error" => Ok(Self::Error),
            _ => Err(SeleniumBaseError::parse(
                text,
                "a message style: info, success or error",
            )),
        }
    }
}

/// How Messenger draws its messages.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum MessengerTheme {
    /// Flat boxes.
    Flat,
    /// Rounded boxes with a shadow (the default).
    #[default]
    Future,
    /// Square boxes.
    Block,
    /// Bars across the screen.
    Air,
    /// Dark boxes.
    Ice,
}

impl MessengerTheme {
    /// The name Messenger gives it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Flat => "flat",
            Self::Future => "future",
            Self::Block => "block",
            Self::Air => "air",
            Self::Ice => "ice",
        }
    }
}

impl FromStr for MessengerTheme {
    type Err = SeleniumBaseError;

    /// Reads `flat`, `future`, `block`, `air` or `ice`; `default` is `future`.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text.trim().to_ascii_lowercase().as_str() {
            "flat" => Ok(Self::Flat),
            "future" | "default" => Ok(Self::Future),
            "block" => Ok(Self::Block),
            "air" => Ok(Self::Air),
            "ice" => Ok(Self::Ice),
            _ => Err(SeleniumBaseError::parse(
                text,
                "a messenger theme: flat, future, block, air or ice",
            )),
        }
    }
}

/// Which corner or edge of the page the messages appear in.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum MessengerLocation {
    /// Top left.
    TopLeft,
    /// Top, in the middle.
    TopCenter,
    /// Top right.
    TopRight,
    /// Bottom left.
    BottomLeft,
    /// Bottom, in the middle.
    BottomCenter,
    /// Bottom right (the default).
    #[default]
    BottomRight,
}

impl MessengerLocation {
    /// The name Python SeleniumBase gives it, such as `top_left`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TopLeft => "top_left",
            Self::TopCenter => "top_center",
            Self::TopRight => "top_right",
            Self::BottomLeft => "bottom_left",
            Self::BottomCenter => "bottom_center",
            Self::BottomRight => "bottom_right",
        }
    }

    /// The CSS classes that put Messenger there.
    fn css_classes(self) -> &'static str {
        match self {
            Self::TopLeft => "messenger-fixed messenger-on-top messenger-on-left",
            Self::TopCenter => "messenger-fixed messenger-on-top",
            Self::TopRight => "messenger-fixed messenger-on-top messenger-on-right",
            Self::BottomLeft => "messenger-fixed messenger-on-bottom messenger-on-left",
            Self::BottomCenter => "messenger-fixed messenger-on-bottom",
            Self::BottomRight => "messenger-fixed messenger-on-bottom messenger-on-right",
        }
    }
}

impl FromStr for MessengerLocation {
    type Err = SeleniumBaseError;

    /// Reads `top_left`, `top_center`, `top_right`, `bottom_left`,
    /// `bottom_center` or `bottom_right`; `default` is `bottom_right`.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "top_left" => Ok(Self::TopLeft),
            "top_center" => Ok(Self::TopCenter),
            "top_right" => Ok(Self::TopRight),
            "bottom_left" => Ok(Self::BottomLeft),
            "bottom_center" => Ok(Self::BottomCenter),
            "bottom_right" | "default" => Ok(Self::BottomRight),
            _ => Err(SeleniumBaseError::parse(
                text,
                "a messenger location such as top_left or bottom_right",
            )),
        }
    }
}

impl fmt::Display for MessengerLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How the messages in a page look and where they appear.
///
/// The default is Python SeleniumBase's: the future theme in the bottom right
/// corner, eight messages at most.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MessengerSettings {
    /// How messages are drawn.
    pub theme: MessengerTheme,
    /// Where they appear.
    pub location: MessengerLocation,
    /// The most messages shown at once.
    pub max_messages: u16,
}

impl Default for MessengerSettings {
    fn default() -> Self {
        Self {
            theme: MessengerTheme::default(),
            location: MessengerLocation::default(),
            max_messages: DEFAULT_MAX_MESSAGES,
        }
    }
}

impl From<MessengerTheme> for MessengerSettings {
    /// The settings with this theme and Python's defaults for the rest.
    fn from(theme: MessengerTheme) -> Self {
        Self {
            theme,
            ..Self::default()
        }
    }
}

impl From<MessengerLocation> for MessengerSettings {
    /// The settings with this location and Python's defaults for the rest.
    fn from(location: MessengerLocation) -> Self {
        Self {
            location,
            ..Self::default()
        }
    }
}

impl MessengerSettings {
    /// The `Messenger.options` object these settings make.
    fn options(self) -> Value {
        json!({
            "theme": self.theme.as_str(),
            "extraClasses": self.location.css_classes(),
            "maxMessages": self.max_messages,
        })
    }
}

/// How one message is shown.
///
/// The default is Python SeleniumBase's: an `info` message for 2.55 seconds,
/// and the call waits that long before it returns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MessageOptions {
    /// How long the message stays.
    pub duration: Duration,
    /// Whether the call waits for [`duration`](Self::duration) before it
    /// returns, so the next step does not start until the message is gone.
    pub pause: bool,
    /// The kind of message.
    pub style: MessageStyle,
}

impl Default for MessageOptions {
    fn default() -> Self {
        Self {
            duration: DEFAULT_DURATION,
            pause: true,
            style: MessageStyle::default(),
        }
    }
}

/// `text` with the characters HTML gives a meaning written as entities.
pub(crate) fn html_escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            other => escaped.push(other),
        }
    }
    escaped
}

/// `text` as a JavaScript string literal.
fn js_string(text: &str) -> String {
    // JSON allows the line and paragraph separators raw; a script engine older
    // than ES2019 ends the literal at one, so they are written as escapes.
    Value::String(text.to_owned())
        .to_string()
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}

/// A script that loads what Python's `activate_messenger` loads (jQuery, the
/// style sheets of Messenger, its five themes and its spinner, Underscore, the
/// Messenger script and SeleniumBase's own style), sets Python's starting
/// options, loads the scripts of the flat and future themes, and settles once
/// all are loaded. Each file is added only if the page does not have it yet.
/// Something the page already has, such as its own jQuery, is left alone.
/// The promise the script returns settles when everything has loaded, or is
/// rejected with an error naming the file the page could not load.
pub(crate) fn activate_script() -> String {
    format!(
        "(async () => {{
            const attach = (element) => new Promise((resolve, reject) => {{
                element.onload = () => resolve(true);
                element.onerror = () => reject(new Error(
                    'the page could not load ' + (element.src || element.href)));
                (document.head || document.documentElement).appendChild(element);
            }});
            const script = async (url) => {{
                if (document.querySelector('script[src=\"' + url + '\"]')) {{ return; }}
                const element = document.createElement('script');
                element.src = url;
                await attach(element);
            }};
            const sheet = async (url) => {{
                if (document.querySelector('link[href=\"' + url + '\"]')) {{ return; }}
                const element = document.createElement('link');
                element.rel = 'stylesheet';
                element.href = url;
                await attach(element);
            }};
            if (typeof window.jQuery === 'undefined') {{ await script({jquery}); }}
            for (const url of {sheets}) {{ await sheet(url); }}
            if (typeof window._ === 'undefined') {{ await script({underscore}); }}
            await sheet({spinner});
            if (typeof window.Messenger === 'undefined') {{ await script({messenger}); }}
            if (!document.getElementById('sbcdp-messenger-style')) {{
                const style = document.createElement('style');
                style.id = 'sbcdp-messenger-style';
                style.textContent = {style};
                (document.head || document.documentElement).appendChild(style);
            }}
            window.Messenger.options = {options};
            for (const url of {themes}) {{ await script(url); }}
            return true;
        }})()",
        jquery = js_string(JQUERY_URL),
        sheets = json!(STYLE_SHEET_URLS),
        underscore = js_string(UNDERSCORE_URL),
        spinner = js_string(SPINNER_CSS_URL),
        messenger = js_string(MESSENGER_JS_URL),
        style = js_string(MESSENGER_STYLE),
        options = MessengerSettings::default().options(),
        themes = json!(THEME_SCRIPT_URLS),
    )
}

/// A script that applies `settings`, and gives `false` if Messenger is not in
/// the page.
pub(crate) fn theme_script(settings: MessengerSettings) -> String {
    format!(
        "(() => {{
            if (typeof window.Messenger === 'undefined') {{ return false; }}
            const options = {options};
            window.Messenger.options = options;
            const current = window.Messenger();
            current.options = Object.assign(current.options || {{}}, options);
            return true;
        }})()",
        options = settings.options(),
    )
}

/// A script that posts `markup`, which Messenger shows as HTML (escape it
/// first to show text, as [`Page::post_message`] does), and gives `false` if
/// Messenger is not in the page.
pub(crate) fn post_markup_script(markup: &str, options: &MessageOptions) -> String {
    format!(
        "(() => {{
            if (typeof window.Messenger === 'undefined') {{ return false; }}
            window.Messenger().post({{
                message: {message},
                type: {style},
                hideAfter: {seconds},
                hideOnNavigate: true,
            }});
            return true;
        }})()",
        message = js_string(markup),
        style = js_string(options.style.as_str()),
        seconds = json!(options.duration.as_secs_f64()),
    )
}

impl Page {
    /// Loads Messenger, and the jQuery it needs, into the page.
    ///
    /// Python's `activate_messenger`. Anything the page already has is not
    /// loaded again, so calling it twice is harmless. The libraries come from
    /// public CDNs; a page with no network cannot load them.
    ///
    /// The other messenger methods call this themselves when Messenger is not
    /// in the page. A navigation drops Messenger with the page, so it has to be
    /// loaded again after one.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the page cannot load one of
    /// the files.
    pub async fn activate_messenger(&self) -> Result<(), SeleniumBaseError> {
        self.evaluate(activate_script()).await?;
        Ok(())
    }

    /// Chooses how messages look and where they appear.
    ///
    /// Python's `set_messenger_theme`. It applies to the messages posted
    /// afterwards. Messenger is loaded first when the page does not have it.
    /// Pass [`MessengerSettings`], or just a [`MessengerTheme`] or a
    /// [`MessengerLocation`] to change one thing and keep Python's defaults for
    /// the others.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the page cannot load
    /// Messenger.
    pub async fn set_messenger_theme(
        &self,
        settings: impl Into<MessengerSettings>,
    ) -> Result<(), SeleniumBaseError> {
        let settings = settings.into();
        self.run_with_messenger(&theme_script(settings), settings)
            .await
    }

    /// Shows `message` in the page.
    ///
    /// Python's `post_message`. The text is shown as it is, not as HTML: `<b>`
    /// appears as the characters `<b>`. Messenger is loaded, with its default
    /// theme, when the page does not have it. With
    /// [`pause`](MessageOptions::pause) set the call returns only after the
    /// message has had its [`duration`](MessageOptions::duration).
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the page cannot load
    /// Messenger.
    pub async fn post_message(
        &self,
        message: &str,
        options: &MessageOptions,
    ) -> Result<(), SeleniumBaseError> {
        let script = post_markup_script(&html_escape(message), options);
        self.run_with_messenger(&script, MessengerSettings::default())
            .await?;
        if options.pause {
            tokio::time::sleep(options.duration).await;
        }
        Ok(())
    }

    /// Runs a script that answers `false` when Messenger is missing: loads it
    /// and applies `defaults`, then runs the script again.
    async fn run_with_messenger(
        &self,
        script: &str,
        defaults: MessengerSettings,
    ) -> Result<(), SeleniumBaseError> {
        if self.evaluate(script).await? == Value::Bool(true) {
            return Ok(());
        }
        self.activate_messenger().await?;
        self.evaluate(theme_script(defaults)).await?;
        if self.evaluate(script).await? == Value::Bool(true) {
            return Ok(());
        }
        Err(SeleniumBaseError::cdp_driver(
            "Messenger is still missing from the page after it was loaded",
        ))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::sb_cdp::Browser;

    /// What the mocked page answers to the scripts: `ready` is whether
    /// Messenger is "in the page".
    fn answer(mock: &crate::sb_cdp::MockCtrl, ready: bool) {
        mock.reply("Runtime.evaluate", json!({ "result": { "value": ready } }));
    }

    /// The scripts the messenger methods ran, leaving out the page helpers
    /// every tab installs when it is opened.
    fn expressions(mock: &crate::sb_cdp::MockCtrl) -> Vec<String> {
        mock.calls_to("Runtime.evaluate")
            .iter()
            .map(|call| {
                call.params["expression"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned()
            })
            .filter(|expression| !expression.contains("window.__sbcdp = {"))
            .collect()
    }

    #[test]
    fn the_defaults_are_the_ones_python_uses() {
        let options = MessageOptions::default();
        assert_eq!(options.duration, Duration::from_millis(2550));
        assert!(options.pause);
        assert_eq!(options.style, MessageStyle::Info);

        let settings = MessengerSettings::default();
        assert_eq!(settings.theme, MessengerTheme::Future);
        assert_eq!(settings.location, MessengerLocation::BottomRight);
        assert_eq!(settings.max_messages, 8);
    }

    #[test]
    fn the_names_python_takes_are_read_and_the_rest_refused() {
        assert_eq!(
            "future".parse::<MessengerTheme>().unwrap(),
            MessengerTheme::Future
        );
        assert_eq!(
            "default".parse::<MessengerTheme>().unwrap(),
            MessengerTheme::Future
        );
        assert_eq!(
            "top_left".parse::<MessengerLocation>().unwrap(),
            MessengerLocation::TopLeft
        );
        assert_eq!(
            "bottom-center".parse::<MessengerLocation>().unwrap(),
            MessengerLocation::BottomCenter
        );
        assert_eq!(
            "SUCCESS".parse::<MessageStyle>().unwrap(),
            MessageStyle::Success
        );
        for bad in ["", "left", "top left"] {
            assert!(bad.parse::<MessengerLocation>().is_err(), "{bad:?}");
        }
        assert!("neon".parse::<MessengerTheme>().is_err());
        assert!("warning".parse::<MessageStyle>().is_err());
    }

    #[test]
    fn every_location_has_its_own_classes() {
        let all = [
            MessengerLocation::TopLeft,
            MessengerLocation::TopCenter,
            MessengerLocation::TopRight,
            MessengerLocation::BottomLeft,
            MessengerLocation::BottomCenter,
            MessengerLocation::BottomRight,
        ];
        let mut seen = std::collections::BTreeSet::new();
        for location in all {
            assert!(seen.insert(location.css_classes()), "{location} repeats");
            assert!(location.css_classes().starts_with("messenger-fixed"));
            assert_eq!(
                location.as_str().parse::<MessengerLocation>().unwrap(),
                location
            );
        }
        assert!(MessengerLocation::TopLeft
            .css_classes()
            .contains("messenger-on-top"));
        assert!(MessengerLocation::TopLeft
            .css_classes()
            .contains("messenger-on-left"));
        assert!(!MessengerLocation::TopCenter
            .css_classes()
            .contains("-on-left"));
    }

    #[test]
    fn html_is_escaped_so_a_message_shows_as_text() {
        assert_eq!(
            html_escape(r#"<img src=x onerror="alert('hi')"> & more"#),
            "&lt;img src=x onerror=&quot;alert(&#39;hi&#39;)&quot;&gt; &amp; more"
        );
        assert_eq!(html_escape("plain"), "plain");
        assert_eq!(html_escape(""), "");
    }

    #[test]
    fn a_message_can_not_break_out_of_its_string_literal() {
        let hostile = "\"});alert(1);//\n\u{2028}'</script>\\";
        let script = post_markup_script(&html_escape(hostile), &MessageOptions::default());
        // The literal is JSON, so the quote that would end it is escaped, and
        // the HTML in it is already entities.
        let literal = js_string(&html_escape(hostile));
        assert!(script.contains(&format!("message: {literal},")), "{script}");
        assert!(!script.contains("alert(1);//\n"), "{script}");
        assert!(!script.contains("</script>"), "{script}");
        // And the separators are escapes, not raw characters.
        assert!(!literal.contains(['\u{2028}', '\u{2029}']), "{literal}");
        // The literal reads back as exactly the escaped message.
        let back: String = serde_json::from_str(&literal).unwrap();
        assert_eq!(back, html_escape(hostile));
    }

    #[test]
    fn the_post_script_carries_the_style_and_the_seconds() {
        let options = MessageOptions {
            duration: Duration::from_millis(1500),
            pause: false,
            style: MessageStyle::Error,
        };
        let script = post_markup_script("hi", &options);
        assert!(script.contains("type: \"error\","), "{script}");
        assert!(script.contains("hideAfter: 1.5,"), "{script}");
        assert!(script.contains("hideOnNavigate: true"), "{script}");
        assert!(script.contains("window.Messenger().post({"), "{script}");
    }

    #[test]
    fn the_theme_script_sets_the_options_for_the_location_and_limit() {
        let script = theme_script(MessengerSettings {
            theme: MessengerTheme::Future,
            location: MessengerLocation::TopRight,
            max_messages: 3,
        });
        assert!(script.contains(r#""theme":"future""#), "{script}");
        assert!(
            script.contains(
                r#""extraClasses":"messenger-fixed messenger-on-top messenger-on-right""#
            ),
            "{script}"
        );
        assert!(script.contains(r#""maxMessages":3"#), "{script}");
    }

    #[test]
    fn a_theme_or_a_location_alone_is_a_complete_setting() {
        let theme = MessengerSettings::from(MessengerTheme::Air);
        assert_eq!(theme.theme, MessengerTheme::Air);
        assert_eq!(theme.location, MessengerLocation::BottomRight);
        assert_eq!(theme.max_messages, 8);

        let location = MessengerSettings::from(MessengerLocation::TopLeft);
        assert_eq!(location.theme, MessengerTheme::Future);
        assert_eq!(location.location, MessengerLocation::TopLeft);
        assert_eq!(location.max_messages, 8);
    }

    #[tokio::test]
    async fn set_messenger_theme_takes_a_theme_a_location_or_whole_settings() {
        let (browser, mock) = Browser::new_mocked();
        answer(&mock, true);
        let page = browser.default_page().await.unwrap();

        page.set_messenger_theme(MessengerTheme::Air).await.unwrap();
        page.set_messenger_theme(MessengerLocation::TopLeft)
            .await
            .unwrap();
        page.set_messenger_theme(MessengerSettings::default())
            .await
            .unwrap();

        let sent = expressions(&mock);
        assert_eq!(sent.len(), 3, "{sent:?}");
        assert!(sent[0].contains(r#""theme":"air""#), "{}", sent[0]);
        assert!(sent[0].contains("messenger-on-bottom messenger-on-right"));
        assert!(sent[0].contains(r#""maxMessages":8"#), "{}", sent[0]);
        assert!(sent[1].contains(r#""theme":"future""#), "{}", sent[1]);
        assert!(sent[1].contains("messenger-on-top messenger-on-left"));
        assert!(sent[2].contains(r#""theme":"future""#), "{}", sent[2]);
    }

    #[test]
    fn the_activate_script_names_each_file_python_loads_as_a_string_literal() {
        let script = activate_script();
        let files = STYLE_SHEET_URLS
            .into_iter()
            .chain(THEME_SCRIPT_URLS)
            .chain([
                JQUERY_URL,
                UNDERSCORE_URL,
                SPINNER_CSS_URL,
                MESSENGER_JS_URL,
            ]);
        for url in files {
            assert!(
                script.contains(&format!("\"{url}\"")),
                "{url} missing from {script}"
            );
        }
        assert!(script.contains("typeof window.jQuery === 'undefined'"));
        assert!(script.contains("typeof window.Messenger === 'undefined'"));
    }

    #[test]
    fn every_theme_has_its_style_sheet_and_flat_and_future_their_script() {
        for theme in ["flat", "future", "block", "air", "ice"] {
            let sheet = format!("messenger-theme-{theme}.min.css");
            assert!(
                STYLE_SHEET_URLS.iter().any(|url| url.ends_with(&sheet)),
                "no style sheet for {theme}"
            );
        }
        for theme in ["flat", "future"] {
            let file = format!("messenger-theme-{theme}.min.js");
            assert!(
                THEME_SCRIPT_URLS.iter().any(|url| url.ends_with(&file)),
                "no script for {theme}"
            );
        }
        assert!(STYLE_SHEET_URLS[0].ends_with("/messenger.min.css"));
        assert!(SPINNER_CSS_URL.ends_with("/messenger-spinner.min.css"));
        assert!(UNDERSCORE_URL.contains("1.13.6"));
    }

    #[test]
    fn the_activate_script_starts_with_pythons_theme_and_adds_its_own_style_once() {
        let script = activate_script();
        // Python sets `Messenger.options` to the future theme in the bottom
        // right corner, eight messages at most, once Messenger has loaded.
        assert!(script.contains("window.Messenger.options = {"), "{script}");
        assert!(script.contains(r#""theme":"future""#), "{script}");
        assert!(
            script.contains("messenger-fixed messenger-on-bottom messenger-on-right"),
            "{script}"
        );
        assert!(script.contains("getElementById('sbcdp-messenger-style')"));
        assert!(script.contains("messenger-message-inner"), "{script}");
        // The two theme scripts come after the options, as in Python.
        let options = script.find("window.Messenger.options").unwrap();
        let theme_script = script.find("messenger-theme-flat.min.js").unwrap();
        assert!(options < theme_script, "{script}");
    }

    #[tokio::test]
    async fn a_page_that_has_messenger_is_posted_to_directly() {
        let (browser, mock) = Browser::new_mocked();
        answer(&mock, true);
        let page = browser.default_page().await.unwrap();

        let options = MessageOptions {
            pause: false,
            ..MessageOptions::default()
        };
        page.post_message("Hello <b>world</b>", &options)
            .await
            .unwrap();

        let sent = expressions(&mock);
        assert_eq!(sent.len(), 1, "{sent:?}");
        assert!(sent[0].contains("&lt;b&gt;world&lt;/b&gt;"), "{}", sent[0]);
        assert!(!sent[0].contains("<b>"), "{}", sent[0]);
    }

    #[tokio::test]
    async fn a_page_without_messenger_gets_it_loaded_and_themed_first() {
        let (browser, mock) = Browser::new_mocked();
        // The first script finds no Messenger; once it is loaded, every
        // script is answered with success.
        let loaded = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = std::sync::Arc::clone(&loaded);
        mock.on("Runtime.evaluate", move |params| {
            let expression = params["expression"].as_str().unwrap_or_default();
            if expression.contains("https://code.jquery.com") {
                flag.store(true, std::sync::atomic::Ordering::SeqCst);
                return Ok(json!({ "result": { "value": true } }));
            }
            let ready = flag.load(std::sync::atomic::Ordering::SeqCst);
            Ok(json!({ "result": { "value": ready } }))
        });
        let page = browser.default_page().await.unwrap();

        let options = MessageOptions {
            pause: false,
            style: MessageStyle::Success,
            ..MessageOptions::default()
        };
        page.post_message("done", &options).await.unwrap();

        let sent = expressions(&mock);
        assert_eq!(sent.len(), 4, "{sent:?}");
        assert!(
            sent[0].contains("hideOnNavigate"),
            "first tries to post: {}",
            sent[0]
        );
        assert!(
            sent[1].contains("code.jquery.com"),
            "then loads: {}",
            sent[1]
        );
        assert!(
            sent[2].contains("Messenger.options"),
            "then themes: {}",
            sent[2]
        );
        assert!(
            sent[3].contains("hideOnNavigate"),
            "then posts: {}",
            sent[3]
        );
    }

    #[tokio::test]
    async fn a_page_that_never_gets_messenger_is_an_error() {
        let (browser, mock) = Browser::new_mocked();
        answer(&mock, false);
        let page = browser.default_page().await.unwrap();
        let options = MessageOptions {
            pause: false,
            ..MessageOptions::default()
        };
        let error = page.post_message("x", &options).await.unwrap_err();
        assert!(error.to_string().contains("Messenger"), "{error}");
    }

    #[tokio::test]
    async fn a_script_error_in_the_page_is_reported() {
        let (browser, mock) = Browser::new_mocked();
        mock.reply(
            "Runtime.evaluate",
            json!({ "exceptionDetails": { "text": "Uncaught", "exception": {
                "description": "Error: the page could not load https://x/y.js" } } }),
        );
        let page = browser.default_page().await.unwrap();
        let error = page.activate_messenger().await.unwrap_err();
        assert!(error.to_string().contains("could not load"), "{error}");
    }

    #[tokio::test(start_paused = true)]
    async fn pause_waits_out_the_duration_and_no_pause_does_not() {
        let (browser, mock) = Browser::new_mocked();
        answer(&mock, true);
        let page = browser.default_page().await.unwrap();

        let started = tokio::time::Instant::now();
        let no_pause = MessageOptions {
            pause: false,
            ..MessageOptions::default()
        };
        page.post_message("a", &no_pause).await.unwrap();
        assert!(started.elapsed() < Duration::from_millis(100));

        let pause = MessageOptions {
            duration: Duration::from_secs(5),
            ..MessageOptions::default()
        };
        page.post_message("b", &pause).await.unwrap();
        assert!(
            started.elapsed() >= Duration::from_secs(5),
            "{:?}",
            started.elapsed()
        );
    }

    #[tokio::test]
    async fn the_theme_is_applied_to_the_tab_it_was_set_on() {
        let (browser, mock) = Browser::new_mocked();
        answer(&mock, true);
        let page = browser.default_page().await.unwrap();
        page.set_messenger_theme(MessengerSettings {
            location: MessengerLocation::TopLeft,
            ..MessengerSettings::default()
        })
        .await
        .unwrap();
        let sent = expressions(&mock);
        assert_eq!(sent.len(), 1, "{sent:?}");
        assert!(
            sent[0].contains("messenger-on-top messenger-on-left"),
            "{}",
            sent[0]
        );
        // And it went to the tab's own protocol session.
        let call = mock
            .calls_to("Runtime.evaluate")
            .into_iter()
            .find(|call| call.params["expression"].as_str() == Some(sent[0].as_str()))
            .expect("the theme script was sent");
        assert_eq!(call.session_id.as_deref(), Some("S1"));
    }
}
