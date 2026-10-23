//! jquery-confirm dialog boxes: the page dialogs of Python's `get_jqc_*`
//! methods, with their themes.
//!
//! [`BaseCase::get_jqc_button_input`], [`BaseCase::get_jqc_text_input`] and
//! [`BaseCase::get_jqc_form_inputs`] open a [jquery-confirm] box in the page,
//! wait for the person to answer it and return what was clicked or typed:
//!
//! ```no_run
//! # use seleniumbase_rs::{BaseCase, SeleniumBaseError};
//! # async fn demo(sb: &BaseCase) -> Result<(), SeleniumBaseError> {
//! let answer = sb
//!     .get_jqc_button_input("Continue?", [("Yes", "green"), ("No", "red")])
//!     .await?;
//! let name = sb
//!     .get_jqc_text_input("Your name?")
//!     .button(("Send", "orange"))
//!     .options([("theme", "material"), ("width", "52%")])
//!     .await?;
//! let (text, button) = sb.get_jqc_form_inputs("Reason?", ["OK", "Cancel"]).await?;
//! # Ok(()) }
//! ```
//!
//! Python's keyword arguments are builder methods (see [`call`](crate::api::call)):
//! `.options(...)` takes Python's list of `(name, value)` pairs, and `.theme`,
//! `.color` and `.width` take the typed [`JqcTheme`], [`JqcColor`] and
//! [`JqcWidth`]. Buttons are a text, or a text with a colour: `"OK"`,
//! `("OK", "green")` or `("OK", JqcColor::Green)`, or a [`JqcButton`].
//!
//! # Where nobody watches
//!
//! In a headless browser or on a virtual display (`nobody_watches`, Python's
//! `headless or headless2 or xvfb`) no dialog opens and no script runs: the
//! calls return at once with the value Python returns there. That is the text
//! of the last button for [`BaseCase::get_jqc_button_input`], `""` for
//! [`BaseCase::get_jqc_text_input`] and `("", last button)` for
//! [`BaseCase::get_jqc_form_inputs`]. The arguments are checked first, so a
//! bad colour or option fails in a headless run too.
//!
//! # Themes
//!
//! [`BaseCase::set_jqc_theme`] stores the theme, border colour and width the
//! next dialogs use, and [`BaseCase::reset_jqc_theme`] brings the factory
//! defaults ([`JqcDefaults::default`]) back. They are kept per [`BaseCase`], not
//! process-wide as in Python, and a reset really resets (upstream's is a no-op).
//!
//! [jquery-confirm]: https://craftpip.github.io/jquery-confirm/

use std::fmt;
use std::future::IntoFuture;
use std::str::FromStr;

use serde_json::Value;

use crate::api::base_case::BaseCase;
use crate::api::call::CallFuture;
use crate::error::SeleniumBaseError;
use crate::js_code::quote;

/// The theme names, in the order Python lists them in its error messages.
const THEME_NAMES: [&str; 7] = [
    "bootstrap",
    "modern",
    "material",
    "supervan",
    "light",
    "dark",
    "seamless",
];

/// The colour names, in the order Python lists them in its error messages.
const COLOR_NAMES: [&str; 7] = [
    "blue", "default", "green", "red", "purple", "orange", "dark",
];

/// The form a text or form dialog shows: Python's `form_code`, with the input
/// width as the percent sign it meant (Python's `%%` is not valid CSS).
const FORM_HTML: &str = concat!(
    r#"<form align="center" action="" class="jqc_form"><div class="form-group">"#,
    r#"<input style="font-size:20px; background-color: #f8fdfd; width: 84%; "#,
    r#"border: 1px solid blue; box-shadow:inset 0 0 2px 2px #f4fafa;" "#,
    r#"type="text" class="jqc_input" /></div></form>"#,
);

/// Focuses the box of a button dialog.
const FOCUS_BOX: &str = "document.querySelector('.jconfirm-box').focus();";

/// Focuses the input of a text or form dialog.
const FOCUS_INPUT: &str = "document.querySelector('.jconfirm-box input.jqc_input').focus();";

/// How many dialogs are open: `jconfirm.instances.length`, or `-1` when the
/// page no longer has jquery-confirm (it navigated).
pub(crate) const OPEN_COUNT_SCRIPT: &str =
    "return (typeof jconfirm === 'undefined' || !jconfirm.instances) ? -1 : jconfirm.instances.length;";

/// Reads what the last dialog stored, or `null` when it stored nothing.
pub(crate) const ANSWER_SCRIPT: &str = "return window.__sbJqcAnswer || null;";

/// A jquery-confirm theme.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum JqcTheme {
    /// `bootstrap`, the default.
    #[default]
    Bootstrap,
    /// `modern`.
    Modern,
    /// `material`.
    Material,
    /// `supervan`.
    Supervan,
    /// `light`.
    Light,
    /// `dark`.
    Dark,
    /// `seamless`.
    Seamless,
}

impl JqcTheme {
    /// Every theme, in the order Python lists them.
    pub const ALL: [Self; 7] = [
        Self::Bootstrap,
        Self::Modern,
        Self::Material,
        Self::Supervan,
        Self::Light,
        Self::Dark,
        Self::Seamless,
    ];

    /// The theme's name as jquery-confirm knows it: lower case.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bootstrap => "bootstrap",
            Self::Modern => "modern",
            Self::Material => "material",
            Self::Supervan => "supervan",
            Self::Light => "light",
            Self::Dark => "dark",
            Self::Seamless => "seamless",
        }
    }

    /// How dark the page behind the box is, as Python sets it per theme.
    const fn overlay_opacity(self) -> &'static str {
        match self {
            Self::Supervan => "0.56",
            Self::Bootstrap => "0.64",
            Self::Modern => "0.5",
            Self::Material => "0.4",
            Self::Light | Self::Dark | Self::Seamless => "0.32",
        }
    }
}

impl fmt::Display for JqcTheme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for JqcTheme {
    type Err = SeleniumBaseError;

    /// Reads a theme name, in any case, as Python's `set_jqc_theme` does.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::InvalidConfig`] with Python's message when the
    /// name is not one of the seven themes.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let lower = text.to_lowercase();
        Self::ALL
            .into_iter()
            .find(|theme| theme.as_str() == lower)
            .ok_or_else(|| {
                invalid(format!(
                    "{lower} is not a valid jquery-confirm theme! Select from {}",
                    python_list(&THEME_NAMES)
                ))
            })
    }
}

/// A jquery-confirm colour: the colour of a dialog's border, or of a button.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum JqcColor {
    /// `blue`, the default.
    #[default]
    Blue,
    /// `default`, a plain grey.
    Default,
    /// `green`.
    Green,
    /// `red`.
    Red,
    /// `purple`.
    Purple,
    /// `orange`.
    Orange,
    /// `dark`.
    Dark,
}

impl JqcColor {
    /// Every colour, in the order Python lists them.
    pub const ALL: [Self; 7] = [
        Self::Blue,
        Self::Default,
        Self::Green,
        Self::Red,
        Self::Purple,
        Self::Orange,
        Self::Dark,
    ];

    /// The colour's name as jquery-confirm knows it: lower case.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Blue => "blue",
            Self::Default => "default",
            Self::Green => "green",
            Self::Red => "red",
            Self::Purple => "purple",
            Self::Orange => "orange",
            Self::Dark => "dark",
        }
    }

    /// The colour called `text` (in any case), or `None`.
    fn named(text: &str) -> Option<Self> {
        let lower = text.to_lowercase();
        Self::ALL.into_iter().find(|color| color.as_str() == lower)
    }

    /// Reads the colour of a button, with Python's message for a bad one.
    fn for_button(text: &str) -> Result<Self, SeleniumBaseError> {
        Self::named(text).ok_or_else(|| {
            invalid(format!(
                "{} is an invalid jquery-confirm button color!\nSelect from {}",
                text.to_lowercase(),
                python_list(&COLOR_NAMES)
            ))
        })
    }
}

impl fmt::Display for JqcColor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for JqcColor {
    type Err = SeleniumBaseError;

    /// Reads a border colour name, in any case, as Python's `set_jqc_theme`
    /// does.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::InvalidConfig`] with Python's message when the
    /// name is not one of the seven colours.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::named(text).ok_or_else(|| {
            invalid(format!(
                "{} is not a valid jquery-confirm border color! Select from {}",
                text.to_lowercase(),
                python_list(&COLOR_NAMES)
            ))
        })
    }
}

/// The width of a dialog box: a percentage (`38%`) or pixels (`450px`).
///
/// [`FromStr`] follows Python's `set_jqc_theme`: a whole number is a percent up
/// to 100 and pixels above (`52` is `52%`, `450` is `450px`), and anything else
/// must end in `%` or `px` after a positive number.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JqcWidth(String);

impl JqcWidth {
    /// The width as CSS: `38%` or `450px`.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for JqcWidth {
    /// `38%`, as in Python.
    fn default() -> Self {
        Self("38%".to_owned())
    }
}

impl fmt::Display for JqcWidth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for JqcWidth {
    type Err = SeleniumBaseError;

    /// Reads a width with Python's rules and messages.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::InvalidConfig`] when the width is zero or
    /// negative, is not a number, or does not end in `%` or `px`.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let text = text.trim();
        if !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit()) {
            // A whole number: up to 100 it is a percent, above it pixels.
            let digits = text.trim_start_matches('0');
            if digits.is_empty() {
                return Err(invalid("Width must be set to a positive number!"));
            }
            let unit = if digits.len() < 3 || digits == "100" {
                "%"
            } else {
                "px"
            };
            return Ok(Self(format!("{text}{unit}")));
        }
        let number = text
            .strip_suffix('%')
            .or_else(|| text.strip_suffix("px"))
            .ok_or_else(|| invalid("jqc width must end with % for percent or px for pixels!"))?;
        let value = number
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite())
            .ok_or_else(|| invalid(format!("{number} is not a numeric value!")))?;
        if value <= 0.0 {
            return Err(invalid(format!("{value:?} is not a positive number!")));
        }
        Ok(Self(text.to_owned()))
    }
}

/// The theme, border colour and width the dialogs use unless a call says
/// otherwise: what [`BaseCase::set_jqc_theme`] stores.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct JqcDefaults {
    /// The theme; `bootstrap` by default.
    pub theme: JqcTheme,
    /// The border colour; `blue` by default.
    pub color: JqcColor,
    /// The width of the box; `38%` by default.
    pub width: JqcWidth,
}

/// One button of a dialog: its text, which is also what the call returns when it
/// is clicked, and its colour.
///
/// A text (`"OK"`), a text and a colour name (`("OK", "green")`) or a text and a
/// [`JqcColor`] convert into one, so the calls take any of them. A button with
/// no colour is blue. A colour name is checked when the call runs: a bad one is
/// an error then, with Python's message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JqcButton {
    /// The text on the button, which may hold HTML only as text: it is shown
    /// escaped.
    pub text: String,
    color: ButtonColor,
}

/// What a button's colour was given as.
#[derive(Clone, Debug, PartialEq, Eq)]
enum ButtonColor {
    /// No colour: blue.
    Unset,
    /// A [`JqcColor`].
    Typed(JqcColor),
    /// A name that is read when the call runs.
    Named(String),
}

impl JqcButton {
    /// A blue button with `text`.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            color: ButtonColor::Unset,
        }
    }

    /// The same button in `color`.
    #[must_use]
    pub fn color(mut self, color: JqcColor) -> Self {
        self.color = ButtonColor::Typed(color);
        self
    }

    /// The button with its colour read, or the error for a bad colour name.
    fn resolve(&self) -> Result<ResolvedButton, SeleniumBaseError> {
        let color = match &self.color {
            ButtonColor::Unset => JqcColor::Blue,
            ButtonColor::Typed(color) => *color,
            // Python treats an empty colour as none given.
            ButtonColor::Named(name) if name.is_empty() => JqcColor::Blue,
            ButtonColor::Named(name) => JqcColor::for_button(name)?,
        };
        Ok(ResolvedButton {
            text: self.text.clone(),
            color,
        })
    }
}

impl From<&str> for JqcButton {
    fn from(text: &str) -> Self {
        Self::new(text)
    }
}

impl From<String> for JqcButton {
    fn from(text: String) -> Self {
        Self::new(text)
    }
}

impl From<(&str, &str)> for JqcButton {
    fn from((text, color): (&str, &str)) -> Self {
        Self {
            text: text.to_owned(),
            color: ButtonColor::Named(color.to_owned()),
        }
    }
}

impl From<(&str, JqcColor)> for JqcButton {
    fn from((text, color): (&str, JqcColor)) -> Self {
        Self::new(text).color(color)
    }
}

/// A button whose colour has been checked.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ResolvedButton {
    text: String,
    color: JqcColor,
}

/// One setting a call changes for its own dialog.
#[derive(Clone, Debug)]
enum JqcOption {
    /// A `(name, value)` pair, Python's form: the name is `theme`, `color` or
    /// `width`, in any case.
    Named(String, String),
    Theme(JqcTheme),
    Color(JqcColor),
    Width(JqcWidth),
}

/// `defaults` with `options` applied in order, or the error of the first bad one.
fn resolve_style(
    defaults: &JqcDefaults,
    options: &[JqcOption],
) -> Result<JqcDefaults, SeleniumBaseError> {
    let mut style = defaults.clone();
    for option in options {
        match option {
            JqcOption::Named(name, value) => match name.to_lowercase().as_str() {
                "theme" => style.theme = value.parse()?,
                "color" => style.color = value.parse()?,
                "width" => style.width = value.parse()?,
                _ => return Err(invalid(format!("Unknown option: \"{name}\""))),
            },
            JqcOption::Theme(theme) => style.theme = *theme,
            JqcOption::Color(color) => style.color = *color,
            JqcOption::Width(width) => style.width = width.clone(),
        }
    }
    Ok(style)
}

impl JqcDefaults {
    /// These defaults with the theme, and the colour and width when they are
    /// given, replaced. Nothing changes unless every part is valid. An empty
    /// colour or width counts as none, as in Python.
    pub(crate) fn changed(
        &self,
        theme: &str,
        color: Option<&str>,
        width: Option<&str>,
    ) -> Result<Self, SeleniumBaseError> {
        fn given(text: Option<&str>) -> Option<&str> {
            text.filter(|text| !text.is_empty())
        }
        let mut next = self.clone();
        next.theme = theme.parse()?;
        if let Some(color) = given(color) {
            next.color = color.parse()?;
        }
        if let Some(width) = given(width) {
            next.width = width.parse()?;
        }
        Ok(next)
    }
}

/// What a dialog was answered with.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Answer {
    /// The text of the button clicked.
    pub(crate) button: String,
    /// The text of the input; empty for a button dialog.
    pub(crate) input: String,
}

impl Answer {
    /// Reads what the page stored, or the error that says the dialog went away
    /// without an answer.
    pub(crate) fn from_value(value: &Value) -> Result<Self, SeleniumBaseError> {
        if !value.is_object() {
            return Err(closed_without_answer());
        }
        let text = |key: &str| {
            value
                .get(key)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned()
        };
        Ok(Self {
            button: text("button"),
            input: text("input"),
        })
    }
}

/// The error of a dialog that closed with no answer.
pub(crate) fn closed_without_answer() -> SeleniumBaseError {
    invalid("the jquery-confirm dialog closed without an answer: the page navigated or was closed")
}

fn invalid(message: impl Into<String>) -> SeleniumBaseError {
    SeleniumBaseError::InvalidConfig(message.into())
}

/// Python's `repr` of a list of strings, as its messages print the choices.
fn python_list(names: &[&str]) -> String {
    let items: Vec<String> = names.iter().map(|name| format!("'{name}'")).collect();
    format!("[{}]", items.join(", "))
}

/// `text` as HTML text: what shows is `text`, whatever characters it holds.
fn escape_html(text: &str) -> String {
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

/// The keys that click button `number` (counting from 1) of `total`.
///
/// One button answers to Enter, `y` and `1`. Of several, a `Yes` button answers
/// to `y`, a `No` button to `n`, and each to its number while it is below 10.
fn shortcut_keys(total: usize, number: usize, text: &str) -> Vec<String> {
    if total == 1 {
        return ["enter", "y", "1"].map(str::to_owned).to_vec();
    }
    let mut keys = Vec::new();
    match text.to_lowercase().as_str() {
        "yes" => keys.push("y".to_owned()),
        "no" => keys.push("n".to_owned()),
        _ => {}
    }
    if number < 10 {
        keys.push(number.to_string());
    }
    keys
}

/// The JavaScript of a button's action: it stores the answer, in the globals
/// Python's dialogs set and in `window.__sbJqcAnswer`, which is what the call
/// reads. `reads_input` takes the text from the dialog's input.
fn action_script(text: &str, reads_input: bool) -> String {
    let input = if reads_input {
        "this.$content.find('.jqc_input').val()"
    } else {
        "''"
    };
    format!(
        "function () {{ var input = {input}; var button = {button}; \
         window.jqc_input = input; window.$jqc_input = input; jconfirm.lastInputText = input; \
         window.jqc_status = button; window.$jqc_status = button; jconfirm.lastButtonText = button; \
         window.__sbJqcAnswer = {{ button: button, input: input }}; }}",
        button = quote(text)
    )
}

/// One entry of the `buttons` object of `jconfirm({...})`.
fn button_entry(name: &str, button: &ResolvedButton, keys: &[String], reads_input: bool) -> String {
    let keys: Vec<String> = keys.iter().map(|key| quote(key)).collect();
    let shown = format!("<b>{}</b>", escape_html(&button.text));
    format!(
        "{name}: {{ btnClass: {class}, text: {text}, keys: [{keys}], action: {action} }}",
        class = quote(&format!("btn-{}", button.color)),
        text = quote(&shown),
        keys = keys.join(", "),
        action = action_script(&button.text, reads_input)
    )
}

/// The `jconfirm({...})` call around the parts that differ between the kinds of
/// dialog. Every value from the caller goes in as a JavaScript string literal;
/// the message stays HTML, as in Python.
fn dialog_script(
    message: &str,
    style: &JqcDefaults,
    with_form: bool,
    buttons: &[String],
    on_ready: &str,
) -> String {
    let title = format!("<div></div><font color=\"#0066ee\">{message}</font>");
    let content = if with_form {
        format!("<div></div>{FORM_HTML}")
    } else {
        "<div></div>".to_owned()
    };
    let ready = if on_ready.is_empty() {
        String::new()
    } else {
        format!(", onContentReady: function () {{ var jc = this; {on_ready} }}")
    };
    format!(
        "window.__sbJqcAnswer = null; \
         jconfirm({{ boxWidth: {width}, useBootstrap: false, containerFluid: true, \
         bgOpacity: {opacity}, type: {color}, theme: {theme}, animationBounce: 1, \
         typeAnimated: true, animation: 'scale', draggable: true, dragWindowGap: 1, \
         container: 'body', title: {title}, content: {content}, \
         buttons: {{ {buttons} }}{ready} }});",
        width = quote(style.width.as_str()),
        opacity = style.theme.overlay_opacity(),
        color = quote(style.color.as_str()),
        theme = quote(style.theme.as_str()),
        title = quote(&title),
        content = quote(&content),
        buttons = buttons.join(", "),
    )
}

/// The script that opens a dialog of buttons, for `message`, `buttons` and the
/// resolved `style`.
fn button_dialog_script(message: &str, buttons: &[ResolvedButton], style: &JqcDefaults) -> String {
    let entries: Vec<String> = buttons
        .iter()
        .enumerate()
        .map(|(index, button)| {
            let number = index + 1;
            let keys = shortcut_keys(buttons.len(), number, &button.text);
            button_entry(&format!("button_{number}"), button, &keys, false)
        })
        .collect();
    dialog_script(message, style, false, &entries, "")
}

/// The script that opens a dialog with one input and one button, which Enter
/// also presses.
fn text_dialog_script(message: &str, button: &ResolvedButton, style: &JqcDefaults) -> String {
    let entry = button_entry("formSubmit", button, &[], true);
    let on_ready = "this.$content.find('form.jqc_form').on('submit', function (e) { \
                    e.preventDefault(); jc.$$formSubmit.trigger('click'); });";
    dialog_script(message, style, true, &[entry], on_ready)
}

/// The script that opens a dialog with an input and buttons. Enter presses the
/// button only when there is one.
fn form_dialog_script(message: &str, buttons: &[ResolvedButton], style: &JqcDefaults) -> String {
    let entries: Vec<String> = buttons
        .iter()
        .enumerate()
        .map(|(index, button)| {
            let name = if buttons.len() == 1 {
                "formSubmit".to_owned()
            } else {
                format!("button_{}", index + 1)
            };
            button_entry(&name, button, &[], true)
        })
        .collect();
    let trigger = if buttons.len() == 1 {
        "jc.$$formSubmit.trigger('click');"
    } else {
        ""
    };
    let on_ready = format!(
        "this.$content.find('form.jqc_form').on('submit', function (e) {{ \
         e.preventDefault(); {trigger} }});"
    );
    dialog_script(message, style, true, &entries, &on_ready)
}

/// The kind of dialog a call opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Button,
    Text,
    Form,
}

/// Everything one call needs, before it runs.
struct Request<'a> {
    kind: Kind,
    message: &'a str,
    buttons: Vec<JqcButton>,
    options: Vec<JqcOption>,
    defaults: JqcDefaults,
}

/// A checked request: the script to run, what to focus, and the answer to
/// return where nobody watches.
struct Prepared {
    script: String,
    focus: &'static str,
    headless: Answer,
}

impl Request<'_> {
    /// Checks the arguments, in Python's order of complaint, and builds the
    /// script. Nothing is sent to a page.
    fn prepare(&self) -> Result<Prepared, SeleniumBaseError> {
        if self.kind != Kind::Text && self.buttons.is_empty() {
            return Err(invalid("List \"buttons\" requires at least one button!"));
        }
        let buttons = self
            .buttons
            .iter()
            .map(JqcButton::resolve)
            .collect::<Result<Vec<_>, _>>()?;
        let style = resolve_style(&self.defaults, &self.options)?;
        let last = buttons
            .last()
            .map(|button| button.text.clone())
            .unwrap_or_default();
        Ok(match self.kind {
            Kind::Button => Prepared {
                script: button_dialog_script(self.message, &buttons, &style),
                focus: FOCUS_BOX,
                headless: Answer {
                    button: last,
                    input: String::new(),
                },
            },
            Kind::Text => {
                let button = buttons.into_iter().next().unwrap_or(ResolvedButton {
                    text: "Submit".to_owned(),
                    color: JqcColor::Blue,
                });
                Prepared {
                    script: text_dialog_script(self.message, &button, &style),
                    focus: FOCUS_INPUT,
                    headless: Answer::default(),
                }
            }
            Kind::Form => Prepared {
                script: form_dialog_script(self.message, &buttons, &style),
                focus: FOCUS_INPUT,
                headless: Answer {
                    button: last,
                    input: String::new(),
                },
            },
        })
    }
}

/// Runs a request: checks it, and either answers it at once where nobody
/// watches or opens the dialog and waits for the person.
async fn ask(sb: &BaseCase, request: Request<'_>) -> Result<Answer, SeleniumBaseError> {
    let prepared = request.prepare()?;
    if sb.nobody_watches() {
        return Ok(prepared.headless);
    }
    sb.run_jqc_dialog(&prepared.script, prepared.focus).await
}

/// The builder methods every dialog call has for Python's `options`.
macro_rules! option_methods {
    () => {
        /// Python's `options`: `(name, value)` pairs, where the name is
        /// `theme`, `color` (the border) or `width`, such as
        /// `[("theme", "material"), ("width", "52%")]`. They apply to this
        /// dialog only, over the defaults [`BaseCase::set_jqc_theme`] stored.
        ///
        /// A name that is none of the three, and a value that is not valid for
        /// its name, fail the call when it runs.
        pub fn options<K, V>(mut self, options: impl IntoIterator<Item = (K, V)>) -> Self
        where
            K: Into<String>,
            V: Into<String>,
        {
            self.request.options.extend(
                options
                    .into_iter()
                    .map(|(name, value)| JqcOption::Named(name.into(), value.into())),
            );
            self
        }

        /// Sets the theme of this dialog, like `.options([("theme", ...)])`.
        pub fn theme(mut self, theme: JqcTheme) -> Self {
            self.request.options.push(JqcOption::Theme(theme));
            self
        }

        /// Sets the border colour of this dialog, like
        /// `.options([("color", ...)])`.
        pub fn color(mut self, color: JqcColor) -> Self {
            self.request.options.push(JqcOption::Color(color));
            self
        }

        /// Sets the width of this dialog, like `.options([("width", ...)])`.
        pub fn width(mut self, width: JqcWidth) -> Self {
            self.request.options.push(JqcOption::Width(width));
            self
        }
    };
}

/// The call of [`BaseCase::get_jqc_button_input`]: await it to get the text of
/// the button that was clicked.
#[must_use = "nothing happens until the call is awaited"]
pub struct JqcButtonInput<'a> {
    sb: &'a BaseCase,
    request: Request<'a>,
}

impl<'a> JqcButtonInput<'a> {
    pub(crate) fn new(
        sb: &'a BaseCase,
        message: &'a str,
        buttons: Vec<JqcButton>,
        defaults: JqcDefaults,
    ) -> Self {
        Self {
            sb,
            request: Request {
                kind: Kind::Button,
                message,
                buttons,
                options: Vec::new(),
                defaults,
            },
        }
    }

    option_methods!();
}

impl<'a> IntoFuture for JqcButtonInput<'a> {
    type Output = Result<String, SeleniumBaseError>;
    type IntoFuture = CallFuture<'a, String>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move { Ok(ask(self.sb, self.request).await?.button) })
    }
}

/// The call of [`BaseCase::get_jqc_text_input`]: await it to get the text typed
/// into the dialog's input.
#[must_use = "nothing happens until the call is awaited"]
pub struct JqcTextInput<'a> {
    sb: &'a BaseCase,
    request: Request<'a>,
}

impl<'a> JqcTextInput<'a> {
    pub(crate) fn new(sb: &'a BaseCase, message: &'a str, defaults: JqcDefaults) -> Self {
        Self {
            sb,
            request: Request {
                kind: Kind::Text,
                message,
                buttons: Vec::new(),
                options: Vec::new(),
                defaults,
            },
        }
    }

    /// Python's `button`: the one button that submits the input, a text or a
    /// text with a colour. The default is a blue `Submit`.
    pub fn button(mut self, button: impl Into<JqcButton>) -> Self {
        self.request.buttons = vec![button.into()];
        self
    }

    option_methods!();
}

impl<'a> IntoFuture for JqcTextInput<'a> {
    type Output = Result<String, SeleniumBaseError>;
    type IntoFuture = CallFuture<'a, String>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move { Ok(ask(self.sb, self.request).await?.input) })
    }
}

/// The call of [`BaseCase::get_jqc_form_inputs`]: await it to get the text of
/// the input and the text of the button that was clicked.
#[must_use = "nothing happens until the call is awaited"]
pub struct JqcFormInputs<'a> {
    sb: &'a BaseCase,
    request: Request<'a>,
}

impl<'a> JqcFormInputs<'a> {
    pub(crate) fn new(
        sb: &'a BaseCase,
        message: &'a str,
        buttons: Vec<JqcButton>,
        defaults: JqcDefaults,
    ) -> Self {
        Self {
            sb,
            request: Request {
                kind: Kind::Form,
                message,
                buttons,
                options: Vec::new(),
                defaults,
            },
        }
    }

    option_methods!();
}

impl<'a> IntoFuture for JqcFormInputs<'a> {
    type Output = Result<(String, String), SeleniumBaseError>;
    type IntoFuture = CallFuture<'a, (String, String)>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            let answer = ask(self.sb, self.request).await?;
            Ok((answer.input, answer.button))
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use serde_json::json;

    use super::*;
    use crate::api::base_case::webdriver_support::{MockWebDriver, Reply};
    use crate::browser::config::BrowserConfig;

    fn message_of<T: std::fmt::Debug>(result: Result<T, SeleniumBaseError>) -> String {
        match result {
            Err(SeleniumBaseError::InvalidConfig(message)) => message,
            other => panic!("expected an InvalidConfig, got {other:?}"),
        }
    }

    fn width(text: &str) -> Result<String, SeleniumBaseError> {
        text.parse::<JqcWidth>().map(|width| width.0)
    }

    /// `text` as it stands inside a JavaScript string literal of a script: the
    /// literal `quote` makes, without its quotes.
    fn literal(text: &str) -> String {
        let quoted = quote(text);
        quoted[1..quoted.len() - 1].to_owned()
    }

    fn resolved(text: &str, color: JqcColor) -> ResolvedButton {
        ResolvedButton {
            text: text.to_owned(),
            color,
        }
    }

    #[test]
    fn a_theme_is_read_in_any_case_and_a_bad_one_lists_the_choices() {
        assert_eq!("Material".parse::<JqcTheme>().unwrap(), JqcTheme::Material);
        assert_eq!("SUPERVAN".parse::<JqcTheme>().unwrap().as_str(), "supervan");
        for theme in JqcTheme::ALL {
            assert_eq!(theme.as_str().parse::<JqcTheme>().unwrap(), theme);
        }
        assert_eq!(
            message_of("Neon".parse::<JqcTheme>()),
            "neon is not a valid jquery-confirm theme! Select from ['bootstrap', 'modern', \
             'material', 'supervan', 'light', 'dark', 'seamless']"
        );
    }

    #[test]
    fn a_border_colour_and_a_button_colour_have_their_own_messages() {
        assert_eq!("Red".parse::<JqcColor>().unwrap(), JqcColor::Red);
        let list = "['blue', 'default', 'green', 'red', 'purple', 'orange', 'dark']";
        assert_eq!(
            message_of("pink".parse::<JqcColor>()),
            format!("pink is not a valid jquery-confirm border color! Select from {list}")
        );
        let button = JqcButton::from(("OK", "Pink")).resolve();
        assert_eq!(
            message_of(button),
            format!("pink is an invalid jquery-confirm button color!\nSelect from {list}")
        );
    }

    #[test]
    fn a_width_follows_pythons_rules() {
        // A whole number is a percent up to 100 and pixels above.
        assert_eq!(width("52").unwrap(), "52%");
        assert_eq!(width("100").unwrap(), "100%");
        assert_eq!(width("101").unwrap(), "101px");
        assert_eq!(width("450").unwrap(), "450px");
        // Anything else says its unit.
        assert_eq!(width("36.0%").unwrap(), "36.0%");
        assert_eq!(width("450px").unwrap(), "450px");
        assert_eq!(JqcWidth::default().as_str(), "38%");

        assert_eq!(
            message_of(width("0")),
            "Width must be set to a positive number!"
        );
        assert_eq!(
            message_of(width("00")),
            "Width must be set to a positive number!"
        );
        assert_eq!(
            message_of(width("36.5")),
            "jqc width must end with % for percent or px for pixels!"
        );
        assert_eq!(message_of(width("wide%")), "wide is not a numeric value!");
        assert_eq!(message_of(width("px")), " is not a numeric value!");
        assert_eq!(message_of(width("-5%")), "-5.0 is not a positive number!");
        assert_eq!(message_of(width("0.0px")), "0.0 is not a positive number!");
        // A value that is a number to Rust but not a width is refused too.
        assert_eq!(message_of(width("inf%")), "inf is not a numeric value!");
    }

    #[test]
    fn set_jqc_theme_changes_what_it_is_given_and_nothing_when_a_part_is_bad() {
        let defaults = JqcDefaults::default();
        assert_eq!(
            (defaults.theme, defaults.color, defaults.width.as_str()),
            (JqcTheme::Bootstrap, JqcColor::Blue, "38%")
        );

        let changed = defaults.changed("Modern", None, None).unwrap();
        assert_eq!(changed.theme, JqcTheme::Modern);
        assert_eq!(changed.color, JqcColor::Blue);
        assert_eq!(changed.width, JqcWidth::default());

        // An empty colour or width is none given.
        let kept = changed.changed("light", Some(""), Some("")).unwrap();
        assert_eq!(kept.color, JqcColor::Blue);

        let all = defaults
            .changed("material", Some("red"), Some("52"))
            .unwrap();
        assert_eq!(
            (all.theme, all.color, all.width.as_str()),
            (JqcTheme::Material, JqcColor::Red, "52%")
        );
        assert!(defaults.changed("material", Some("pink"), None).is_err());
        assert!(defaults.changed("neon", None, None).is_err());
        assert!(defaults.changed("material", None, Some("-1")).is_err());
    }

    #[test]
    fn options_apply_in_order_and_unknown_names_are_refused() {
        let defaults = JqcDefaults::default();
        let style = resolve_style(
            &defaults,
            &[
                JqcOption::Named("Theme".into(), "material".into()),
                JqcOption::Named("width".into(), "450".into()),
                JqcOption::Color(JqcColor::Green),
                JqcOption::Theme(JqcTheme::Dark),
            ],
        )
        .unwrap();
        assert_eq!(style.theme, JqcTheme::Dark);
        assert_eq!(style.color, JqcColor::Green);
        assert_eq!(style.width.as_str(), "450px");

        let unknown = resolve_style(&defaults, &[JqcOption::Named("Shape".into(), "x".into())]);
        assert_eq!(message_of(unknown), "Unknown option: \"Shape\"");
        let bad = resolve_style(
            &defaults,
            &[JqcOption::Named("theme".into(), "neon".into())],
        );
        assert!(message_of(bad).contains("neon is not a valid jquery-confirm theme!"));
    }

    #[test]
    fn the_shortcut_keys_follow_pythons_rows() {
        assert_eq!(shortcut_keys(1, 1, "Anything"), ["enter", "y", "1"]);
        assert_eq!(shortcut_keys(2, 1, "Yes"), ["y", "1"]);
        assert_eq!(shortcut_keys(2, 2, "NO"), ["n", "2"]);
        assert_eq!(shortcut_keys(3, 3, "Maybe"), ["3"]);
        // Past nine buttons the number is no key, and Yes and No keep theirs.
        assert!(shortcut_keys(12, 10, "Ten").is_empty());
        assert_eq!(shortcut_keys(12, 11, "yes"), ["y"]);
    }

    #[test]
    fn the_overlay_is_as_dark_as_the_theme_says() {
        let opacities: Vec<_> = JqcTheme::ALL
            .iter()
            .map(|theme| (theme.as_str(), theme.overlay_opacity()))
            .collect();
        assert_eq!(
            opacities,
            [
                ("bootstrap", "0.64"),
                ("modern", "0.5"),
                ("material", "0.4"),
                ("supervan", "0.56"),
                ("light", "0.32"),
                ("dark", "0.32"),
                ("seamless", "0.32"),
            ]
        );
    }

    #[test]
    fn a_button_dialog_script_carries_the_style_the_buttons_and_the_keys() {
        let style = JqcDefaults {
            theme: JqcTheme::Material,
            color: JqcColor::Red,
            width: "52%".parse().unwrap(),
        };
        let buttons = [
            resolved("Yes", JqcColor::Green),
            resolved("No", JqcColor::Red),
        ];
        let script = button_dialog_script("Go on?", &buttons, &style);

        assert!(script.starts_with("window.__sbJqcAnswer = null; jconfirm({"));
        assert!(script.contains(r#"boxWidth: "52%""#), "{script}");
        assert!(script.contains("bgOpacity: 0.4,"), "{script}");
        assert!(script.contains(r#"type: "red""#), "{script}");
        assert!(script.contains(r#"theme: "material""#), "{script}");
        assert!(
            script.contains("button_1: { btnClass: \"btn-green\""),
            "{script}"
        );
        assert!(
            script.contains("button_2: { btnClass: \"btn-red\""),
            "{script}"
        );
        assert!(script.contains(r#"keys: ["y", "1"]"#), "{script}");
        assert!(script.contains(r#"keys: ["n", "2"]"#), "{script}");
        // The button text is shown in bold and answered as it was written.
        assert!(
            script.contains("\\u003cb\\u003eYes\\u003c/b\\u003e"),
            "{script}"
        );
        assert!(script.contains(r#"var button = "Yes";"#), "{script}");
        assert!(script.contains("window.__sbJqcAnswer = { button: button, input: input };"));
        assert!(script.contains("jconfirm.lastButtonText = button;"));
    }

    #[test]
    fn what_the_caller_passes_stays_inside_its_string() {
        let message = "it's \"quoted\" </script><b>bold</b>";
        let button = resolved("Don't <i>\"go\"</i>", JqcColor::Blue);
        let script = button_dialog_script(message, &[button], &JqcDefaults::default());

        // No raw tag end: the script cannot close an element it is embedded in.
        assert!(!script.contains("</script"), "{script}");
        assert!(
            script.contains("it's \\\"quoted\\\" \\u003c/script\\u003e"),
            "{script}"
        );
        // The message is HTML (Python parity), the button text is escaped.
        assert!(
            script.contains("\\u003cb\\u003ebold\\u003c/b\\u003e"),
            "{script}"
        );
        assert!(
            script.contains(&literal("Don&#39;t &lt;i&gt;&quot;go&quot;&lt;/i&gt;")),
            "{script}"
        );
        // The answer is the text as written.
        assert!(script.contains(r#"var button = "Don't \u003ci\u003e\"go\"\u003c/i\u003e";"#));
    }

    #[test]
    fn a_text_dialog_has_the_form_one_button_and_an_enter_that_submits() {
        let script = text_dialog_script(
            "Name?",
            &resolved("Submit", JqcColor::Blue),
            &JqcDefaults::default(),
        );
        assert!(script.contains("class=\\\"jqc_form\\\""), "{script}");
        assert!(script.contains("class=\\\"jqc_input\\\""), "{script}");
        assert!(
            script.contains("formSubmit: { btnClass: \"btn-blue\""),
            "{script}"
        );
        assert!(
            script.contains("this.$content.find('.jqc_input').val()"),
            "{script}"
        );
        assert!(
            script.contains("jc.$$formSubmit.trigger('click');"),
            "{script}"
        );
        assert!(script.contains("bgOpacity: 0.64,"), "{script}");
    }

    #[test]
    fn a_form_dialog_submits_on_enter_only_with_one_button() {
        let style = JqcDefaults::default();
        let one = form_dialog_script("Why?", &[resolved("OK", JqcColor::Blue)], &style);
        assert!(one.contains("formSubmit: {"), "{one}");
        assert!(one.contains("jc.$$formSubmit.trigger('click');"), "{one}");

        let two = form_dialog_script(
            "Why?",
            &[
                resolved("OK", JqcColor::Blue),
                resolved("Cancel", JqcColor::Red),
            ],
            &style,
        );
        assert!(
            two.contains("button_1: {") && two.contains("button_2: {"),
            "{two}"
        );
        assert!(!two.contains("$$formSubmit"), "{two}");
        assert!(two.contains("e.preventDefault();"), "{two}");
    }

    fn case_without_session(headless: bool) -> BaseCase {
        BaseCase::without_session(BrowserConfig::default().with_headless(headless))
    }

    #[tokio::test]
    async fn where_nobody_watches_the_calls_return_pythons_values_and_send_nothing() {
        let driver = MockWebDriver::start(json!({}), |_| Reply::Value(Value::Null));
        let mut config = BrowserConfig::default()
            .with_headless(true)
            .with_webdriver_url(driver.url().to_owned());
        config.auto_start_driver = false;
        let sb = BaseCase::new(config).await.unwrap();

        let button = sb
            .get_jqc_button_input("Sure?", [("A", "red"), ("B", "green")])
            .await
            .unwrap();
        assert_eq!(button, "B");
        let text = sb.get_jqc_text_input("Name?").await.unwrap();
        assert_eq!(text, "");
        let form = sb
            .get_jqc_form_inputs("Why?", ["First", "Last"])
            .await
            .unwrap();
        assert_eq!(form, (String::new(), "Last".to_owned()));

        let scripted: Vec<_> = driver
            .all_requests()
            .into_iter()
            .filter(|request| request.path.starts_with("/execute"))
            .collect();
        assert!(scripted.is_empty(), "{scripted:?}");
    }

    #[tokio::test]
    async fn the_arguments_are_checked_even_where_nobody_watches() {
        let sb = case_without_session(true);
        let none: [&str; 0] = [];
        let empty = sb.get_jqc_button_input("?", none).await;
        assert_eq!(
            message_of(empty),
            "List \"buttons\" requires at least one button!"
        );
        let empty = sb.get_jqc_form_inputs("?", none).await;
        assert!(message_of(empty).contains("requires at least one button"));

        let colour = sb.get_jqc_button_input("?", [("A", "pink")]).await;
        assert!(message_of(colour).contains("pink is an invalid jquery-confirm button color!"));
        let colour = sb.get_jqc_text_input("?").button(("Go", "pink")).await;
        assert!(message_of(colour).contains("invalid jquery-confirm button color"));
        let option = sb
            .get_jqc_text_input("?")
            .options([("shape", "round")])
            .await;
        assert_eq!(message_of(option), "Unknown option: \"shape\"");
    }

    #[test]
    fn the_futures_of_the_calls_are_send() {
        fn assert_send<T: Send>(_: &T) {}
        let sb = case_without_session(true);
        assert_send(&sb.get_jqc_button_input("m", ["a"]).into_future());
        assert_send(&sb.get_jqc_text_input("m").into_future());
        assert_send(&sb.get_jqc_form_inputs("m", ["a"]).into_future());
    }

    /// A browser that shows a dialog: it answers the library loads, reports the
    /// `counts` of open dialogs one poll at a time (the last one stays) and
    /// returns `answer` when asked for it. The scripts it was sent are
    /// recorded by the driver.
    fn dialog_driver(counts: Vec<i64>, answer: Value) -> MockWebDriver {
        let polls = AtomicUsize::new(0);
        MockWebDriver::start(json!({}), move |request| {
            let script = request.body["script"].as_str().unwrap_or_default();
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/execute/async") => Reply::Value(json!(true)),
                ("POST", "/execute/sync") if script == OPEN_COUNT_SCRIPT => {
                    let poll = polls.fetch_add(1, Ordering::SeqCst);
                    Reply::Value(json!(counts[poll.min(counts.len() - 1)]))
                }
                ("POST", "/execute/sync") if script == ANSWER_SCRIPT => {
                    Reply::Value(answer.clone())
                }
                _ => Reply::Value(Value::Null),
            }
        })
    }

    async fn watched_case(driver: &MockWebDriver) -> BaseCase {
        let mut config = BrowserConfig::default()
            .with_headless(false)
            .with_webdriver_url(driver.url().to_owned());
        config.auto_start_driver = false;
        BaseCase::new(config).await.unwrap()
    }

    fn scripts(driver: &MockWebDriver) -> Vec<String> {
        driver
            .requests("/execute/sync")
            .iter()
            .filter_map(|request| request.body["script"].as_str().map(str::to_owned))
            .collect()
    }

    #[tokio::test]
    async fn a_watched_dialog_is_opened_polled_until_closed_and_answered() {
        let driver = dialog_driver(vec![1, 1, 0], json!({ "button": "B", "input": "typed" }));
        let sb = watched_case(&driver).await;

        let (text, button) = sb
            .get_jqc_form_inputs("Why?", [("A", "green"), ("B", "red")])
            .theme(JqcTheme::Seamless)
            .await
            .unwrap();
        assert_eq!((text.as_str(), button.as_str()), ("typed", "B"));

        let sent = scripts(&driver);
        let opened = sent
            .iter()
            .position(|script| script.contains("jconfirm({"))
            .expect("the dialog was opened");
        assert!(
            sent[opened].contains(r#"theme: "seamless""#),
            "{}",
            sent[opened]
        );
        // It focused the input, polled three times, then read the answer.
        assert_eq!(sent[opened + 1], FOCUS_INPUT);
        let polls = sent
            .iter()
            .filter(|script| *script == OPEN_COUNT_SCRIPT)
            .count();
        assert_eq!(polls, 3);
        assert_eq!(sent.last().map(String::as_str), Some(ANSWER_SCRIPT));
        // The libraries were loaded first.
        assert!(!driver.requests("/execute/async").is_empty());
    }

    #[tokio::test]
    async fn the_stored_defaults_are_what_a_dialog_is_built_from() {
        let driver = dialog_driver(vec![0], json!({ "button": "OK", "input": "" }));
        let mut sb = watched_case(&driver).await;
        sb.set_jqc_theme("Material", Some("red"), Some("52"))
            .unwrap();

        sb.get_jqc_button_input("Hi", ["OK"]).await.unwrap();
        let script = scripts(&driver)
            .into_iter()
            .find(|script| script.contains("jconfirm({"))
            .unwrap();
        assert!(script.contains(r#"theme: "material""#), "{script}");
        assert!(script.contains(r#"type: "red""#), "{script}");
        assert!(script.contains(r#"boxWidth: "52%""#), "{script}");

        // A call's own option wins for that call only.
        sb.get_jqc_button_input("Hi", ["OK"])
            .options([("theme", "dark")])
            .await
            .unwrap();
        sb.reset_jqc_theme();
        assert_eq!(*sb.jqc_defaults(), JqcDefaults::default());
        sb.get_jqc_button_input("Hi", ["OK"]).await.unwrap();
        let opened: Vec<_> = scripts(&driver)
            .into_iter()
            .filter(|script| script.contains("jconfirm({"))
            .collect();
        assert!(opened[1].contains(r#"theme: "dark""#));
        assert!(opened[2].contains(r#"theme: "bootstrap""#));
    }

    #[tokio::test]
    async fn a_dialog_that_goes_away_without_an_answer_is_an_error() {
        // The page navigated: jconfirm is gone.
        let driver = dialog_driver(vec![1, -1], Value::Null);
        let sb = watched_case(&driver).await;
        let error = sb.get_jqc_text_input("Name?").await;
        assert!(message_of(error).contains("closed without an answer"));

        // It closed, but stored nothing.
        let driver = dialog_driver(vec![0], Value::Null);
        let sb = watched_case(&driver).await;
        let error = sb.get_jqc_button_input("Sure?", ["OK"]).await;
        assert!(message_of(error).contains("closed without an answer"));
    }

    #[test]
    fn an_answer_is_read_from_what_the_page_stored() {
        let answer = Answer::from_value(&json!({ "button": "OK", "input": "x" })).unwrap();
        assert_eq!((answer.button.as_str(), answer.input.as_str()), ("OK", "x"));
        let button_only = Answer::from_value(&json!({ "button": "OK" })).unwrap();
        assert_eq!(button_only.input, "");
        assert!(Answer::from_value(&Value::Null).is_err());
        assert!(Answer::from_value(&json!("OK")).is_err());
    }
}
