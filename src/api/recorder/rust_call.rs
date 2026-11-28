//! The Rust call of a recorded command, and its replay.
//!
//! [`rust_call`] writes the line of `BaseCase` code that does what a
//! [`RecordedCommand`] does (`sb.method(args).await?;`), and [`replay`] runs the
//! same command on a [`BaseCase`]. Both match on every variant of the command,
//! so a variant added to [`RecordedCommand`] stops the build until it has its
//! call and its replay, and the two cannot disagree about which actions exist
//! or what their arguments mean. The replay is compiled against `BaseCase`, so
//! a method that changes its signature stops the build instead of failing in a
//! recorded test.
//!
//! A command that has no call (a CAPTCHA, an action the recording does not keep
//! enough of, a name nobody knows, arguments that are not readable) is the
//! reason the generators write after `// skipped:`; the replay counts it as
//! skipped or as a failed step.

use crate::SeleniumBaseError;
use crate::api::base_case::BaseCase;

use super::command::{ALIASES, CALLABLE_NAMES, CommandError, RecordedCommand};
use super::{
    RecordedAction, ReplayReport, ReplayStep, SECRET_PLACEHOLDER, literal, recorded_secret,
    revealed, text_argument,
};

/// The known action closest to a mistyped `name`, if one is a small edit away.
fn closest_action(name: &str) -> Option<&'static str> {
    let allowed = (name.chars().count() / 3).clamp(1, 3);
    CALLABLE_NAMES
        .iter()
        .copied()
        .chain(ALIASES.iter().map(|(alias, _)| *alias))
        .map(|known| (edit_distance(name, known), known))
        .filter(|(distance, _)| *distance <= allowed)
        .min_by_key(|(distance, _)| *distance)
        .map(|(_, known)| known)
}

/// The number of single-character insertions, deletions and substitutions that
/// turn `a` into `b`.
fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let above = row[j + 1];
            row[j + 1] = if ca == *cb {
                diagonal
            } else {
                1 + diagonal.min(above).min(row[j])
            };
            diagonal = above;
        }
    }
    row[b.len()]
}

/// What every generator writes, after `// skipped:`, for a `captcha` action:
/// there is no call to write, and the person is told how to go on. `solver` is
/// the call that tries the widget in the form being written.
pub(super) fn captcha_note(provider: Option<&str>, solver: &str) -> String {
    // The provider comes from the page; only its letters can reach a comment.
    let provider: String = provider
        .unwrap_or("")
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect();
    let provider = if provider.is_empty() {
        String::new()
    } else {
        format!(" ({provider})")
    };
    format!("captcha{provider}: solve it by hand or use {solver}")
}

/// Query parameters a site writes anew for each visit. An address recorded
/// with them may answer differently, or be refused, when it is opened again.
const VOLATILE_QUERY: [&str; 9] = [
    "ei", "sxsrf", "ved", "gs_lp", "gs_lcrp", "sca_esv", "sca_upv", "uact", "iflsig",
];

/// A note to write after the call that opens `url`, if the address is one that
/// a replay may not get through: Google's `/sorry/` bot check, or an address
/// with query parameters that change at every visit. Python records both
/// as they are seen (it cuts a search address at the first `&` only when a
/// click or Enter led to it), so the address stays and the note says why it
/// may fail.
fn url_note(url: &str) -> Option<String> {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let (location, query) = rest.split_once('?').unwrap_or((rest, ""));
    let path = location.find('/').map_or("", |at| &location[at..]);
    let query = query.split('#').next().unwrap_or("");
    let mut names: Vec<&str> = Vec::new();
    for pair in query.split('&') {
        let name = pair.split('=').next().unwrap_or("");
        if VOLATILE_QUERY.contains(&name) && !names.contains(&name) {
            names.push(name);
        }
    }
    let mut notes: Vec<String> = Vec::new();
    if path.starts_with("/sorry/") {
        notes.push(
            "a bot-check page (/sorry/): its address works once, so a replay cannot get past it"
                .to_owned(),
        );
    }
    if !names.is_empty() {
        notes.push(format!(
            "the query has parameters that change at every visit ({}); remove them if the page refuses a replay",
            names.join(", ")
        ));
    }
    (!notes.is_empty()).then(|| format!(" // note: {}", notes.join("; ")))
}

/// Why an action that is known has no call: `name: reason`.
fn no_call(name: &str, reason: &str) -> String {
    format!("{name}: {reason}")
}

/// The line of Rust that replays `command`, or why there is none.
#[expect(
    clippy::too_many_lines,
    reason = "one arm per kind of argument list reads best as one table"
)]
pub(super) fn rust_call(command: &RecordedCommand) -> Result<String, String> {
    use RecordedCommand as C;
    let mut method = command.name();
    // The builder methods after the call: `sb.click_with_offset(..).center(true)`.
    let mut keywords = "";
    // `let _ = ` before a call whose result is not needed.
    let mut discard = "";

    let args = match command {
        // The actions that have no call say why.
        C::Captcha { provider } => {
            return Err(captcha_note(provider.as_deref(), "sb.solve_captcha()"));
        }
        C::DragAndDropWithOffset { .. } => {
            return Err(no_call(
                method,
                "the recording keeps the selector but not the offset",
            ));
        }
        C::DownloadFile { .. } => {
            return Err(no_call(
                method,
                "a recorded download does not say which folder the file went to",
            ));
        }
        C::SaveAsPdfToLogs => {
            return Err(no_call(
                method,
                "BaseCase::save_as_pdf_to_logs needs a file name, which the recording does not hold",
            ));
        }
        C::SaveScreenshotToFolder { .. } => {
            return Err(no_call(
                method,
                "BaseCase::save_screenshot takes a file name only, not a folder",
            ));
        }
        C::Unknown { name, .. } => {
            return Err(match closest_action(name) {
                Some(near) => format!("unknown action {name:?}; did you mean {near:?}?"),
                None => format!("unknown action {name:?}"),
            });
        }
        C::Malformed { error, .. } => return Err(error.to_string()),
        // `sleep` returns nothing, so there is no `?`.
        C::Sleep { seconds } => return Ok(format!("sb.{method}({seconds:?}).await;")),

        // `sb.method()`
        C::SwitchToDefaultContent
        | C::SwitchToParentFrame
        | C::WaitForReadyStateComplete
        | C::MaximizeWindow
        | C::SwitchToNewWindow
        | C::SwitchToNewestWindow
        | C::SwitchToDefaultWindow
        | C::GoBack
        | C::GoForward
        | C::Refresh
        | C::DeleteAllCookies
        | C::ScrollToBottom
        | C::ScrollToTop
        | C::AcceptAlert
        | C::DismissAlert
        | C::ClearLocalStorage
        | C::CloseWindow
        | C::OpenNewWindow
        | C::SetContentToDefault
        | C::SetContentToParent
        | C::SaveScreenshotToLogs
        | C::SavePageSourceToLogs
        | C::ShowFileChoosers
        | C::ProcessDeferredAsserts
        | C::ActivateDemoMode
        | C::DeactivateDemoMode
        | C::ClearSessionStorage => String::new(),
        // `sb.method(10)`: a wait for an alert, with a ten-second timeout.
        C::WaitForAndAcceptAlert | C::WaitForAndDismissAlert => "10".to_owned(),
        // `sb.open_new_tab()`, or `sb.open_new_tab_url(url)` when the action
        // remembers the page it opened.
        C::OpenNewTab { url } => match url {
            Some(url) => {
                method = "open_new_tab_url";
                literal(url)
            }
            None => String::new(),
        },

        // `sb.method("selector")`
        C::Click { selector }
        | C::Clear { selector }
        | C::Submit { selector }
        | C::Hover { selector }
        | C::SelectAll { selector }
        | C::UnselectAll { selector }
        | C::CdpClick { selector }
        | C::DoubleClick { selector }
        | C::ContextClick { selector }
        | C::ScrollTo { selector }
        | C::ScrollIntoView { selector }
        | C::SmoothScrollTo { selector }
        | C::SlowScrollTo { selector }
        | C::CheckIfUnchecked { selector }
        | C::UncheckIfChecked { selector }
        | C::JsClick { selector }
        | C::JsClickIfPresent { selector }
        | C::JsClickAll { selector }
        | C::JqueryClick { selector }
        | C::JqueryClickAll { selector }
        | C::HighlightClick { selector }
        | C::ClickVisibleElements { selector }
        | C::SlowClick { selector }
        | C::HumanClick { selector }
        | C::UcClick { selector }
        | C::Highlight { selector }
        | C::WaitForElement { selector }
        | C::AssertElement { selector }
        | C::AssertElementPresent { selector }
        | C::AssertElementNotVisible { selector }
        | C::AssertNonEmptyText { selector }
        | C::DeferredAssertElement { selector }
        | C::DeferredAssertElementPresent { selector }
        | C::DeferredAssertNonEmptyText { selector } => literal(selector),
        // `let _ = sb.method("selector").await?;`: the result is not needed.
        C::FindElement { selector } | C::FindElements { selector } => {
            discard = "let _ = ";
            literal(selector)
        }
        // `sb.method("selector", 10)`: a wait, with a ten-second timeout.
        C::WaitForElementVisible { selector }
        | C::WaitForElementAbsent { selector }
        | C::WaitForElementNotVisible { selector }
        | C::WaitForElementPresent { selector }
        | C::WaitForElementClickable { selector } => format!("{}, 10", literal(selector)),
        // `sb.method("what")`: the first argument is not a selector.
        C::Open { url }
        | C::Goto { url }
        | C::GotoIfNotUrl { url }
        | C::AssertUrl { url }
        | C::AssertUrlContains { url } => literal(url),
        C::ClickLinkText { link_text }
        | C::ClickLink { link_text }
        | C::ClickPartialLink { link_text }
        | C::ClickPartialLinkText { link_text }
        | C::AssertLinkText { link_text } => literal(link_text),
        C::SwitchToFrame { frame } | C::SetContentToFrame { frame } => literal(frame),
        C::AssertTitle { title } | C::AssertTitleContains { title } => literal(title),
        C::SwitchToWindow { window } => literal(window),
        C::DeleteCookie { name }
        | C::AssertDownloadedFile { name }
        | C::SaveScreenshot { name, .. } => literal(name),
        C::TypeAlertText { text } => literal(text),
        C::RemoveLocalStorageItem { key } => literal(key),
        C::ClickXpath { xpath } => literal(xpath),

        // `sb.method("selector", text)`: a recorded password is read from the
        // environment.
        C::TypeText { selector, text }
        | C::SelectOptionByText { selector, text }
        | C::AddText { selector, text }
        | C::JsType { selector, text }
        | C::JsUpdateText { selector, text }
        | C::JqueryType { selector, text }
        | C::JqueryUpdateText { selector, text }
        | C::HumanType { selector, text }
        | C::UcType { selector, text }
        | C::UpdateText { selector, text } => {
            format!("{}, {}", literal(selector), text_argument(text))
        }
        C::SelectOptionByValue { selector, value } | C::SetValue { selector, value } => {
            format!("{}, {}", literal(selector), text_argument(value))
        }
        C::SendKeys { selector, keys } | C::PressKeys { selector, keys } => {
            format!("{}, {}", literal(selector), text_argument(keys))
        }
        C::EnterMfaCode { selector, code } => {
            format!("{}, {}", literal(selector), text_argument(code))
        }
        C::RemoveAttribute {
            selector,
            attribute,
        } => format!("{}, {}", literal(selector), text_argument(attribute)),
        C::ChooseFile { selector, path } => {
            format!("{}, {}", literal(selector), text_argument(path))
        }
        C::DragAndDrop {
            source,
            destination,
        } => format!("{}, {}", literal(source), text_argument(destination)),
        C::HoverAndClick {
            hover_selector,
            click_selector,
        } => format!(
            "{}, {}",
            literal(hover_selector),
            text_argument(click_selector)
        ),
        C::SetLocalStorageItem { key, value } => {
            format!("{}, {}", literal(key), text_argument(value))
        }
        C::AddCookie { name, value } => format!(
            "&seleniumbase_rs::Cookie::new({}, {})",
            literal(name),
            text_argument(value)
        ),
        // `sb.method("text", "selector")`: the method takes the text first.
        C::AssertText { text, selector }
        | C::AssertTextVisible { text, selector }
        | C::AssertTextNotVisible { text, selector }
        | C::AssertExactText { text, selector }
        | C::AssertExactTextNotVisible { text, selector }
        | C::DeferredAssertText { text, selector }
        | C::DeferredAssertExactText { text, selector } => {
            format!("{}, {}", literal(text), literal(selector))
        }
        // `sb.method("selector", "name", "content")`
        C::SetAttribute {
            selector,
            attribute,
            value,
        }
        | C::SetAttributes {
            selector,
            attribute,
            value,
        } => format!(
            "{}, {}, {}",
            literal(selector),
            literal(attribute.trim()),
            literal(value.trim())
        ),
        C::AssertAttribute {
            selector,
            attribute,
            content,
        } => {
            let Some(content) = content else {
                return Err(CommandError::NotNameContent {
                    action: method.to_owned(),
                    value: attribute.clone(),
                }
                .to_string());
            };
            format!(
                "{}, {}, {}",
                literal(selector),
                literal(attribute.trim()),
                literal(content.trim())
            )
        }
        C::SetWindowSize { width, height } => format!("{width}, {height}"),
        C::ClickWithOffset {
            selector,
            x,
            y,
            center,
        }
        | C::DoubleClickWithOffset {
            selector,
            x,
            y,
            center,
        } => {
            if *center {
                keywords = ".center(true)";
            }
            format!("{}, {x}, {y}", literal(selector))
        }
        C::SelectOptionByIndex { selector, index } => {
            format!("{}, {index}", literal(selector))
        }
    };

    // A page that a replay may not get through says so after its call.
    let note = match command {
        C::Open { url } | C::Goto { url } | C::GotoIfNotUrl { url } => {
            url_note(url).unwrap_or_default()
        }
        _ => String::new(),
    };
    Ok(format!(
        "{discard}sb.{method}({args}){keywords}.await?;{note}"
    ))
}

/// A call's outcome as the replay reports it: nothing, or the reason.
fn ran<T, E: std::fmt::Display>(result: Result<T, E>) -> Result<(), String> {
    result.map(drop).map_err(|error| error.to_string())
}

/// The text to type for a recorded `text`: the password for the placeholder of
/// one, as the reason when it is not set.
fn reveal(text: &str) -> Result<String, String> {
    revealed(text).map_err(|error| error.to_string())
}

/// Runs the command on `sb`: `Ok(true)` when it ran, `Ok(false)` when it has
/// no call to run (an unknown name, a CAPTCHA, an action the recording does not
/// keep enough of), and the reason when its arguments are not readable or the
/// call failed.
#[expect(
    clippy::too_many_lines,
    reason = "one arm per action reads best as one table"
)]
async fn call(sb: &mut BaseCase, command: &RecordedCommand) -> Result<bool, String> {
    use RecordedCommand as C;
    match command {
        C::Captcha { .. }
        | C::DragAndDropWithOffset { .. }
        | C::DownloadFile { .. }
        | C::SaveAsPdfToLogs
        | C::SaveScreenshotToFolder { .. }
        | C::Unknown { .. } => return Ok(false),
        C::Malformed { error, .. } => return Err(error.to_string()),

        C::Open { url } => ran(sb.open(url).await)?,
        C::Goto { url } => ran(sb.goto(url).await)?,
        C::GotoIfNotUrl { url } => ran(sb.goto_if_not_url(url).await)?,
        C::Click { selector } => ran(sb.click(selector).await)?,
        C::TypeText { selector, text } => ran(sb.type_text(selector, &reveal(text)?).await)?,
        C::Clear { selector } => ran(sb.clear(selector).await)?,
        C::ClickLinkText { link_text } => ran(sb.click_link_text(link_text).await)?,
        C::ClickLink { link_text } => ran(sb.click_link(link_text).await)?,
        C::ClickPartialLink { link_text } => ran(sb.click_partial_link(link_text).await)?,
        C::ClickPartialLinkText { link_text } => {
            ran(sb.click_partial_link_text(link_text).await)?;
        }
        C::Submit { selector } => ran(sb.submit(selector).await)?,
        C::Hover { selector } => ran(sb.hover(selector).await)?,
        C::SelectOptionByText { selector, text } => {
            ran(sb.select_option_by_text(selector, &reveal(text)?).await)?;
        }
        C::SelectOptionByValue { selector, value } => {
            ran(sb.select_option_by_value(selector, &reveal(value)?).await)?;
        }
        C::SelectOptionByIndex { selector, index } => {
            ran(sb.select_option_by_index(selector, *index).await)?;
        }
        C::SelectAll { selector } => ran(sb.select_all(selector).await)?,
        C::UnselectAll { selector } => ran(sb.unselect_all(selector).await)?,
        C::SwitchToFrame { frame } => ran(sb.switch_to_frame(frame).await)?,
        C::SwitchToDefaultContent => ran(sb.switch_to_default_content().await)?,
        C::SwitchToParentFrame => ran(sb.switch_to_parent_frame().await)?,
        C::DragAndDrop {
            source,
            destination,
        } => ran(sb.drag_and_drop(source, &reveal(destination)?).await)?,
        C::CdpClick { selector } => ran(sb.cdp_click(selector).await)?,
        C::WaitForElementVisible { selector } => {
            ran(sb.wait_for_element_visible(selector, 10).await)?;
        }
        C::WaitForElementAbsent { selector } => {
            ran(sb.wait_for_element_absent(selector, 10).await)?;
        }
        C::WaitForElementNotVisible { selector } => {
            ran(sb.wait_for_element_not_visible(selector, 10).await)?;
        }
        C::WaitForElementPresent { selector } => {
            ran(sb.wait_for_element_present(selector, 10).await)?;
        }
        C::WaitForElementClickable { selector } => {
            ran(sb.wait_for_element_clickable(selector, 10).await)?;
        }
        C::WaitForReadyStateComplete => ran(sb.wait_for_ready_state_complete().await)?,
        C::AssertText { text, selector } => ran(sb.assert_text(text, selector).await)?,
        C::AssertTextVisible { text, selector } => {
            ran(sb.assert_text_visible(text, selector).await)?;
        }
        C::AssertTextNotVisible { text, selector } => {
            ran(sb.assert_text_not_visible(text, selector).await)?;
        }
        C::AssertExactText { text, selector } => {
            ran(sb.assert_exact_text(text, selector).await)?;
        }
        C::AssertAttribute {
            selector,
            attribute,
            content,
        } => {
            let Some(content) = content else {
                return Err(CommandError::NotNameContent {
                    action: command.name().to_owned(),
                    value: attribute.clone(),
                }
                .to_string());
            };
            ran(sb
                .assert_attribute(selector, attribute.trim(), content.trim())
                .await)?;
        }
        C::AssertTitle { title } => ran(sb.assert_title(title).await)?,
        C::AssertTitleContains { title } => ran(sb.assert_title_contains(title).await)?,
        C::AssertLinkText { link_text } => ran(sb.assert_link_text(link_text).await)?,
        C::MaximizeWindow => ran(sb.maximize_window().await)?,
        C::SetWindowSize { width, height } => ran(sb.set_window_size(*width, *height).await)?,
        C::SwitchToWindow { window } => ran(sb.switch_to_window(window).await)?,
        C::SwitchToNewWindow => ran(sb.switch_to_new_window().await)?,
        C::SwitchToNewestWindow => ran(sb.switch_to_newest_window().await)?,
        C::SwitchToDefaultWindow => ran(sb.switch_to_default_window().await)?,
        C::GoBack => ran(sb.go_back().await)?,
        C::GoForward => ran(sb.go_forward().await)?,
        C::Refresh => ran(sb.refresh().await)?,
        C::DeleteAllCookies => ran(sb.delete_all_cookies().await)?,
        C::DeleteCookie { name } => ran(sb.delete_cookie(name).await)?,
        C::AddCookie { name, value } => {
            let cookie = crate::Cookie::new(name, reveal(value)?);
            ran(sb.add_cookie(&cookie).await)?;
        }
        C::DoubleClick { selector } => ran(sb.double_click(selector).await)?,
        C::ContextClick { selector } => ran(sb.context_click(selector).await)?,
        C::ClickWithOffset {
            selector,
            x,
            y,
            center,
        } => {
            ran(sb
                .click_with_offset(selector, *x as f64, *y as f64)
                .center(*center)
                .await)?;
        }
        C::DoubleClickWithOffset {
            selector,
            x,
            y,
            center,
        } => {
            ran(sb
                .double_click_with_offset(selector, *x as f64, *y as f64)
                .center(*center)
                .await)?;
        }
        C::ScrollTo { selector } => ran(sb.scroll_to(selector).await)?,
        C::ScrollToBottom => ran(sb.scroll_to_bottom().await)?,
        C::ScrollToTop => ran(sb.scroll_to_top().await)?,
        C::ScrollIntoView { selector } => ran(sb.scroll_into_view(selector).await)?,
        C::SmoothScrollTo { selector } => ran(sb.smooth_scroll_to(selector).await)?,
        C::SlowScrollTo { selector } => ran(sb.slow_scroll_to(selector).await)?,
        C::CheckIfUnchecked { selector } => ran(sb.check_if_unchecked(selector).await)?,
        C::UncheckIfChecked { selector } => ran(sb.uncheck_if_checked(selector).await)?,
        C::AddText { selector, text } => ran(sb.add_text(selector, &reveal(text)?).await)?,
        C::SendKeys { selector, keys } => ran(sb.send_keys(selector, &reveal(keys)?).await)?,
        C::JsClick { selector } => ran(sb.js_click(selector).await)?,
        C::JsType { selector, text } => ran(sb.js_type(selector, &reveal(text)?).await)?,
        C::JsUpdateText { selector, text } => {
            ran(sb.js_update_text(selector, &reveal(text)?).await)?;
        }
        C::JsClickIfPresent { selector } => ran(sb.js_click_if_present(selector).await)?,
        C::JsClickAll { selector } => ran(sb.js_click_all(selector).await)?,
        C::JqueryClick { selector } => ran(sb.jquery_click(selector).await)?,
        C::JqueryClickAll { selector } => ran(sb.jquery_click_all(selector).await)?,
        C::JqueryType { selector, text } => {
            ran(sb.jquery_type(selector, &reveal(text)?).await)?;
        }
        C::JqueryUpdateText { selector, text } => {
            ran(sb.jquery_update_text(selector, &reveal(text)?).await)?;
        }
        C::SetAttribute {
            selector,
            attribute,
            value,
        } => ran(sb
            .set_attribute(selector, attribute.trim(), value.trim())
            .await)?,
        C::RemoveAttribute {
            selector,
            attribute,
        } => ran(sb.remove_attribute(selector, &reveal(attribute)?).await)?,
        C::ChooseFile { selector, path } => {
            ran(sb.choose_file(selector, &reveal(path)?).await)?;
        }
        C::WaitForAndAcceptAlert => ran(sb.wait_for_and_accept_alert(10).await)?,
        C::WaitForAndDismissAlert => ran(sb.wait_for_and_dismiss_alert(10).await)?,
        C::AcceptAlert => ran(sb.accept_alert().await)?,
        C::DismissAlert => ran(sb.dismiss_alert().await)?,
        C::TypeAlertText { text } => ran(sb.type_alert_text(text).await)?,
        C::ClearLocalStorage => ran(sb.clear_local_storage().await)?,
        C::RemoveLocalStorageItem { key } => ran(sb.remove_local_storage_item(key).await)?,
        C::SetLocalStorageItem { key, value } => {
            ran(sb.set_local_storage_item(key, &reveal(value)?).await)?;
        }
        C::CloseWindow => ran(sb.close_window().await)?,
        C::HighlightClick { selector } => ran(sb.highlight_click(selector).await)?,
        C::OpenNewWindow => ran(sb.open_new_window().await)?,
        C::ClickVisibleElements { selector } => {
            ran(sb.click_visible_elements(selector).await)?;
        }
        C::FindElement { selector } => ran(sb.find_element(selector).await)?,
        C::FindElements { selector } => ran(sb.find_elements(selector).await)?,
        C::SlowClick { selector } => ran(sb.slow_click(selector).await)?,
        C::HumanType { selector, text } => {
            ran(sb.human_type(selector, &reveal(text)?).await)?;
        }
        C::HumanClick { selector } => ran(sb.human_click(selector).await)?,
        C::UcClick { selector } => ran(sb.uc_click(selector).await)?,
        C::UcType { selector, text } => ran(sb.uc_type(selector, &reveal(text)?).await)?,
        C::UpdateText { selector, text } => {
            ran(sb.update_text(selector, &reveal(text)?).await)?;
        }
        C::Sleep { seconds } => sb.sleep(*seconds).await,
        C::ClickXpath { xpath } => ran(sb.click_xpath(xpath).await)?,
        C::Highlight { selector } => ran(sb.highlight(selector).await)?,
        C::PressKeys { selector, keys } => ran(sb.press_keys(selector, &reveal(keys)?).await)?,
        C::EnterMfaCode { selector, code } => {
            ran(sb.enter_mfa_code(selector, &reveal(code)?).await)?;
        }
        C::HoverAndClick {
            hover_selector,
            click_selector,
        } => ran(sb
            .hover_and_click(hover_selector, &reveal(click_selector)?)
            .await)?,
        C::SetValue { selector, value } => {
            ran(sb.set_value(selector, &reveal(value)?).await)?;
        }
        C::SetContentToFrame { frame } => ran(sb.set_content_to_frame(frame).await)?,
        C::SetContentToDefault => ran(sb.set_content_to_default().await)?,
        C::SetContentToParent => ran(sb.set_content_to_parent().await)?,
        C::WaitForElement { selector } => ran(sb.wait_for_element(selector).await)?,
        C::AssertElement { selector } => ran(sb.assert_element(selector).await)?,
        C::AssertElementPresent { selector } => ran(sb.assert_element_present(selector).await)?,
        C::AssertElementNotVisible { selector } => {
            ran(sb.assert_element_not_visible(selector).await)?;
        }
        C::AssertNonEmptyText { selector } => ran(sb.assert_non_empty_text(selector).await)?,
        C::DeferredAssertElement { selector } => {
            ran(sb.deferred_assert_element(selector).await)?;
        }
        C::DeferredAssertElementPresent { selector } => {
            ran(sb.deferred_assert_element_present(selector).await)?;
        }
        C::DeferredAssertNonEmptyText { selector } => {
            ran(sb.deferred_assert_non_empty_text(selector).await)?;
        }
        C::DeferredAssertText { text, selector } => {
            ran(sb.deferred_assert_text(text, selector).await)?;
        }
        C::DeferredAssertExactText { text, selector } => {
            ran(sb.deferred_assert_exact_text(text, selector).await)?;
        }
        C::AssertExactTextNotVisible { text, selector } => {
            ran(sb.assert_exact_text_not_visible(text, selector).await)?;
        }
        C::SetAttributes {
            selector,
            attribute,
            value,
        } => ran(sb
            .set_attributes(selector, attribute.trim(), value.trim())
            .await)?,
        C::AssertUrl { url } => ran(sb.assert_url(url).await)?,
        C::AssertUrlContains { url } => ran(sb.assert_url_contains(url).await)?,
        C::AssertDownloadedFile { name } => ran(sb.assert_downloaded_file(name).await)?,
        C::SaveScreenshot { name, .. } => ran(sb.save_screenshot(name).await)?,
        C::SaveScreenshotToLogs => ran(sb.save_screenshot_to_logs().await)?,
        C::SavePageSourceToLogs => ran(sb.save_page_source_to_logs().await)?,
        C::ShowFileChoosers => ran(sb.show_file_choosers().await)?,
        C::ProcessDeferredAsserts => ran(sb.process_deferred_asserts().await)?,
        C::ActivateDemoMode => ran(sb.activate_demo_mode().await)?,
        C::DeactivateDemoMode => ran(sb.deactivate_demo_mode().await)?,
        C::ClearSessionStorage => ran(sb.clear_session_storage().await)?,
        C::OpenNewTab { url } => match url {
            Some(url) => ran(sb.open_new_tab_url(url).await)?,
            None => ran(sb.open_new_tab().await)?,
        },
    }
    Ok(true)
}

/// Replays `actions` on `sb`, one step each: see
/// [`RecorderSession::replay`](super::RecorderSession::replay).
pub(super) async fn replay(
    sb: &mut BaseCase,
    actions: &[RecordedAction],
) -> Result<ReplayReport, SeleniumBaseError> {
    // A recording that types a password needs it before the browser moves.
    if actions.iter().any(|action| {
        action
            .command
            .value()
            .is_some_and(|value| value.starts_with(SECRET_PLACEHOLDER))
    }) {
        recorded_secret()?;
    }

    let mut report = ReplayReport::default();
    for action in actions {
        let outcome = call(sb, &action.command).await;
        if outcome == Ok(false) {
            report.skipped += 1;
            report.skipped_names.push(action.command.name().to_owned());
            continue;
        }
        report.steps.push(ReplayStep {
            name: action.command.name().to_owned(),
            description: action.command.to_string(),
            error: outcome.err(),
        });
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::api::base_case::webdriver_support::{MockWebDriver, Reply, element};
    use crate::api::recorder::RecorderSession;
    use crate::browser::config::BrowserConfig;

    /// The command a recorded name, target and value stand for.
    fn command(name: &str, target: Option<&str>, value: Option<&str>) -> RecordedCommand {
        RecordedCommand::from_parts(name, target, value)
    }

    fn action(name: &str, target: Option<&str>, value: Option<&str>) -> RecordedAction {
        RecordedAction::new(command(name, target, value))
    }

    /// A WebDriver that finds every element and answers a few reads.
    fn driver() -> MockWebDriver {
        MockWebDriver::start(json!({}), |request| {
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/element") => Reply::Value(element("e1")),
                ("POST", "/elements") => Reply::Value(json!([element("e1")])),
                ("GET", "/url") => Reply::Value(json!("http://x.test/")),
                ("GET", "/title") => Reply::Value(json!("Title")),
                ("GET", "/window/handles") => Reply::Value(json!(["h1", "h2"])),
                ("POST", "/window/new") => Reply::Value(json!({ "handle": "h2", "type": "tab" })),

                ("GET", "/element/e1/rect") => {
                    Reply::Value(json!({ "x": 10.0, "y": 20.0, "width": 100.0, "height": 50.0 }))
                }

                // Selenium's `getAttribute` page script, which `get_attribute`
                // runs in place of the driver's attribute command.
                ("POST", "/execute/sync")
                    if request.body["script"]
                        .as_str()
                        .is_some_and(|script| script.contains("/* getAttribute */")) =>
                {
                    Reply::Value(json!("/x"))
                }

                ("GET", path) if path.contains("/attribute/") || path.contains("/property/") => {
                    Reply::Value(json!("/x"))
                }

                ("GET", "/element/e1/text") => Reply::Value(json!("Hello")),
                ("GET", path) if path.ends_with("/displayed") || path.ends_with("/enabled") => {
                    Reply::Value(json!(true))
                }

                _ => Reply::Value(Value::Null),
            }
        })
    }

    fn session_less() -> BaseCase {
        BaseCase::without_session(BrowserConfig::default())
    }

    async fn replay_one(
        sb: &mut BaseCase,
        name: &str,
        target: Option<&str>,
        value: Option<&str>,
    ) -> ReplayReport {
        let action = action(name, target, value);
        replay(sb, &[action]).await.expect("no password in this")
    }

    /// The commands sent to the driver since `before` requests, as
    /// `(method, path)`.
    fn sent_since(driver: &MockWebDriver, before: usize) -> Vec<(String, String)> {
        driver.all_requests()[before..]
            .iter()
            .map(|request| (request.method.clone(), request.path.clone()))
            .collect()
    }

    #[tokio::test]
    async fn every_name_that_needs_an_argument_is_a_step_and_none_is_skipped() {
        let mut sb = session_less();
        for name in CALLABLE_NAMES {
            // The others take no argument and act on the browser at once;
            // `each_kind_of_action_...` replays a number of them against a driver.
            if !matches!(command(name, None, None), RecordedCommand::Malformed { .. }) {
                continue;
            }
            let report = replay_one(&mut sb, name, None, None).await;
            assert_eq!(
                (report.steps.len(), report.skipped),
                (1, 0),
                "{name} is known, so it is a step"
            );
        }
    }

    #[tokio::test]
    async fn an_action_that_lacks_an_argument_is_a_failed_step_that_names_it() {
        let mut sb = session_less();
        for (name, target, value, reason) in [
            ("click", None, None, "click has no target"),
            ("type_text", Some("#a"), None, "type_text has no value"),
            ("type_text", None, Some("x"), "type_text has no target"),
            ("type", None, Some("x"), "type_text has no target"),
            ("assert_text", Some("#a"), None, "assert_text has no value"),
            ("assert_text", None, Some("Hi"), "assert_text has no target"),
            (
                "wait_for_element_visible",
                None,
                None,
                "wait_for_element_visible has no target",
            ),
            ("sleep", None, None, "sleep has no value"),
            (
                "assert_attribute",
                Some("#a"),
                Some("no equals sign"),
                // The Gherkin step takes an attribute alone; a call needs a content.
                "assert_attribute: value \"no equals sign\" is not name=content",
            ),
            (
                "set_window_size",
                Some("wide,tall"),
                None,
                "set_window_size: width \"wide\" is not a whole number in range",
            ),
            (
                "click_with_offset",
                Some("#a"),
                Some("1"),
                "click_with_offset: value \"1\" is not x, y",
            ),
            (
                "select_option_by_index",
                Some("#a"),
                Some("two"),
                "select_option_by_index: index \"two\" is not a whole number in range",
            ),
            (
                "sleep",
                None,
                Some("soon"),
                "sleep: \"soon\" is not a number of seconds",
            ),
        ] {
            let report = replay_one(&mut sb, name, target, value).await;
            let step = &report.steps[0];
            assert_eq!(step.error.as_deref(), Some(reason), "{name}");
            assert_eq!(step.name, name, "the step keeps the recorded name");
            assert!(!report.is_success());
        }
    }

    #[tokio::test]
    async fn a_name_the_table_does_not_know_is_skipped_and_named() {
        let mut sb = session_less();
        let actions = [
            action("levitate", Some("#a"), None),
            // The recording does not keep what this one needs.
            action("drag_and_drop_with_offset", Some("#a"), Some("#b")),
            action("download_file", Some("http://x.test/f"), None),
            action("click", None, None),
        ];
        let report = replay(&mut sb, &actions).await.unwrap();
        assert_eq!(report.skipped, 3);
        assert_eq!(
            report.skipped_names,
            ["levitate", "drag_and_drop_with_offset", "download_file"]
        );
        assert_eq!(report.steps.len(), 1);
        assert!(report.skipped_names.iter().all(|name| name != "click"));
    }

    #[tokio::test]
    async fn each_kind_of_action_sends_the_command_that_does_it() {
        let driver = driver();
        let mut sb = driver.base_case().await;
        let cases = [
            ("open", Some("http://x.test/a"), None, "POST", "/url"),
            ("navigate", Some("http://x.test/a"), None, "POST", "/url"),
            ("goto", Some("http://x.test/a"), None, "POST", "/url"),
            (
                "goto_if_not_url",
                Some("http://x.test/b"),
                None,
                "POST",
                "/url",
            ),
            ("click", Some("#a"), None, "POST", "/element/e1/click"),
            (
                "type_text",
                Some("#a"),
                Some("hi"),
                "POST",
                "/element/e1/value",
            ),
            ("type", Some("#a"), Some("hi"), "POST", "/element/e1/value"),
            ("clear", Some("#a"), None, "POST", "/element/e1/clear"),
            ("go_back", None, None, "POST", "/back"),
            ("go_forward", None, None, "POST", "/forward"),
            ("refresh", None, None, "POST", "/refresh"),
            ("maximize_window", None, None, "POST", "/window/maximize"),
            (
                "set_window_size",
                Some("800,600"),
                None,
                "POST",
                "/window/rect",
            ),
            ("delete_all_cookies", None, None, "DELETE", "/cookie"),
            ("accept_alert", None, None, "POST", "/alert/accept"),
            ("dismiss_alert", None, None, "POST", "/alert/dismiss"),
            ("switch_to_default_content", None, None, "POST", "/frame"),
            (
                "switch_to_parent_frame",
                None,
                None,
                "POST",
                "/frame/parent",
            ),
            ("assert_title", Some("Title"), None, "GET", "/title"),
            (
                "assert_text",
                Some("#a"),
                Some("Hello"),
                "GET",
                "/element/e1/text",
            ),
            ("js_click", Some("#a"), None, "POST", "/execute/sync"),
            ("scroll_to_top", None, None, "POST", "/execute/sync"),
            ("hover", Some("#a"), None, "POST", "/actions"),
            ("find_element", Some("#a"), None, "POST", "/element"),
            (
                "wait_for_element_present",
                Some("#a"),
                None,
                "POST",
                "/element",
            ),
            (
                "wait_for_element_visible",
                Some("#a"),
                None,
                "GET",
                "/element/e1/displayed",
            ),
            (
                "click_with_offset",
                Some("#a"),
                Some("5, -7"),
                "POST",
                "/actions",
            ),
            (
                "assert_attribute",
                Some("#a"),
                Some("href=/x"),
                "POST",
                "/execute/sync",
            ),
            ("open_new_tab", None, None, "POST", "/execute/sync"),
            (
                "open_new_tab",
                Some("http://x.test/n"),
                None,
                "POST",
                "/url",
            ),
            ("sleep", None, Some("0"), "", ""),
        ];
        for (name, target, value, method, path) in cases {
            let before = driver.all_requests().len();
            let report = replay_one(&mut sb, name, target, value).await;
            let step = &report.steps[0];
            assert!(step.passed(), "{name}: {:?}", step.error);
            let sent = sent_since(&driver, before);
            if path.is_empty() {
                assert!(sent.is_empty(), "{name} sent {sent:?}");
            } else {
                assert!(
                    sent.contains(&(method.to_owned(), path.to_owned())),
                    "{name} should send {method} {path}, sent {sent:?}"
                );
            }
        }
    }

    #[tokio::test]
    async fn the_arguments_reach_the_command_in_the_order_the_method_takes_them() {
        let driver = driver();
        let mut sb = driver.base_case().await;
        replay_one(&mut sb, "assert_text", Some("#msg"), Some("Hello")).await;
        replay_one(&mut sb, "set_window_size", Some("800,600"), None).await;
        replay_one(&mut sb, "type_text", Some("#a"), Some("typed")).await;
        replay_one(&mut sb, "open", Some("http://x.test/a"), None).await;

        let found: Vec<_> = driver
            .all_requests()
            .into_iter()
            .filter(|request| request.body["value"] == "#msg")
            .collect();
        assert!(
            !found.is_empty(),
            "assert_text(text, css): css is the target"
        );
        assert_eq!(driver.requests("/window/rect")[0].body["width"], 800);
        assert_eq!(driver.requests("/window/rect")[0].body["height"], 600);
        assert_eq!(
            driver.requests("/element/e1/value")[0].body["text"],
            "typed"
        );
        assert_eq!(driver.requests("/url")[0].body["url"], "http://x.test/a");
    }

    #[tokio::test]
    async fn goto_if_not_url_stays_on_the_page_it_is_on() {
        let driver = driver();
        let mut sb = driver.base_case().await;
        // The driver says the browser is at http://x.test/.
        let before = driver.all_requests().len();
        replay_one(&mut sb, "goto_if_not_url", Some("http://x.test/"), None).await;
        let sent = sent_since(&driver, before);
        assert!(
            !sent.contains(&("POST".to_owned(), "/url".to_owned())),
            "{sent:?}"
        );
    }

    #[tokio::test]
    async fn a_saved_recording_replays_from_json_and_from_yaml() {
        let driver = driver();
        let mut sb = driver.base_case().await;
        let mut session = RecorderSession::new();
        session.push(command("goto", Some("http://x.test/a"), None));
        session.push(command("type", Some("#q"), Some("rust \"lang\"\n")));
        session.push(command("click", Some("#go"), None));
        let folder = tempfile::tempdir().unwrap();

        for file in ["rec.json", "rec.yaml"] {
            let path = session.save(&folder.path().join(file)).unwrap();
            let actions = RecorderSession::load(&path).unwrap();
            let before = driver.all_requests().len();

            let report = replay(&mut sb, &actions).await.unwrap();

            assert!(report.is_success(), "{file}: {report:?}");
            assert_eq!((report.steps.len(), report.skipped), (3, 0), "{file}");
            let sent = sent_since(&driver, before);
            for expected in [
                ("POST", "/url"),
                ("POST", "/element/e1/value"),
                ("POST", "/element/e1/click"),
            ] {
                assert!(
                    sent.contains(&(expected.0.to_owned(), expected.1.to_owned())),
                    "{file}: {expected:?} not in {sent:?}"
                );
            }
        }
        let typed = driver.requests("/element/e1/value");
        assert_eq!(typed[0].body["text"], "rust \"lang\"\n");
    }

    #[tokio::test]
    async fn a_recording_with_a_password_needs_it_before_the_browser_moves() {
        if std::env::var(super::super::SECRET_VARIABLE).is_ok() {
            return;
        }
        let driver = driver();
        let mut sb = driver.base_case().await;
        let before = driver.all_requests().len();
        let actions = [
            action("goto", Some("http://x.test/a"), None),
            action("type", Some("#pw"), Some("{{SB_RECORDED_SECRET}}\n")),
        ];

        let error = replay(&mut sb, &actions).await.unwrap_err().to_string();

        assert!(error.contains("SB_RECORDED_SECRET"), "{error}");
        assert!(sent_since(&driver, before).is_empty(), "nothing ran");
    }

    #[tokio::test]
    async fn what_basecase_records_replays_without_a_skip_and_sends_the_same_commands() {
        let first = driver();
        let mut sb = first.base_case().await;
        let setup = first.all_requests().len();
        sb.start_recording();
        sb.open("http://x.test/a").await.unwrap();
        sb.type_text("#q", "rust").await.unwrap();
        sb.click("#go").await.unwrap();
        sb.assert_text("Hello", "#msg").await.unwrap();
        let session = sb.recorder_session().unwrap();
        let names: Vec<_> = session.actions().iter().map(|a| a.command.name()).collect();
        assert_eq!(names, ["open", "type_text", "click", "assert_text"]);

        let second = driver();
        let mut again = second.base_case().await;
        let setup_again = second.all_requests().len();
        let report = RecorderSession::replay(&mut again, session.actions())
            .await
            .unwrap();

        assert!(report.is_success(), "{report:?}");
        assert_eq!((report.steps.len(), report.skipped), (4, 0));
        assert_eq!(
            sent_since(&first, setup),
            sent_since(&second, setup_again),
            "the replay sends what the recorded calls sent"
        );
    }

    #[test]
    fn a_captcha_is_a_comment_that_names_the_solver() {
        let captcha = command("captcha", Some("recaptcha"), None);
        assert_eq!(
            rust_call(&captcha),
            Err("captcha (recaptcha): solve it by hand or use sb.solve_captcha()".to_owned())
        );
        // No provider: no parentheses.
        let bare = command("captcha", None, None);
        assert_eq!(
            rust_call(&bare),
            Err("captcha: solve it by hand or use sb.solve_captcha()".to_owned())
        );
        // The provider comes from the page: only its letters reach a comment.
        let hostile = command("captcha", Some("x\n}; evil()"), None);
        let note = rust_call(&hostile).unwrap_err();
        assert!(!note.contains('\n') && !note.contains(';'), "{note}");
        // It is no call to replay.
        assert!(!CALLABLE_NAMES.contains(&"captcha"));
    }

    #[test]
    fn a_bot_check_page_and_changing_query_parameters_get_a_note_after_the_call() {
        let goto = |url: &str| rust_call(&command("goto", Some(url), None)).unwrap();
        assert_eq!(
            goto("https://a.test/search?q=rust"),
            "sb.goto(\"https://a.test/search?q=rust\").await?;"
        );
        let sorry = goto("https://www.google.com/sorry/index?continue=https://a.test/&q=EgR");
        assert!(
            sorry.contains(".await?; // note: a bot-check page (/sorry/)"),
            "{sorry}"
        );
        let search =
            goto("https://www.google.com/search?q=rust&sca_esv=1&ei=2&sxsrf=3&ved=4&ei=5#top");
        assert!(
            search.ends_with("(sca_esv, ei, sxsrf, ved); remove them if the page refuses a replay"),
            "{search}"
        );
        // Only the opens carry the note, and `sorry` in a query is not the path.
        let click = rust_call(&command("click", Some("a[href*=ei]"), None)).unwrap();
        assert!(!click.contains("note"), "{click}");
        assert_eq!(url_note("https://a.test/x?sorry=/sorry/&other=1"), None);
        let both = url_note("https://a.test/sorry/x?ved=1").unwrap();
        assert!(both.contains("/sorry/") && both.contains("ved"), "{both}");
    }

    #[test]
    fn an_action_with_no_call_says_why_and_a_typo_names_the_closest_action() {
        for (name, reason) in [
            (
                "drag_and_drop_with_offset",
                "the recording keeps the selector but not the offset",
            ),
            (
                "download_file",
                "a recorded download does not say which folder the file went to",
            ),
        ] {
            assert_eq!(
                rust_call(&command(name, Some("#a"), None)),
                Err(format!("{name}: {reason}"))
            );
        }
        assert_eq!(
            rust_call(&command("clik", Some("#a"), None)),
            Err("unknown action \"clik\"; did you mean \"click\"?".to_owned())
        );
        assert_eq!(
            rust_call(&command("levitate", Some("#a"), None)),
            Err("unknown action \"levitate\"".to_owned())
        );
    }
}
