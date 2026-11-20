//! The recorded command: what one recorded action does, as a type.
//!
//! [`RecordedCommand`] has one variant for every action the recorder knows.
//! Each variant holds the arguments of its [`BaseCase`](crate::BaseCase) call
//! as typed fields (a selector, a URL, a number of pixels, ...), so a
//! generator or the replay matches on the variant and cannot read the wrong
//! argument. The code generators, the Gherkin writers and the replay all work
//! on this type.
//!
//! A recording is saved and loaded as `name`, `target` and `value`, the form
//! the page script and Python's recorder use ([`RecordedCommand::from_parts`]
//! reads it and [`RecordedCommand::parts`] writes it). That is the only place
//! where an action is told from its name.

use std::fmt;

/// What one recorded action does.
///
/// The variants are named after the [`BaseCase`](crate::BaseCase) method they
/// call; the snake case name of the method is the name of the action in a saved
/// recording ([`name`](Self::name)).
///
/// A name this crate does not know is [`Unknown`](Self::Unknown), and a known
/// name whose arguments are missing or cannot be read is
/// [`Malformed`](Self::Malformed). Both are kept as they were recorded, so a
/// saved recording loses nothing, and the generators say why they write no
/// call for them.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::api::recorder::{ActionRecorder, RecordedCommand};
///
/// let mut recorder = ActionRecorder::default();
/// recorder.record(RecordedCommand::Open {
///     url: "https://example.com".to_owned(),
/// });
/// recorder.record(RecordedCommand::Click {
///     selector: "#go".to_owned(),
/// });
/// assert!(recorder.to_rust_script().contains("sb.click(\"#go\").await?;"));
///
/// // The name, target and value of a saved recording make the same command.
/// let click = RecordedCommand::from_parts("click", Some("#go"), None);
/// assert_eq!(click.name(), "click");
/// assert_eq!(click.to_string(), "click #go");
/// ```
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum RecordedCommand {
    /// Opens `url` (`open`; the old name `navigate` is the same action).
    Open { url: String },
    /// Opens `url` (`goto`), the call Python's recorder writes.
    Goto { url: String },
    /// Opens `url` unless the browser is on it already (`goto_if_not_url`).
    GotoIfNotUrl { url: String },
    /// Clicks the element `selector` (`click`).
    Click { selector: String },
    /// Types `text` into the element `selector`, after it is cleared
    /// (`type_text`; the old name `type` is the same action).
    TypeText { selector: String, text: String },
    /// Clears the element `selector` (`clear`).
    Clear { selector: String },
    /// Clicks the link with the text `link_text` (`click_link_text`).
    ClickLinkText { link_text: String },
    /// Clicks the link with the text `link_text` (`click_link`).
    ClickLink { link_text: String },
    /// Clicks the link whose text holds `link_text` (`click_partial_link`).
    ClickPartialLink { link_text: String },
    /// Clicks the link whose text holds `link_text` (`click_partial_link_text`).
    ClickPartialLinkText { link_text: String },
    /// Submits the form of the element `selector` (`submit`).
    Submit { selector: String },
    /// Moves the mouse over the element `selector` (`hover`).
    Hover { selector: String },
    /// Selects the option with the text `text` in the list `selector`
    /// (`select_option_by_text`).
    SelectOptionByText { selector: String, text: String },
    /// Selects the option with the value `value` in the list `selector`
    /// (`select_option_by_value`).
    SelectOptionByValue { selector: String, value: String },
    /// Selects the option number `index` in the list `selector`, counting from
    /// zero (`select_option_by_index`).
    SelectOptionByIndex { selector: String, index: usize },
    /// Selects every option of the list `selector` (`select_all`).
    SelectAll { selector: String },
    /// Clears the selection of the list `selector` (`unselect_all`).
    UnselectAll { selector: String },
    /// Switches into the frame `frame` (`switch_to_frame`), a selector or a
    /// name.
    SwitchToFrame { frame: String },
    /// Switches back to the top document (`switch_to_default_content`).
    SwitchToDefaultContent,
    /// Switches to the parent of the current frame (`switch_to_parent_frame`).
    SwitchToParentFrame,
    /// Drags the element `source` onto the element `destination`
    /// (`drag_and_drop`).
    DragAndDrop { source: String, destination: String },
    /// Clicks the element `selector` with a script call of the pure CDP
    /// engine (`cdp_click`; the old name `cdp_click_element` is the same
    /// action).
    CdpClick { selector: String },
    /// Waits ten seconds for the element `selector` to be visible
    /// (`wait_for_element_visible`).
    WaitForElementVisible { selector: String },
    /// Waits ten seconds for the element `selector` to leave the page
    /// (`wait_for_element_absent`).
    WaitForElementAbsent { selector: String },
    /// Waits ten seconds for the element `selector` to be hidden
    /// (`wait_for_element_not_visible`).
    WaitForElementNotVisible { selector: String },
    /// Waits ten seconds for the element `selector` to be in the page
    /// (`wait_for_element_present`).
    WaitForElementPresent { selector: String },
    /// Waits ten seconds for the element `selector` to be clickable
    /// (`wait_for_element_clickable`).
    WaitForElementClickable { selector: String },
    /// Waits for the page to finish loading (`wait_for_ready_state_complete`).
    WaitForReadyStateComplete,
    /// Asserts that the element `selector` shows the text `text`
    /// (`assert_text`).
    AssertText { text: String, selector: String },
    /// Asserts that the element `selector` shows the text `text`
    /// (`assert_text_visible`).
    AssertTextVisible { text: String, selector: String },
    /// Asserts that the element `selector` does not show the text `text`
    /// (`assert_text_not_visible`).
    AssertTextNotVisible { text: String, selector: String },
    /// Asserts that the element `selector` shows exactly the text `text`
    /// (`assert_exact_text`).
    AssertExactText { text: String, selector: String },
    /// Asserts that the attribute `attribute` of the element `selector` is
    /// `content` (`assert_attribute`). Without `content` the attribute need
    /// only exist, which the Gherkin step says; the Rust generators need it.
    AssertAttribute {
        selector: String,
        attribute: String,
        content: Option<String>,
    },
    /// Asserts that the title of the page is `title` (`assert_title`).
    AssertTitle { title: String },
    /// Asserts that the title of the page holds `title`
    /// (`assert_title_contains`).
    AssertTitleContains { title: String },
    /// Asserts that a link has the text `link_text` (`assert_link_text`).
    AssertLinkText { link_text: String },
    /// Maximizes the window (`maximize_window`).
    MaximizeWindow,
    /// Sets the size of the window in pixels (`set_window_size`).
    SetWindowSize { width: u32, height: u32 },
    /// Switches to the window `window`, a handle or a number
    /// (`switch_to_window`).
    SwitchToWindow { window: String },
    /// Switches to the window that opened last (`switch_to_new_window`).
    SwitchToNewWindow,
    /// Switches to the newest window (`switch_to_newest_window`).
    SwitchToNewestWindow,
    /// Switches to the first window (`switch_to_default_window`).
    SwitchToDefaultWindow,
    /// Goes back one page (`go_back`).
    GoBack,
    /// Goes forward one page (`go_forward`).
    GoForward,
    /// Reloads the page (`refresh`).
    Refresh,
    /// Deletes every cookie (`delete_all_cookies`).
    DeleteAllCookies,
    /// Deletes the cookie `name` (`delete_cookie`).
    DeleteCookie { name: String },
    /// Adds the cookie `name` with the value `value` (`add_cookie`). A password
    /// typed in the browser is [`SECRET_PLACEHOLDER`](super::SECRET_PLACEHOLDER).
    AddCookie { name: String, value: String },
    /// Double-clicks the element `selector` (`double_click`).
    DoubleClick { selector: String },
    /// Right-clicks the element `selector` (`context_click`).
    ContextClick { selector: String },
    /// Clicks `x`, `y` pixels from the top-left corner of the element
    /// `selector`, or from its centre when `center` (`click_with_offset`).
    ClickWithOffset {
        selector: String,
        x: i64,
        y: i64,
        center: bool,
    },
    /// Double-clicks `x`, `y` pixels from the top-left corner of the element
    /// `selector`, or from its centre when `center`
    /// (`double_click_with_offset`).
    DoubleClickWithOffset {
        selector: String,
        x: i64,
        y: i64,
        center: bool,
    },
    /// Scrolls to the element `selector` (`scroll_to`).
    ScrollTo { selector: String },
    /// Scrolls to the bottom of the page (`scroll_to_bottom`).
    ScrollToBottom,
    /// Scrolls to the top of the page (`scroll_to_top`).
    ScrollToTop,
    /// Scrolls the element `selector` into view (`scroll_into_view`).
    ScrollIntoView { selector: String },
    /// Scrolls to the element `selector` smoothly (`smooth_scroll_to`).
    SmoothScrollTo { selector: String },
    /// Scrolls to the element `selector` slowly (`slow_scroll_to`).
    SlowScrollTo { selector: String },
    /// Checks the box `selector` if it is not checked (`check_if_unchecked`).
    CheckIfUnchecked { selector: String },
    /// Unchecks the box `selector` if it is checked (`uncheck_if_checked`).
    UncheckIfChecked { selector: String },
    /// Types `text` after what the element `selector` holds (`add_text`).
    AddText { selector: String, text: String },
    /// Sends the keys `keys` to the element `selector` (`send_keys`).
    SendKeys { selector: String, keys: String },
    /// Clicks the element `selector` with a script (`js_click`).
    JsClick { selector: String },
    /// Types `text` into the element `selector` with a script (`js_type`).
    JsType { selector: String, text: String },
    /// Sets the text of the element `selector` with a script
    /// (`js_update_text`).
    JsUpdateText { selector: String, text: String },
    /// Clicks the element `selector` with a script when it is in the page
    /// (`js_click_if_present`).
    JsClickIfPresent { selector: String },
    /// Clicks every element `selector` with a script (`js_click_all`).
    JsClickAll { selector: String },
    /// Clicks the element `selector` with jQuery (`jquery_click`).
    JqueryClick { selector: String },
    /// Clicks every element `selector` with jQuery (`jquery_click_all`).
    JqueryClickAll { selector: String },
    /// Types `text` into the element `selector` with jQuery (`jquery_type`).
    JqueryType { selector: String, text: String },
    /// Sets the text of the element `selector` with jQuery
    /// (`jquery_update_text`).
    JqueryUpdateText { selector: String, text: String },
    /// Sets the attribute `attribute` of the element `selector` to `value`
    /// (`set_attribute`).
    SetAttribute {
        selector: String,
        attribute: String,
        value: String,
    },
    /// Removes the attribute `attribute` of the element `selector`
    /// (`remove_attribute`).
    RemoveAttribute { selector: String, attribute: String },
    /// Chooses the file `path` in the file input `selector` (`choose_file`).
    ChooseFile { selector: String, path: String },
    /// Waits ten seconds for an alert and accepts it
    /// (`wait_for_and_accept_alert`).
    WaitForAndAcceptAlert,
    /// Waits ten seconds for an alert and dismisses it
    /// (`wait_for_and_dismiss_alert`).
    WaitForAndDismissAlert,
    /// Accepts the alert (`accept_alert`).
    AcceptAlert,
    /// Dismisses the alert (`dismiss_alert`).
    DismissAlert,
    /// Types `text` into the alert (`type_alert_text`).
    TypeAlertText { text: String },
    /// Clears local storage (`clear_local_storage`).
    ClearLocalStorage,
    /// Removes the item `key` from local storage (`remove_local_storage_item`).
    RemoveLocalStorageItem { key: String },
    /// Sets the item `key` of local storage to `value`
    /// (`set_local_storage_item`).
    SetLocalStorageItem { key: String, value: String },
    /// Closes the window (`close_window`).
    CloseWindow,
    /// Clicks the element `selector` after it is highlighted
    /// (`highlight_click`).
    HighlightClick { selector: String },
    /// Opens a new window (`open_new_window`).
    OpenNewWindow,
    /// Clicks the visible elements `selector` (`click_visible_elements`).
    ClickVisibleElements { selector: String },
    /// Finds the element `selector` (`find_element`); the element is not
    /// used.
    FindElement { selector: String },
    /// Finds the elements `selector` (`find_elements`); they are not used.
    FindElements { selector: String },
    /// Clicks the element `selector` slowly (`slow_click`).
    SlowClick { selector: String },
    /// Types `text` into the element `selector` like a person (`human_type`).
    HumanType { selector: String, text: String },
    /// Clicks the element `selector` like a person (`human_click`).
    HumanClick { selector: String },
    /// Clicks the element `selector` without the signs of a driver
    /// (`uc_click`).
    UcClick { selector: String },
    /// Types `text` into the element `selector` without the signs of a driver
    /// (`uc_type`).
    UcType { selector: String, text: String },
    /// Sets the text of the element `selector` to `text` (`update_text`).
    UpdateText { selector: String, text: String },
    /// Pauses for `seconds` (`sleep`).
    Sleep { seconds: f64 },
    /// Clicks the element `xpath` (`click_xpath`).
    ClickXpath { xpath: String },
    /// Highlights the element `selector` (`highlight`).
    Highlight { selector: String },
    /// Presses the keys `keys` in the element `selector` (`press_keys`).
    PressKeys { selector: String, keys: String },
    /// Types the one-time code `code` into the element `selector`
    /// (`enter_mfa_code`).
    EnterMfaCode { selector: String, code: String },
    /// Moves to the element `hover_selector`, then clicks the element
    /// `click_selector` (`hover_and_click`).
    HoverAndClick {
        hover_selector: String,
        click_selector: String,
    },
    /// Sets the value of the element `selector` to `value` (`set_value`).
    SetValue { selector: String, value: String },
    /// Makes the frame `frame` the content that the next calls act on
    /// (`set_content_to_frame`).
    SetContentToFrame { frame: String },
    /// Makes the top document the content that the next calls act on
    /// (`set_content_to_default`).
    SetContentToDefault,
    /// Makes the parent of the current content the content that the next
    /// calls act on (`set_content_to_parent`).
    SetContentToParent,
    /// Waits for the element `selector` (`wait_for_element`).
    WaitForElement { selector: String },
    /// Asserts that the element `selector` is visible (`assert_element`).
    AssertElement { selector: String },
    /// Asserts that the element `selector` is in the page
    /// (`assert_element_present`).
    AssertElementPresent { selector: String },
    /// Asserts that the element `selector` is hidden
    /// (`assert_element_not_visible`).
    AssertElementNotVisible { selector: String },
    /// Asserts that the element `selector` holds text (`assert_non_empty_text`).
    AssertNonEmptyText { selector: String },
    /// Checks that the element `selector` is visible when the test ends
    /// (`deferred_assert_element`).
    DeferredAssertElement { selector: String },
    /// Checks that the element `selector` is in the page when the test ends
    /// (`deferred_assert_element_present`).
    DeferredAssertElementPresent { selector: String },
    /// Checks that the element `selector` holds text when the test ends
    /// (`deferred_assert_non_empty_text`).
    DeferredAssertNonEmptyText { selector: String },
    /// Checks that the element `selector` shows `text` when the test ends
    /// (`deferred_assert_text`).
    DeferredAssertText { text: String, selector: String },
    /// Checks that the element `selector` shows exactly `text` when the test
    /// ends (`deferred_assert_exact_text`).
    DeferredAssertExactText { text: String, selector: String },
    /// Asserts that the element `selector` does not show exactly `text`
    /// (`assert_exact_text_not_visible`).
    AssertExactTextNotVisible { text: String, selector: String },
    /// Sets the attribute `attribute` of every element `selector` to `value`
    /// (`set_attributes`).
    SetAttributes {
        selector: String,
        attribute: String,
        value: String,
    },
    /// Asserts that the address of the page is `url` (`assert_url`).
    AssertUrl { url: String },
    /// Asserts that the address of the page holds `url` (`assert_url_contains`).
    AssertUrlContains { url: String },
    /// Asserts that the file `name` is in the downloads
    /// (`assert_downloaded_file`).
    AssertDownloadedFile { name: String },
    /// Saves a screenshot as the file `name` (`save_screenshot`).
    SaveScreenshot {
        name: String,
        folder: Option<String>,
    },
    /// Saves a screenshot in the logs (`save_screenshot_to_logs`).
    SaveScreenshotToLogs,
    /// Saves the source of the page in the logs (`save_page_source_to_logs`).
    SavePageSourceToLogs,
    /// Shows the file choosers of the page (`show_file_choosers`).
    ShowFileChoosers,
    /// Checks the deferred assertions (`process_deferred_asserts`).
    ProcessDeferredAsserts,
    /// Turns demo mode on (`activate_demo_mode`).
    ActivateDemoMode,
    /// Turns demo mode off (`deactivate_demo_mode`).
    DeactivateDemoMode,
    /// Clears session storage (`clear_session_storage`).
    ClearSessionStorage,
    /// Opens a new tab, at `url` when the recording remembers the page it
    /// opened (`open_new_tab`).
    OpenNewTab { url: Option<String> },

    /// A CAPTCHA widget that a person worked in a frame (`captcha`); the
    /// provider is what the page script saw. It has no call: the generators
    /// write a comment that says to solve the widget by hand or with
    /// `solve_captcha`.
    Captcha { provider: Option<String> },
    /// A drag with an offset (`drag_and_drop_with_offset`). `BaseCase` records
    /// the selector but not the offset, so there is no call to write.
    DragAndDropWithOffset {
        selector: String,
        offset: Option<String>,
    },
    /// A download (`download_file`) of `url`, into `destination` if the
    /// recording knows it. It does not say which folder the file went to, so
    /// there is no call to write.
    DownloadFile {
        url: String,
        destination: Option<String>,
    },
    /// A page saved as a PDF in the logs (`save_as_pdf_to_logs`); the
    /// recording does not hold the file name the call needs.
    SaveAsPdfToLogs,
    /// A screenshot saved as `name` in `folder` (`save_screenshot_to_folder`);
    /// `BaseCase::save_screenshot` takes a file name only.
    SaveScreenshotToFolder {
        name: String,
        folder: Option<String>,
    },

    /// An action whose name this crate does not know, as it was recorded.
    Unknown {
        name: String,
        target: Option<String>,
        value: Option<String>,
    },
    /// An action whose name is known but whose arguments are missing or cannot
    /// be read, as it was recorded, with what is wrong.
    Malformed {
        name: String,
        target: Option<String>,
        value: Option<String>,
        error: CommandError,
    },
}

/// What is wrong with the arguments of a recorded action
/// ([`RecordedCommand::Malformed`]).
///
/// The text of each case is what the generators write after `// skipped:` and
/// what a replay reports. `action` is the name of the action as the generators
/// know it (`type_text` for a recorded `type`).
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum CommandError {
    /// The action needs a target and has none.
    MissingTarget { action: String },
    /// The action needs a value and has none.
    MissingValue { action: String },
    /// The value is not `name=content`.
    NotNameContent { action: String, value: String },
    /// The target is not `width,height`.
    NotWidthHeight { action: String, target: String },
    /// The value is not `x, y` or `x, y, center`.
    NotOffset { action: String, value: String },
    /// `what` (a width, an index, an offset) is not a whole number in range.
    NotWholeNumber {
        action: String,
        what: &'static str,
        text: String,
    },
    /// The text is not a number of seconds.
    NotSeconds { action: String, text: String },
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingTarget { action } => write!(f, "{action} has no target"),
            Self::MissingValue { action } => write!(f, "{action} has no value"),
            Self::NotNameContent { action, value } => {
                write!(f, "{action}: value {value:?} is not name=content")
            }
            Self::NotWidthHeight { action, target } => {
                write!(f, "{action}: target {target:?} is not width,height")
            }
            Self::NotOffset { action, value } => {
                write!(f, "{action}: value {value:?} is not x, y")
            }
            Self::NotWholeNumber { action, what, text } => {
                write!(
                    f,
                    "{action}: {what} {text:?} is not a whole number in range"
                )
            }
            Self::NotSeconds { action, text } => {
                write!(f, "{action}: {text:?} is not a number of seconds")
            }
        }
    }
}

impl std::error::Error for CommandError {}

/// The names that the generators and the replay know an action by, with the
/// older names that mean the same ([`ALIASES`]); the actions with no call
/// (`captcha`, `drag_and_drop_with_offset`, `download_file`,
/// `save_as_pdf_to_logs`, `save_screenshot_to_folder`) are not here.
pub(super) const CALLABLE_NAMES: &[&str] = &[
    "open",
    "goto_if_not_url",
    "click",
    "type_text",
    "clear",
    "click_link_text",
    "click_link",
    "click_partial_link",
    "click_partial_link_text",
    "submit",
    "hover",
    "select_option_by_text",
    "select_option_by_value",
    "select_option_by_index",
    "select_all",
    "unselect_all",
    "switch_to_frame",
    "switch_to_default_content",
    "switch_to_parent_frame",
    "drag_and_drop",
    "cdp_click",
    "wait_for_element_visible",
    "wait_for_element_absent",
    "wait_for_element_not_visible",
    "wait_for_element_present",
    "wait_for_element_clickable",
    "wait_for_ready_state_complete",
    "assert_text",
    "assert_text_visible",
    "assert_text_not_visible",
    "assert_exact_text",
    "assert_attribute",
    "assert_title",
    "assert_title_contains",
    "assert_link_text",
    "maximize_window",
    "set_window_size",
    "switch_to_window",
    "switch_to_new_window",
    "switch_to_newest_window",
    "switch_to_default_window",
    "go_back",
    "go_forward",
    "refresh",
    "delete_all_cookies",
    "delete_cookie",
    "add_cookie",
    "double_click",
    "context_click",
    "click_with_offset",
    "double_click_with_offset",
    "scroll_to",
    "scroll_to_bottom",
    "scroll_to_top",
    "scroll_into_view",
    "smooth_scroll_to",
    "slow_scroll_to",
    "check_if_unchecked",
    "uncheck_if_checked",
    "add_text",
    "send_keys",
    "js_click",
    "js_type",
    "js_update_text",
    "js_click_if_present",
    "js_click_all",
    "jquery_click",
    "jquery_click_all",
    "jquery_type",
    "jquery_update_text",
    "set_attribute",
    "remove_attribute",
    "choose_file",
    "wait_for_and_accept_alert",
    "wait_for_and_dismiss_alert",
    "accept_alert",
    "dismiss_alert",
    "type_alert_text",
    "clear_local_storage",
    "remove_local_storage_item",
    "set_local_storage_item",
    "close_window",
    "highlight_click",
    "open_new_window",
    "click_visible_elements",
    "find_element",
    "find_elements",
    "slow_click",
    "human_type",
    "human_click",
    "uc_click",
    "uc_type",
    "update_text",
    "sleep",
    "click_xpath",
    "goto",
    "highlight",
    "press_keys",
    "enter_mfa_code",
    "hover_and_click",
    "set_value",
    "set_content_to_frame",
    "set_content_to_default",
    "set_content_to_parent",
    "wait_for_element",
    "assert_element",
    "assert_element_present",
    "assert_element_not_visible",
    "assert_non_empty_text",
    "deferred_assert_element",
    "deferred_assert_element_present",
    "deferred_assert_non_empty_text",
    "deferred_assert_text",
    "deferred_assert_exact_text",
    "assert_exact_text_not_visible",
    "set_attributes",
    "assert_url",
    "assert_url_contains",
    "assert_downloaded_file",
    "save_screenshot",
    "save_screenshot_to_logs",
    "save_page_source_to_logs",
    "show_file_choosers",
    "process_deferred_asserts",
    "activate_demo_mode",
    "deactivate_demo_mode",
    "clear_session_storage",
    "open_new_tab",
];

/// Other names for an action: those older recordings use, and the name each
/// one stands for.
pub(super) const ALIASES: &[(&str, &str)] = &[
    // `goto` is an action of its own (`BaseCase::goto`, what Python's recorder
    // writes).
    ("navigate", "open"),
    ("type", "type_text"),
    // Deprecated since 0.5.0; older recordings keep the name, and a generated
    // program must not call a deprecated method.
    ("cdp_click_element", "cdp_click"),
];

/// The name an action is known by: `recorded` itself unless it is one of the
/// [`ALIASES`].
fn canonical_name(recorded: &str) -> &str {
    ALIASES
        .iter()
        .find(|(alias, _)| *alias == recorded)
        .map_or(recorded, |(_, canonical)| *canonical)
}

/// The three pieces of a recorded action, with the name it is known by, and
/// the checks the fields of a variant need.
struct Wire<'a> {
    action: &'a str,
    target: Option<&'a str>,
    value: Option<&'a str>,
}

impl Wire<'_> {
    fn target(&self) -> Result<String, CommandError> {
        self.target
            .map(str::to_owned)
            .ok_or_else(|| CommandError::MissingTarget {
                action: self.action.to_owned(),
            })
    }

    fn value(&self) -> Result<String, CommandError> {
        self.value
            .map(str::to_owned)
            .ok_or_else(|| CommandError::MissingValue {
                action: self.action.to_owned(),
            })
    }

    /// `(name, content)` of a value `name=content`, as written.
    fn pair(&self) -> Result<(String, String), CommandError> {
        let value = self.value.ok_or_else(|| CommandError::MissingValue {
            action: self.action.to_owned(),
        })?;
        let (name, content) =
            value
                .split_once('=')
                .ok_or_else(|| CommandError::NotNameContent {
                    action: self.action.to_owned(),
                    value: value.to_owned(),
                })?;
        Ok((name.to_owned(), content.to_owned()))
    }

    /// A whole number read from recorded text.
    fn whole<T: std::str::FromStr>(
        &self,
        what: &'static str,
        text: &str,
    ) -> Result<T, CommandError> {
        text.trim()
            .parse::<T>()
            .map_err(|_| CommandError::NotWholeNumber {
                action: self.action.to_owned(),
                what,
                text: text.to_owned(),
            })
    }

    /// `(width, height)` of a target `width,height`.
    fn size(&self) -> Result<(u32, u32), CommandError> {
        let size = self.target.ok_or_else(|| CommandError::MissingTarget {
            action: self.action.to_owned(),
        })?;
        let (width, height) = size
            .split_once(',')
            .ok_or_else(|| CommandError::NotWidthHeight {
                action: self.action.to_owned(),
                target: size.to_owned(),
            })?;
        Ok((self.whole("width", width)?, self.whole("height", height)?))
    }

    /// `(selector, x, y, center)` of a value `x, y` or `x, y, center`.
    fn offset(&self) -> Result<(String, i64, i64, bool), CommandError> {
        let value = self.value.ok_or_else(|| CommandError::MissingValue {
            action: self.action.to_owned(),
        })?;
        let wrong = || CommandError::NotOffset {
            action: self.action.to_owned(),
            value: value.to_owned(),
        };
        let mut parts = value.split(',');
        let (Some(x), Some(y)) = (parts.next(), parts.next()) else {
            return Err(wrong());
        };
        let center = match parts.next().map(str::trim) {
            None => false,
            Some("center") => true,
            Some(_) => return Err(wrong()),
        };
        if parts.next().is_some() {
            return Err(wrong());
        }
        let selector = self.target()?;
        Ok((
            selector,
            self.whole("x offset", x)?,
            self.whole("y offset", y)?,
            center,
        ))
    }

    /// The pause of a `sleep`, in seconds.
    fn seconds(&self) -> Result<f64, CommandError> {
        let text = self.value.ok_or_else(|| CommandError::MissingValue {
            action: self.action.to_owned(),
        })?;
        text.trim()
            .parse::<f64>()
            .ok()
            .filter(|seconds| seconds.is_finite() && *seconds >= 0.0)
            .ok_or_else(|| CommandError::NotSeconds {
                action: self.action.to_owned(),
                text: text.to_owned(),
            })
    }
}

impl RecordedCommand {
    /// The command that a recorded `name`, `target` and `value` stand for: the
    /// form of a saved recording, of the page script and of Python's recorder.
    ///
    /// A name this crate does not know is [`Unknown`](Self::Unknown); a known
    /// name whose arguments are missing or cannot be read is
    /// [`Malformed`](Self::Malformed). Neither is an error here: a recording
    /// that holds one still loads, and what it cannot do is said when it is
    /// written or replayed. An argument that the action does not use is
    /// dropped.
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::api::recorder::RecordedCommand;
    ///
    /// let typed = RecordedCommand::from_parts("type_text", Some("#q"), Some("rust"));
    /// assert_eq!(
    ///     typed,
    ///     RecordedCommand::TypeText { selector: "#q".to_owned(), text: "rust".to_owned() }
    /// );
    /// assert!(matches!(
    ///     RecordedCommand::from_parts("levitate", Some("#q"), None),
    ///     RecordedCommand::Unknown { .. }
    /// ));
    /// assert!(matches!(
    ///     RecordedCommand::from_parts("click", None, None),
    ///     RecordedCommand::Malformed { .. }
    /// ));
    /// ```
    #[must_use]
    pub fn from_parts(name: &str, target: Option<&str>, value: Option<&str>) -> Self {
        match Self::parse(name, target, value) {
            Ok(Some(command)) => command,
            Ok(None) => Self::Unknown {
                name: name.to_owned(),
                target: target.map(str::to_owned),
                value: value.map(str::to_owned),
            },
            Err(error) => Self::Malformed {
                name: name.to_owned(),
                target: target.map(str::to_owned),
                value: value.map(str::to_owned),
                error,
            },
        }
    }

    /// The command of a known action, `None` for a name this crate does not
    /// know.
    #[expect(
        clippy::too_many_lines,
        reason = "one arm per action reads best as one table"
    )]
    fn parse(
        name: &str,
        target: Option<&str>,
        value: Option<&str>,
    ) -> Result<Option<Self>, CommandError> {
        let action = canonical_name(name);
        let w = Wire {
            action,
            target,
            value,
        };
        Ok(Some(match action {
            "open" => Self::Open { url: w.target()? },
            "goto" => Self::Goto { url: w.target()? },
            "goto_if_not_url" => Self::GotoIfNotUrl { url: w.target()? },
            "click" => Self::Click {
                selector: w.target()?,
            },
            "type_text" => Self::TypeText {
                selector: w.target()?,
                text: w.value()?,
            },
            "clear" => Self::Clear {
                selector: w.target()?,
            },
            "click_link_text" => Self::ClickLinkText {
                link_text: w.target()?,
            },
            "click_link" => Self::ClickLink {
                link_text: w.target()?,
            },
            "click_partial_link" => Self::ClickPartialLink {
                link_text: w.target()?,
            },
            "click_partial_link_text" => Self::ClickPartialLinkText {
                link_text: w.target()?,
            },
            "submit" => Self::Submit {
                selector: w.target()?,
            },
            "hover" => Self::Hover {
                selector: w.target()?,
            },
            "select_option_by_text" => Self::SelectOptionByText {
                selector: w.target()?,
                text: w.value()?,
            },
            "select_option_by_value" => Self::SelectOptionByValue {
                selector: w.target()?,
                value: w.value()?,
            },
            "select_option_by_index" => {
                let selector = w.target()?;
                let index = w.whole("index", &w.value()?)?;
                Self::SelectOptionByIndex { selector, index }
            }
            "select_all" => Self::SelectAll {
                selector: w.target()?,
            },
            "unselect_all" => Self::UnselectAll {
                selector: w.target()?,
            },
            "switch_to_frame" => Self::SwitchToFrame { frame: w.target()? },
            "switch_to_default_content" => Self::SwitchToDefaultContent,
            "switch_to_parent_frame" => Self::SwitchToParentFrame,
            "drag_and_drop" => Self::DragAndDrop {
                source: w.target()?,
                destination: w.value()?,
            },
            "cdp_click" => Self::CdpClick {
                selector: w.target()?,
            },
            "wait_for_element_visible" => Self::WaitForElementVisible {
                selector: w.target()?,
            },
            "wait_for_element_absent" => Self::WaitForElementAbsent {
                selector: w.target()?,
            },
            "wait_for_element_not_visible" => Self::WaitForElementNotVisible {
                selector: w.target()?,
            },
            "wait_for_element_present" => Self::WaitForElementPresent {
                selector: w.target()?,
            },
            "wait_for_element_clickable" => Self::WaitForElementClickable {
                selector: w.target()?,
            },
            "wait_for_ready_state_complete" => Self::WaitForReadyStateComplete,
            "assert_text" => Self::AssertText {
                text: w.value()?,
                selector: w.target()?,
            },
            "assert_text_visible" => Self::AssertTextVisible {
                text: w.value()?,
                selector: w.target()?,
            },
            "assert_text_not_visible" => Self::AssertTextNotVisible {
                text: w.value()?,
                selector: w.target()?,
            },
            "assert_exact_text" => Self::AssertExactText {
                text: w.value()?,
                selector: w.target()?,
            },
            "assert_attribute" => {
                // The Gherkin step takes an attribute with no content; the
                // generators of Rust say that the value is not name=content.
                let value = w.value()?;
                let (attribute, content) = match value.split_once('=') {
                    Some((attribute, content)) => (attribute.to_owned(), Some(content.to_owned())),
                    None => (value, None),
                };
                Self::AssertAttribute {
                    selector: w.target()?,
                    attribute,
                    content,
                }
            }
            "assert_title" => Self::AssertTitle { title: w.target()? },
            "assert_title_contains" => Self::AssertTitleContains { title: w.target()? },
            "assert_link_text" => Self::AssertLinkText {
                link_text: w.target()?,
            },
            "maximize_window" => Self::MaximizeWindow,
            "set_window_size" => {
                let (width, height) = w.size()?;
                Self::SetWindowSize { width, height }
            }
            "switch_to_window" => Self::SwitchToWindow {
                window: w.target()?,
            },
            "switch_to_new_window" => Self::SwitchToNewWindow,
            "switch_to_newest_window" => Self::SwitchToNewestWindow,
            "switch_to_default_window" => Self::SwitchToDefaultWindow,
            "go_back" => Self::GoBack,
            "go_forward" => Self::GoForward,
            "refresh" => Self::Refresh,
            "delete_all_cookies" => Self::DeleteAllCookies,
            "delete_cookie" => Self::DeleteCookie { name: w.target()? },
            "add_cookie" => Self::AddCookie {
                name: w.target()?,
                value: w.value()?,
            },
            "double_click" => Self::DoubleClick {
                selector: w.target()?,
            },
            "context_click" => Self::ContextClick {
                selector: w.target()?,
            },
            "click_with_offset" => {
                let (selector, x, y, center) = w.offset()?;
                Self::ClickWithOffset {
                    selector,
                    x,
                    y,
                    center,
                }
            }
            "double_click_with_offset" => {
                let (selector, x, y, center) = w.offset()?;
                Self::DoubleClickWithOffset {
                    selector,
                    x,
                    y,
                    center,
                }
            }
            "scroll_to" => Self::ScrollTo {
                selector: w.target()?,
            },
            "scroll_to_bottom" => Self::ScrollToBottom,
            "scroll_to_top" => Self::ScrollToTop,
            "scroll_into_view" => Self::ScrollIntoView {
                selector: w.target()?,
            },
            "smooth_scroll_to" => Self::SmoothScrollTo {
                selector: w.target()?,
            },
            "slow_scroll_to" => Self::SlowScrollTo {
                selector: w.target()?,
            },
            "check_if_unchecked" => Self::CheckIfUnchecked {
                selector: w.target()?,
            },
            "uncheck_if_checked" => Self::UncheckIfChecked {
                selector: w.target()?,
            },
            "add_text" => Self::AddText {
                selector: w.target()?,
                text: w.value()?,
            },
            "send_keys" => Self::SendKeys {
                selector: w.target()?,
                keys: w.value()?,
            },
            "js_click" => Self::JsClick {
                selector: w.target()?,
            },
            "js_type" => Self::JsType {
                selector: w.target()?,
                text: w.value()?,
            },
            "js_update_text" => Self::JsUpdateText {
                selector: w.target()?,
                text: w.value()?,
            },
            "js_click_if_present" => Self::JsClickIfPresent {
                selector: w.target()?,
            },
            "js_click_all" => Self::JsClickAll {
                selector: w.target()?,
            },
            "jquery_click" => Self::JqueryClick {
                selector: w.target()?,
            },
            "jquery_click_all" => Self::JqueryClickAll {
                selector: w.target()?,
            },
            "jquery_type" => Self::JqueryType {
                selector: w.target()?,
                text: w.value()?,
            },
            "jquery_update_text" => Self::JqueryUpdateText {
                selector: w.target()?,
                text: w.value()?,
            },
            "set_attribute" => {
                let (attribute, value) = w.pair()?;
                Self::SetAttribute {
                    selector: w.target()?,
                    attribute,
                    value,
                }
            }
            "remove_attribute" => Self::RemoveAttribute {
                selector: w.target()?,
                attribute: w.value()?,
            },
            "choose_file" => Self::ChooseFile {
                selector: w.target()?,
                path: w.value()?,
            },
            "wait_for_and_accept_alert" => Self::WaitForAndAcceptAlert,
            "wait_for_and_dismiss_alert" => Self::WaitForAndDismissAlert,
            "accept_alert" => Self::AcceptAlert,
            "dismiss_alert" => Self::DismissAlert,
            "type_alert_text" => Self::TypeAlertText { text: w.target()? },
            "clear_local_storage" => Self::ClearLocalStorage,
            "remove_local_storage_item" => Self::RemoveLocalStorageItem { key: w.target()? },
            "set_local_storage_item" => Self::SetLocalStorageItem {
                key: w.target()?,
                value: w.value()?,
            },
            "close_window" => Self::CloseWindow,
            "highlight_click" => Self::HighlightClick {
                selector: w.target()?,
            },
            "open_new_window" => Self::OpenNewWindow,
            "click_visible_elements" => Self::ClickVisibleElements {
                selector: w.target()?,
            },
            "find_element" => Self::FindElement {
                selector: w.target()?,
            },
            "find_elements" => Self::FindElements {
                selector: w.target()?,
            },
            "slow_click" => Self::SlowClick {
                selector: w.target()?,
            },
            "human_type" => Self::HumanType {
                selector: w.target()?,
                text: w.value()?,
            },
            "human_click" => Self::HumanClick {
                selector: w.target()?,
            },
            "uc_click" => Self::UcClick {
                selector: w.target()?,
            },
            "uc_type" => Self::UcType {
                selector: w.target()?,
                text: w.value()?,
            },
            "update_text" => Self::UpdateText {
                selector: w.target()?,
                text: w.value()?,
            },
            "sleep" => Self::Sleep {
                seconds: w.seconds()?,
            },
            "click_xpath" => Self::ClickXpath { xpath: w.target()? },
            "highlight" => Self::Highlight {
                selector: w.target()?,
            },
            "press_keys" => Self::PressKeys {
                selector: w.target()?,
                keys: w.value()?,
            },
            "enter_mfa_code" => Self::EnterMfaCode {
                selector: w.target()?,
                code: w.value()?,
            },
            "hover_and_click" => Self::HoverAndClick {
                hover_selector: w.target()?,
                click_selector: w.value()?,
            },
            "set_value" => Self::SetValue {
                selector: w.target()?,
                value: w.value()?,
            },
            "set_content_to_frame" => Self::SetContentToFrame { frame: w.target()? },
            "set_content_to_default" => Self::SetContentToDefault,
            "set_content_to_parent" => Self::SetContentToParent,
            "wait_for_element" => Self::WaitForElement {
                selector: w.target()?,
            },
            "assert_element" => Self::AssertElement {
                selector: w.target()?,
            },
            "assert_element_present" => Self::AssertElementPresent {
                selector: w.target()?,
            },
            "assert_element_not_visible" => Self::AssertElementNotVisible {
                selector: w.target()?,
            },
            "assert_non_empty_text" => Self::AssertNonEmptyText {
                selector: w.target()?,
            },
            "deferred_assert_element" => Self::DeferredAssertElement {
                selector: w.target()?,
            },
            "deferred_assert_element_present" => Self::DeferredAssertElementPresent {
                selector: w.target()?,
            },
            "deferred_assert_non_empty_text" => Self::DeferredAssertNonEmptyText {
                selector: w.target()?,
            },
            "deferred_assert_text" => Self::DeferredAssertText {
                text: w.value()?,
                selector: w.target()?,
            },
            "deferred_assert_exact_text" => Self::DeferredAssertExactText {
                text: w.value()?,
                selector: w.target()?,
            },
            "assert_exact_text_not_visible" => Self::AssertExactTextNotVisible {
                text: w.value()?,
                selector: w.target()?,
            },
            "set_attributes" => {
                let (attribute, value) = w.pair()?;
                Self::SetAttributes {
                    selector: w.target()?,
                    attribute,
                    value,
                }
            }
            "assert_url" => Self::AssertUrl { url: w.target()? },
            "assert_url_contains" => Self::AssertUrlContains { url: w.target()? },
            "assert_downloaded_file" => Self::AssertDownloadedFile { name: w.target()? },
            "save_screenshot" => Self::SaveScreenshot {
                name: w.target()?,
                folder: value.map(str::to_owned),
            },
            "save_screenshot_to_logs" => Self::SaveScreenshotToLogs,
            "save_page_source_to_logs" => Self::SavePageSourceToLogs,
            "show_file_choosers" => Self::ShowFileChoosers,
            "process_deferred_asserts" => Self::ProcessDeferredAsserts,
            "activate_demo_mode" => Self::ActivateDemoMode,
            "deactivate_demo_mode" => Self::DeactivateDemoMode,
            "clear_session_storage" => Self::ClearSessionStorage,
            "open_new_tab" => Self::OpenNewTab {
                url: target.map(str::to_owned),
            },
            "captcha" => Self::Captcha {
                provider: target.map(str::to_owned),
            },
            "drag_and_drop_with_offset" => Self::DragAndDropWithOffset {
                selector: w.target()?,
                offset: value.map(str::to_owned),
            },
            "download_file" => Self::DownloadFile {
                url: w.target()?,
                destination: value.map(str::to_owned),
            },
            "save_as_pdf_to_logs" => Self::SaveAsPdfToLogs,
            "save_screenshot_to_folder" => Self::SaveScreenshotToFolder {
                name: w.target()?,
                folder: value.map(str::to_owned),
            },
            _ => return Ok(None),
        }))
    }

    /// The name of the action in a saved recording: the snake case name of the
    /// `BaseCase` method. For a command that is [`Unknown`](Self::Unknown) or
    /// [`Malformed`](Self::Malformed) it is the name as it was recorded.
    #[must_use]
    #[expect(
        clippy::too_many_lines,
        reason = "one arm per action reads best as one table"
    )]
    pub fn name(&self) -> &str {
        match self {
            Self::Open { .. } => "open",
            Self::Goto { .. } => "goto",
            Self::GotoIfNotUrl { .. } => "goto_if_not_url",
            Self::Click { .. } => "click",
            Self::TypeText { .. } => "type_text",
            Self::Clear { .. } => "clear",
            Self::ClickLinkText { .. } => "click_link_text",
            Self::ClickLink { .. } => "click_link",
            Self::ClickPartialLink { .. } => "click_partial_link",
            Self::ClickPartialLinkText { .. } => "click_partial_link_text",
            Self::Submit { .. } => "submit",
            Self::Hover { .. } => "hover",
            Self::SelectOptionByText { .. } => "select_option_by_text",
            Self::SelectOptionByValue { .. } => "select_option_by_value",
            Self::SelectOptionByIndex { .. } => "select_option_by_index",
            Self::SelectAll { .. } => "select_all",
            Self::UnselectAll { .. } => "unselect_all",
            Self::SwitchToFrame { .. } => "switch_to_frame",
            Self::SwitchToDefaultContent => "switch_to_default_content",
            Self::SwitchToParentFrame => "switch_to_parent_frame",
            Self::DragAndDrop { .. } => "drag_and_drop",
            Self::CdpClick { .. } => "cdp_click",
            Self::WaitForElementVisible { .. } => "wait_for_element_visible",
            Self::WaitForElementAbsent { .. } => "wait_for_element_absent",
            Self::WaitForElementNotVisible { .. } => "wait_for_element_not_visible",
            Self::WaitForElementPresent { .. } => "wait_for_element_present",
            Self::WaitForElementClickable { .. } => "wait_for_element_clickable",
            Self::WaitForReadyStateComplete => "wait_for_ready_state_complete",
            Self::AssertText { .. } => "assert_text",
            Self::AssertTextVisible { .. } => "assert_text_visible",
            Self::AssertTextNotVisible { .. } => "assert_text_not_visible",
            Self::AssertExactText { .. } => "assert_exact_text",
            Self::AssertAttribute { .. } => "assert_attribute",
            Self::AssertTitle { .. } => "assert_title",
            Self::AssertTitleContains { .. } => "assert_title_contains",
            Self::AssertLinkText { .. } => "assert_link_text",
            Self::MaximizeWindow => "maximize_window",
            Self::SetWindowSize { .. } => "set_window_size",
            Self::SwitchToWindow { .. } => "switch_to_window",
            Self::SwitchToNewWindow => "switch_to_new_window",
            Self::SwitchToNewestWindow => "switch_to_newest_window",
            Self::SwitchToDefaultWindow => "switch_to_default_window",
            Self::GoBack => "go_back",
            Self::GoForward => "go_forward",
            Self::Refresh => "refresh",
            Self::DeleteAllCookies => "delete_all_cookies",
            Self::DeleteCookie { .. } => "delete_cookie",
            Self::AddCookie { .. } => "add_cookie",
            Self::DoubleClick { .. } => "double_click",
            Self::ContextClick { .. } => "context_click",
            Self::ClickWithOffset { .. } => "click_with_offset",
            Self::DoubleClickWithOffset { .. } => "double_click_with_offset",
            Self::ScrollTo { .. } => "scroll_to",
            Self::ScrollToBottom => "scroll_to_bottom",
            Self::ScrollToTop => "scroll_to_top",
            Self::ScrollIntoView { .. } => "scroll_into_view",
            Self::SmoothScrollTo { .. } => "smooth_scroll_to",
            Self::SlowScrollTo { .. } => "slow_scroll_to",
            Self::CheckIfUnchecked { .. } => "check_if_unchecked",
            Self::UncheckIfChecked { .. } => "uncheck_if_checked",
            Self::AddText { .. } => "add_text",
            Self::SendKeys { .. } => "send_keys",
            Self::JsClick { .. } => "js_click",
            Self::JsType { .. } => "js_type",
            Self::JsUpdateText { .. } => "js_update_text",
            Self::JsClickIfPresent { .. } => "js_click_if_present",
            Self::JsClickAll { .. } => "js_click_all",
            Self::JqueryClick { .. } => "jquery_click",
            Self::JqueryClickAll { .. } => "jquery_click_all",
            Self::JqueryType { .. } => "jquery_type",
            Self::JqueryUpdateText { .. } => "jquery_update_text",
            Self::SetAttribute { .. } => "set_attribute",
            Self::RemoveAttribute { .. } => "remove_attribute",
            Self::ChooseFile { .. } => "choose_file",
            Self::WaitForAndAcceptAlert => "wait_for_and_accept_alert",
            Self::WaitForAndDismissAlert => "wait_for_and_dismiss_alert",
            Self::AcceptAlert => "accept_alert",
            Self::DismissAlert => "dismiss_alert",
            Self::TypeAlertText { .. } => "type_alert_text",
            Self::ClearLocalStorage => "clear_local_storage",
            Self::RemoveLocalStorageItem { .. } => "remove_local_storage_item",
            Self::SetLocalStorageItem { .. } => "set_local_storage_item",
            Self::CloseWindow => "close_window",
            Self::HighlightClick { .. } => "highlight_click",
            Self::OpenNewWindow => "open_new_window",
            Self::ClickVisibleElements { .. } => "click_visible_elements",
            Self::FindElement { .. } => "find_element",
            Self::FindElements { .. } => "find_elements",
            Self::SlowClick { .. } => "slow_click",
            Self::HumanType { .. } => "human_type",
            Self::HumanClick { .. } => "human_click",
            Self::UcClick { .. } => "uc_click",
            Self::UcType { .. } => "uc_type",
            Self::UpdateText { .. } => "update_text",
            Self::Sleep { .. } => "sleep",
            Self::ClickXpath { .. } => "click_xpath",
            Self::Highlight { .. } => "highlight",
            Self::PressKeys { .. } => "press_keys",
            Self::EnterMfaCode { .. } => "enter_mfa_code",
            Self::HoverAndClick { .. } => "hover_and_click",
            Self::SetValue { .. } => "set_value",
            Self::SetContentToFrame { .. } => "set_content_to_frame",
            Self::SetContentToDefault => "set_content_to_default",
            Self::SetContentToParent => "set_content_to_parent",
            Self::WaitForElement { .. } => "wait_for_element",
            Self::AssertElement { .. } => "assert_element",
            Self::AssertElementPresent { .. } => "assert_element_present",
            Self::AssertElementNotVisible { .. } => "assert_element_not_visible",
            Self::AssertNonEmptyText { .. } => "assert_non_empty_text",
            Self::DeferredAssertElement { .. } => "deferred_assert_element",
            Self::DeferredAssertElementPresent { .. } => "deferred_assert_element_present",
            Self::DeferredAssertNonEmptyText { .. } => "deferred_assert_non_empty_text",
            Self::DeferredAssertText { .. } => "deferred_assert_text",
            Self::DeferredAssertExactText { .. } => "deferred_assert_exact_text",
            Self::AssertExactTextNotVisible { .. } => "assert_exact_text_not_visible",
            Self::SetAttributes { .. } => "set_attributes",
            Self::AssertUrl { .. } => "assert_url",
            Self::AssertUrlContains { .. } => "assert_url_contains",
            Self::AssertDownloadedFile { .. } => "assert_downloaded_file",
            Self::SaveScreenshot { .. } => "save_screenshot",
            Self::SaveScreenshotToLogs => "save_screenshot_to_logs",
            Self::SavePageSourceToLogs => "save_page_source_to_logs",
            Self::ShowFileChoosers => "show_file_choosers",
            Self::ProcessDeferredAsserts => "process_deferred_asserts",
            Self::ActivateDemoMode => "activate_demo_mode",
            Self::DeactivateDemoMode => "deactivate_demo_mode",
            Self::ClearSessionStorage => "clear_session_storage",
            Self::OpenNewTab { .. } => "open_new_tab",
            Self::Captcha { .. } => "captcha",
            Self::DragAndDropWithOffset { .. } => "drag_and_drop_with_offset",
            Self::DownloadFile { .. } => "download_file",
            Self::SaveAsPdfToLogs => "save_as_pdf_to_logs",
            Self::SaveScreenshotToFolder { .. } => "save_screenshot_to_folder",
            Self::Unknown { name, .. } | Self::Malformed { name, .. } => name,
        }
    }

    /// The `target` and the `value` of the command in a saved recording: the
    /// pieces [`from_parts`](Self::from_parts) reads, so that
    /// `from_parts(name, target, value)` of them is the same command.
    #[must_use]
    #[expect(
        clippy::too_many_lines,
        reason = "one arm per action reads best as one table"
    )]
    pub fn parts(&self) -> (Option<String>, Option<String>) {
        let own = |text: &String| Some(text.clone());
        match self {
            // No arguments.
            Self::SwitchToDefaultContent
            | Self::SwitchToParentFrame
            | Self::WaitForReadyStateComplete
            | Self::MaximizeWindow
            | Self::SwitchToNewWindow
            | Self::SwitchToNewestWindow
            | Self::SwitchToDefaultWindow
            | Self::GoBack
            | Self::GoForward
            | Self::Refresh
            | Self::DeleteAllCookies
            | Self::ScrollToBottom
            | Self::ScrollToTop
            | Self::WaitForAndAcceptAlert
            | Self::WaitForAndDismissAlert
            | Self::AcceptAlert
            | Self::DismissAlert
            | Self::ClearLocalStorage
            | Self::CloseWindow
            | Self::OpenNewWindow
            | Self::SetContentToDefault
            | Self::SetContentToParent
            | Self::SaveScreenshotToLogs
            | Self::SavePageSourceToLogs
            | Self::ShowFileChoosers
            | Self::ProcessDeferredAsserts
            | Self::ActivateDemoMode
            | Self::DeactivateDemoMode
            | Self::ClearSessionStorage
            | Self::SaveAsPdfToLogs => (None, None),
            // An element, in the target.
            Self::Click { selector }
            | Self::Clear { selector }
            | Self::Submit { selector }
            | Self::Hover { selector }
            | Self::SelectAll { selector }
            | Self::UnselectAll { selector }
            | Self::CdpClick { selector }
            | Self::WaitForElementVisible { selector }
            | Self::WaitForElementAbsent { selector }
            | Self::WaitForElementNotVisible { selector }
            | Self::WaitForElementPresent { selector }
            | Self::WaitForElementClickable { selector }
            | Self::DoubleClick { selector }
            | Self::ContextClick { selector }
            | Self::ScrollTo { selector }
            | Self::ScrollIntoView { selector }
            | Self::SmoothScrollTo { selector }
            | Self::SlowScrollTo { selector }
            | Self::CheckIfUnchecked { selector }
            | Self::UncheckIfChecked { selector }
            | Self::JsClick { selector }
            | Self::JsClickIfPresent { selector }
            | Self::JsClickAll { selector }
            | Self::JqueryClick { selector }
            | Self::JqueryClickAll { selector }
            | Self::HighlightClick { selector }
            | Self::ClickVisibleElements { selector }
            | Self::FindElement { selector }
            | Self::FindElements { selector }
            | Self::SlowClick { selector }
            | Self::HumanClick { selector }
            | Self::UcClick { selector }
            | Self::Highlight { selector }
            | Self::WaitForElement { selector }
            | Self::AssertElement { selector }
            | Self::AssertElementPresent { selector }
            | Self::AssertElementNotVisible { selector }
            | Self::AssertNonEmptyText { selector }
            | Self::DeferredAssertElement { selector }
            | Self::DeferredAssertElementPresent { selector }
            | Self::DeferredAssertNonEmptyText { selector } => (own(selector), None),
            Self::DragAndDropWithOffset { selector, offset } => (own(selector), offset.clone()),
            // An element and a text, in the target and the value.
            Self::TypeText { selector, text }
            | Self::SelectOptionByText { selector, text }
            | Self::AddText { selector, text }
            | Self::JsType { selector, text }
            | Self::JsUpdateText { selector, text }
            | Self::JqueryType { selector, text }
            | Self::JqueryUpdateText { selector, text }
            | Self::HumanType { selector, text }
            | Self::UcType { selector, text }
            | Self::UpdateText { selector, text } => (own(selector), own(text)),
            Self::SelectOptionByValue { selector, value } | Self::SetValue { selector, value } => {
                (own(selector), own(value))
            }
            Self::SendKeys { selector, keys } | Self::PressKeys { selector, keys } => {
                (own(selector), own(keys))
            }
            Self::EnterMfaCode { selector, code } => (own(selector), own(code)),
            Self::RemoveAttribute {
                selector,
                attribute,
            } => (own(selector), own(attribute)),
            Self::ChooseFile { selector, path } => (own(selector), own(path)),
            Self::SelectOptionByIndex { selector, index } => {
                (own(selector), Some(index.to_string()))
            }
            // A text and an element: the element is the target.
            Self::AssertText { text, selector }
            | Self::AssertTextVisible { text, selector }
            | Self::AssertTextNotVisible { text, selector }
            | Self::AssertExactText { text, selector }
            | Self::DeferredAssertText { text, selector }
            | Self::DeferredAssertExactText { text, selector }
            | Self::AssertExactTextNotVisible { text, selector } => (own(selector), own(text)),
            Self::AssertAttribute {
                selector,
                attribute,
                content,
            } => (
                own(selector),
                Some(match content {
                    Some(content) => format!("{attribute}={content}"),
                    None => attribute.clone(),
                }),
            ),
            Self::SetAttribute {
                selector,
                attribute,
                value,
            }
            | Self::SetAttributes {
                selector,
                attribute,
                value,
            } => (own(selector), Some(format!("{attribute}={value}"))),
            Self::ClickWithOffset {
                selector,
                x,
                y,
                center,
            }
            | Self::DoubleClickWithOffset {
                selector,
                x,
                y,
                center,
            } => (
                own(selector),
                Some(if *center {
                    format!("{x}, {y}, center")
                } else {
                    format!("{x}, {y}")
                }),
            ),
            // Other arguments in the target.
            Self::Open { url }
            | Self::Goto { url }
            | Self::GotoIfNotUrl { url }
            | Self::AssertUrl { url }
            | Self::AssertUrlContains { url } => (own(url), None),
            Self::ClickLinkText { link_text }
            | Self::ClickLink { link_text }
            | Self::ClickPartialLink { link_text }
            | Self::ClickPartialLinkText { link_text }
            | Self::AssertLinkText { link_text } => (own(link_text), None),
            Self::SwitchToFrame { frame } | Self::SetContentToFrame { frame } => (own(frame), None),
            Self::AssertTitle { title } | Self::AssertTitleContains { title } => (own(title), None),
            Self::SwitchToWindow { window } => (own(window), None),
            Self::DeleteCookie { name } | Self::AssertDownloadedFile { name } => (own(name), None),
            Self::SaveScreenshot { name, folder }
            | Self::SaveScreenshotToFolder { name, folder } => (own(name), folder.clone()),
            Self::TypeAlertText { text } => (own(text), None),
            Self::RemoveLocalStorageItem { key } => (own(key), None),
            Self::ClickXpath { xpath } => (own(xpath), None),
            Self::SetWindowSize { width, height } => (Some(format!("{width},{height}")), None),
            Self::DragAndDrop {
                source,
                destination,
            } => (own(source), own(destination)),
            Self::HoverAndClick {
                hover_selector,
                click_selector,
            } => (own(hover_selector), own(click_selector)),
            Self::AddCookie { name, value } => (own(name), own(value)),
            Self::SetLocalStorageItem { key, value } => (own(key), own(value)),
            Self::Sleep { seconds } => (None, Some(seconds.to_string())),
            Self::OpenNewTab { url } => (url.clone(), None),
            Self::Captcha { provider } => (provider.clone(), None),
            Self::DownloadFile { url, destination } => (own(url), destination.clone()),
            Self::Unknown { target, value, .. } | Self::Malformed { target, value, .. } => {
                (target.clone(), value.clone())
            }
        }
    }

    /// The target of the command in a saved recording; see
    /// [`parts`](Self::parts).
    #[must_use]
    pub fn target(&self) -> Option<String> {
        self.parts().0
    }

    /// The value of the command in a saved recording; see
    /// [`parts`](Self::parts).
    #[must_use]
    pub fn value(&self) -> Option<String> {
        self.parts().1
    }
}

/// The name, the target and then the value in quotes: `click #go`,
/// `type_text #q "rust"`. A replay report describes each step so.
impl fmt::Display for RecordedCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())?;
        let (target, value) = self.parts();
        if let Some(target) = target {
            write!(f, " {target}")?;
        }
        if let Some(value) = value {
            write!(f, " {value:?}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every name the table knows makes a typed command and keeps its name,
    /// whatever the arguments: the list of names, `parse` and `name` agree.
    #[test]
    fn every_known_name_makes_a_command_of_that_name() {
        let names = CALLABLE_NAMES.iter().copied().chain([
            "captcha",
            "drag_and_drop_with_offset",
            "download_file",
            "save_as_pdf_to_logs",
            "save_screenshot_to_folder",
        ]);
        for name in names {
            // Arguments that every shape accepts: a pair, a size, a number.
            for (target, value) in [
                (Some("1,2"), Some("a=b")),
                (Some("#a"), Some("3, 4")),
                (Some("#a"), Some("2")),
                (Some("#a"), Some("text")),
                (None, None),
            ] {
                let command = RecordedCommand::from_parts(name, target, value);
                assert!(
                    !matches!(command, RecordedCommand::Unknown { .. }),
                    "{name} is a known name"
                );
                assert_eq!(command.name(), name, "{command:?}");
            }
        }
    }

    #[test]
    fn a_name_that_is_not_known_is_kept_as_it_was_recorded() {
        let command = RecordedCommand::from_parts("levitate", Some("#a"), Some("up"));
        assert_eq!(
            command,
            RecordedCommand::Unknown {
                name: "levitate".to_owned(),
                target: Some("#a".to_owned()),
                value: Some("up".to_owned()),
            }
        );
        assert_eq!(command.name(), "levitate");
        assert_eq!(
            command.parts(),
            (Some("#a".to_owned()), Some("up".to_owned()))
        );
    }

    #[test]
    fn the_older_names_make_the_command_of_the_new_one() {
        for (alias, canonical) in ALIASES {
            let old = RecordedCommand::from_parts(alias, Some("#a"), Some("x"));
            let new = RecordedCommand::from_parts(canonical, Some("#a"), Some("x"));
            assert_eq!(old, new, "{alias}");
            assert_eq!(old.name(), *canonical);
        }
    }

    #[test]
    fn a_known_name_with_a_missing_or_unreadable_argument_is_malformed_and_says_why() {
        for (name, target, value, reason) in [
            ("click", None, None, "click has no target"),
            ("type", None, Some("x"), "type_text has no target"),
            ("type_text", Some("#a"), None, "type_text has no value"),
            (
                "assert_attribute",
                Some("#a"),
                None,
                "assert_attribute has no value",
            ),
            (
                "set_attribute",
                Some("#a"),
                Some("no equals sign"),
                "set_attribute: value \"no equals sign\" is not name=content",
            ),
            (
                "set_window_size",
                Some("wide"),
                None,
                "set_window_size: target \"wide\" is not width,height",
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
                "click_with_offset",
                Some("#a"),
                Some("1.5, 2"),
                "click_with_offset: x offset \"1.5\" is not a whole number in range",
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
            (
                "sleep",
                None,
                Some("-1"),
                "sleep: \"-1\" is not a number of seconds",
            ),
        ] {
            let command = RecordedCommand::from_parts(name, target, value);
            let RecordedCommand::Malformed {
                error, name: kept, ..
            } = &command
            else {
                panic!("{name}: expected a malformed command, got {command:?}");
            };
            assert_eq!(error.to_string(), reason, "{name}");
            assert_eq!(kept, name, "the recorded name is kept");
        }
    }

    #[test]
    fn the_wire_form_of_a_command_reads_back_as_the_same_command() {
        let samples = [
            RecordedCommand::from_parts("open", Some("https://a.test/"), None),
            RecordedCommand::from_parts("type_text", Some("#q"), Some("a b\n")),
            RecordedCommand::from_parts("assert_text", Some("h1"), Some("Hi")),
            RecordedCommand::from_parts("assert_attribute", Some("#a"), Some("href=/x")),
            RecordedCommand::from_parts("assert_attribute", Some("#a"), Some("hidden")),
            RecordedCommand::from_parts("set_attribute", Some("#a"), Some("title=a=b")),
            RecordedCommand::from_parts("set_window_size", Some("1280,720"), None),
            RecordedCommand::from_parts("click_with_offset", Some("#c"), Some("3, -7")),
            RecordedCommand::from_parts("click_with_offset", Some("#c"), Some("3, 7, center")),
            RecordedCommand::from_parts("select_option_by_index", Some("#s"), Some("2")),
            RecordedCommand::from_parts("sleep", None, Some("1.5")),
            RecordedCommand::from_parts("open_new_tab", None, None),
            RecordedCommand::from_parts("open_new_tab", Some("https://a.test/"), None),
            RecordedCommand::from_parts("hover_and_click", Some(".menu"), Some(".item")),
            RecordedCommand::from_parts("download_file", Some("https://a.test/f"), Some("dir")),
            // What an action does not run is kept all the same: a saved recording loads back whole.
            RecordedCommand::from_parts("drag_and_drop_with_offset", Some("#a"), Some("10, 20")),
            RecordedCommand::from_parts("save_screenshot", Some("shot.png"), Some("folder")),
            RecordedCommand::from_parts("save_screenshot_to_folder", Some("shot.png"), None),
            RecordedCommand::from_parts("captcha", Some("recaptcha"), None),
            RecordedCommand::from_parts("levitate", Some("#a"), None),
            RecordedCommand::from_parts("click", None, None),
            RecordedCommand::from_parts("sleep", None, Some("soon")),
        ];
        for command in samples {
            let (target, value) = command.parts();
            let back =
                RecordedCommand::from_parts(command.name(), target.as_deref(), value.as_deref());
            assert_eq!(back, command);
        }
    }

    #[test]
    fn a_command_is_described_by_its_name_target_and_value() {
        let command = RecordedCommand::from_parts("type_text", Some("#q"), Some("rust"));
        assert_eq!(command.to_string(), "type_text #q \"rust\"");
        assert_eq!(
            RecordedCommand::from_parts("go_back", None, None).to_string(),
            "go_back"
        );
    }
}
