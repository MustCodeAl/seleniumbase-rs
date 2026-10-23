//! Messenger alerts: the messages that `post_message` and its relatives show
//! on the page.
//!
//! A message is shown with the Messenger widget, as in Python. `post_message`,
//! `post_success_message` and `post_error_message` return a [`PostMessage`]: the
//! text is the required argument, and Python's keyword arguments `duration`,
//! `pause` and `style` are builder methods. Awaiting the builder shows the message.
//!
//! [`BaseCase::post_message`]: crate::api::base_case::BaseCase::post_message

use std::future::IntoFuture;
use std::time::Duration;

use crate::api::base_case::BaseCase;
use crate::api::call::CallFuture;
use crate::error::SeleniumBaseError;
use crate::sb_cdp::MessageStyle;

/// The line printed in place of a message that could not be shown, as Python
/// prints it: ` * INFO message: Saved`, with the style in capitals.
///
/// A page that cannot load Messenger (offline, or a policy that blocks the
/// scripts) cannot show the message, but the run still announces it.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::api::messages::fallback_line;
/// use seleniumbase_rs::sb_cdp::MessageStyle;
///
/// assert_eq!(fallback_line(MessageStyle::Error, "Not saved"), " * ERROR message: Not saved");
/// ```
#[must_use]
pub fn fallback_line(style: MessageStyle, message: &str) -> String {
    format!(" * {} message: {message}", style.as_str().to_uppercase())
}

/// A message to show in the page with Messenger, with Python's keyword
/// arguments as builder methods. Awaiting it shows the message.
///
/// It comes from [`BaseCase::post_message`], [`BaseCase::post_success_message`]
/// or [`BaseCase::post_error_message`]; the three differ in their starting
/// [`style`](Self::style) only.
///
/// - The message is HTML, as in Python: `<b>Saved</b>` shows in bold. Escape
///   text that is not yours.
/// - It stays for [`duration`](Self::duration), else for the run's message
///   duration (`RunSettings::with_message_duration`, 2.55 s unless set), and at
///   most 0.75 s where nobody watches (a headless browser or a virtual display).
/// - With [`pause`](Self::pause) on, the call returns 0.15 s after the message
///   has gone, so the next step does not start under it.
/// - When Messenger cannot be loaded into the page, the message is printed
///   instead (see [`fallback_line`]) and the call still pauses. Showing a
///   message never makes a test fail.
///
/// # Examples
///
/// ```no_run
/// use std::time::Duration;
/// use seleniumbase_rs::api::base_case::BaseCase;
/// use seleniumbase_rs::browser::config::BrowserConfig;
/// use seleniumbase_rs::sb_cdp::MessageStyle;
///
/// # async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
/// let sb = BaseCase::new(BrowserConfig::default()).await?;
/// sb.open("https://example.com").await?;
/// sb.post_message("Hello").await?;
/// sb.post_message("Quick <b>note</b>")
///     .duration(Duration::from_millis(800))
///     .pause(false)
///     .style(MessageStyle::Success)
///     .await?;
/// sb.post_error_message("Something is off").await?;
/// # Ok(())
/// # }
/// ```
#[must_use = "nothing happens until the call is awaited"]
pub struct PostMessage<'a> {
    pub(crate) sb: &'a BaseCase,
    pub(crate) message: &'a str,
    pub(crate) duration: Option<Duration>,
    pub(crate) pause: bool,
    pub(crate) style: MessageStyle,
}

impl<'a> PostMessage<'a> {
    /// A message of `style`, shown for the run's duration, with a pause.
    pub(crate) fn new(sb: &'a BaseCase, message: &'a str, style: MessageStyle) -> Self {
        Self {
            sb,
            message,
            duration: None,
            pause: true,
            style,
        }
    }

    /// How long the message stays. Python's `duration`; it defaults to the
    /// run's message duration.
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }

    /// Whether the call waits until the message has gone (and 0.15 s more)
    /// before it returns. Python's `pause`; on by default.
    pub fn pause(mut self, pause: bool) -> Self {
        self.pause = pause;
        self
    }

    /// The kind of message: info, success or error. Python's `style`.
    pub fn style(mut self, style: MessageStyle) -> Self {
        self.style = style;
        self
    }
}

impl<'a> IntoFuture for PostMessage<'a> {
    type Output = Result<(), SeleniumBaseError>;
    type IntoFuture = CallFuture<'a, ()>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            self.sb.show_messenger_message(&self).await;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::config::BrowserConfig;

    fn assert_send<T: Send>(_: &T) {}

    #[test]
    fn the_fallback_line_is_the_one_python_prints() {
        assert_eq!(
            fallback_line(MessageStyle::Info, "Saved"),
            " * INFO message: Saved"
        );
        assert_eq!(
            fallback_line(MessageStyle::Success, "Saved"),
            " * SUCCESS message: Saved"
        );
        assert_eq!(
            fallback_line(MessageStyle::Error, "a <b>tag</b>"),
            " * ERROR message: a <b>tag</b>"
        );
    }

    #[test]
    fn a_message_starts_with_a_pause_and_the_run_settings_duration() {
        let sb = BaseCase::without_session(BrowserConfig::default());
        let call = sb.post_message("Hello");
        assert_eq!(call.message, "Hello");
        assert_eq!(call.duration, None);
        assert!(call.pause);
        assert_eq!(call.style, MessageStyle::Info);
    }

    #[test]
    fn the_three_posts_differ_in_their_starting_style_only() {
        let sb = BaseCase::without_session(BrowserConfig::default());
        assert_eq!(sb.post_message("m").style, MessageStyle::Info);
        assert_eq!(sb.post_success_message("m").style, MessageStyle::Success);
        assert_eq!(sb.post_error_message("m").style, MessageStyle::Error);
    }

    #[test]
    fn the_keywords_are_builder_methods() {
        let sb = BaseCase::without_session(BrowserConfig::default());
        let call = sb
            .post_message("m")
            .duration(Duration::from_millis(40))
            .pause(false)
            .style(MessageStyle::Error);
        assert_eq!(call.duration, Some(Duration::from_millis(40)));
        assert!(!call.pause);
        assert_eq!(call.style, MessageStyle::Error);
    }

    #[test]
    fn the_future_of_a_message_is_send() {
        let sb = BaseCase::without_session(BrowserConfig::default());
        let future = sb.post_message("m").into_future();
        assert_send(&future);
    }
}
