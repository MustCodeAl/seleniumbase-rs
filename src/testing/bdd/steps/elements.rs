//! Steps that act on elements: clicks, typing, selecting, hovering, dragging,
//! check boxes, keys, and the JavaScript and jQuery variants.
//!
//! rstest-bdd hands the captures of a pattern to the parameters of the step
//! function in the order they appear in the pattern, so an action that has a
//! wording with its arguments the other way round (`In "#a" type "x"` and
//! `Type "x" into "#a"`) has a second function for it, named `as` the action.

use super::{normalize_text, trim_quotes};
use crate::error::SeleniumBaseError;

sb_steps! {
    /// Clicks an element.
    fn click_element(sb, selector: String) { sb.click(&selector).await }
        "Click {selector:string}", "Click element {selector:string}",
        "User clicks {selector:string}", "User clicks element {selector:string}";

    /// Types text into a field, replacing what it holds.
    fn type_text(sb, text: String, selector: String) {
        sb.type_text(&selector, &normalize_text(&text)).await
    }
        "Type text {text:string} into {selector:string}", "Type text {text:string} in {selector:string}",
        "Type {text:string} into {selector:string}", "Type {text:string} in {selector:string}",
        "User types {text:string} in {selector:string}",
        "User types {text:string} into {selector:string}";

    /// `type_text` with the selector first.
    fn type_text_selector_first as type_text(sb, selector: String, text: String) {
        type_text(sb, text, selector).await
    }
        "In {selector:string} type {text:string}", "Into {selector:string} type {text:string}",
        "Find {selector:string} and type {text:string}";

    /// Adds text to a field.
    fn add_text(sb, text: String, selector: String) {
        sb.add_text(&selector, &normalize_text(&text)).await
    }
        "Add text {text:string} into {selector:string}", "Add text {text:string} in {selector:string}",
        "Add {text:string} into {selector:string}", "Add {text:string} in {selector:string}",
        "User adds {text:string} in {selector:string}",
        "User adds {text:string} into {selector:string}";

    /// `add_text` with the selector first.
    fn add_text_selector_first as add_text(sb, selector: String, text: String) {
        add_text(sb, text, selector).await
    }
        "Into {selector:string} add {text:string}", "In {selector:string} add {text:string}";

    /// Highlights an element.
    fn highlight_element(sb, selector: String) { sb.highlight(&selector).await }
        "Highlight {selector:string}", "Highlight element {selector:string}",
        "Use JS to highlight {selector:string}";

    /// Clicks a link by its text.
    fn click_link(sb, link: String) { sb.click_link(&link).await }
        "Click link {link:string}", "User clicks link {link:string}";

    /// Clicks an element with JavaScript.
    fn js_click(sb, selector: String) { sb.js_click(&selector).await }
        "JS click {selector:string}", "JS click element {selector:string}",
        "Use JS to click {selector:string}";

    /// Clicks every matching element with JavaScript.
    fn js_click_all(sb, selector: String) { sb.js_click_all(&selector).await }
        "JS click all {selector:string}", "Use JS to click all {selector:string}";

    /// Sets the value of a field.
    fn set_value(sb, selector: String, text: String) {
        sb.set_value(&selector, &normalize_text(&text)).await
    }
        "Set value of {selector:string} to {text:string}",
        "User sets value of {selector:string} to {text:string}";

    /// Selects an option, a radio button or a check box, unless it is.
    fn select_if_unselected(sb, selector: String) { sb.select_if_unselected(&selector).await }
        "Select if unselected {selector:string}", "Select {selector:string} if unselected",
        "User selects {selector:string} if unselected";

    /// Unselects it, if it is selected.
    fn unselect_if_selected(sb, selector: String) { sb.unselect_if_selected(&selector).await }
        "Unselect if selected {selector:string}", "Unselect {selector:string} if selected",
        "User unselects {selector:string} if selected";

    /// Checks a check box, unless it is.
    fn check_if_unchecked(sb, selector: String) { sb.check_if_unchecked(&selector).await }
        "Check if unchecked {selector:string}", "Check {selector:string} if unchecked",
        "User checks {selector:string} if unchecked";

    /// Unchecks a check box, if it is.
    fn uncheck_if_checked(sb, selector: String) { sb.uncheck_if_checked(&selector).await }
        "Uncheck if checked {selector:string}", "Uncheck {selector:string} if checked",
        "User unchecks {selector:string} if checked";

    /// Drags an element onto another.
    fn drag_and_drop(sb, drag_selector: String, drop_selector: String) {
        sb.drag_and_drop(&drag_selector, &drop_selector).await
    }
        "Drag {drag_selector:string} into {drop_selector:string}",
        "User drags {drag_selector:string} into {drop_selector:string}";

    /// Hovers over an element and clicks another that appears.
    fn hover_and_click(sb, hover_selector: String, click_selector: String) {
        sb.hover_and_click(&hover_selector, &click_selector).await
    }
        "Hover {hover_selector:string} and click {click_selector:string}",
        "User hovers {hover_selector:string} and clicks {click_selector:string}";

    /// Selects an option of a drop-down list by its text.
    fn select_option_by_text(sb, selector: String, text: String) {
        sb.select_option_by_text(&selector, &normalize_text(&text)).await
    }
        "Find {selector:string} and select {text:string}",
        "User finds {selector:string} and selects {text:string}";

    /// `select_option_by_text` with the text first.
    fn select_option_by_text_text_first as select_option_by_text(
        sb, text: String, selector: String
    ) {
        select_option_by_text(sb, selector, text).await
    }
        "User selects {text:string} in {selector:string}";

    /// Selects an option by its text, index or value, as `option` says.
    fn select_option_by_option(sb, selector: String, text: String, option: String) {
        let text = normalize_text(&text);
        match trim_quotes(&option) {
            "text" => sb.select_option_by_text(&selector, &text).await,
            "index" => {
                let index = text.trim().parse::<usize>().map_err(|_| {
                    SeleniumBaseError::InvalidConfig(format!("Not an option index: {text}"))
                })?;
                sb.select_option_by_index(&selector, index).await
            }
            "value" => sb.select_option_by_value(&selector, &text).await,
            other => Err(SeleniumBaseError::Unsupported(format!("Unknown option: {other}"))),
        }
    }
        "Find {selector:string} and select {text:string} by {option}",
        "User finds {selector:string} and selects {text:string} by {option}";

    /// Double-clicks an element.
    fn double_click_element(sb, selector: String) { sb.double_click(&selector).await }
        "Double click {selector:string}", "Double click element {selector:string}",
        "User double clicks {selector:string}", "User double clicks element {selector:string}";

    /// Clicks an element slowly.
    fn slow_click_element(sb, selector: String) { sb.slow_click(&selector).await }
        "Slow click {selector:string}", "Slow click element {selector:string}",
        "User slow clicks {selector:string}", "User slow clicks element {selector:string}";

    /// Clears a text field.
    fn clear_text_field(sb, selector: String) { sb.clear(&selector).await }
        "Clear text field {selector:string}", "Clear text in {selector:string}",
        "User clears text field {selector:string}", "User clears text in {selector:string}";

    /// Presses the up arrow key.
    fn press_up_arrow(sb) { sb.press_up_arrow().await }
        "Press up arrow", "User presses up arrow";

    /// Presses the down arrow key.
    fn press_down_arrow(sb) { sb.press_down_arrow().await }
        "Press down arrow", "User presses down arrow";

    /// Presses the left arrow key.
    fn press_left_arrow(sb) { sb.press_left_arrow().await }
        "Press left arrow", "User presses left arrow";

    /// Presses the right arrow key.
    fn press_right_arrow(sb) { sb.press_right_arrow().await }
        "Press right arrow", "User presses right arrow";

    /// Clicks an element at an offset from its top left corner.
    fn click_with_offset(sb, selector: String, px: i64, py: i64) {
        sb.click_with_offset(&selector, px as f64, py as f64).await
    }
        "Click {selector:string} at ({px:i64},{py:i64})",
        "Click {selector:string} at ({px:i64}, {py:i64})",
        "User clicks {selector:string} at ({px:i64},{py:i64})",
        "User clicks {selector:string} at ({px:i64}, {py:i64})";

    /// Chooses a file in a file input.
    fn choose_file(sb, selector: String, file_path: String) {
        sb.choose_file(&selector, &file_path).await
    }
        "In {selector:string} choose file {file_path:string}",
        "Into {selector:string} choose file {file_path:string}";

    /// `choose_file` with the path first.
    fn choose_file_path_first as choose_file(sb, file_path: String, selector: String) {
        choose_file(sb, selector, file_path).await
    }
        "User chooses file {file_path:string} for {selector:string}";

    /// Shows the file inputs a page hides.
    fn show_file_choosers(sb) { sb.show_file_choosers().await }
        "Show file choosers", "Show hidden file choosers", "Use JS to show file choosers",
        "Use JS to show hidden file choosers";

    /// Hovers over an element.
    fn hover(sb, selector: String) { sb.hover(&selector).await }
        "Hover {selector:string}", "Hover over {selector:string}", "Hover element {selector:string}",
        "User hovers over {selector:string}", "User hovers over element {selector:string}";

    /// Right-clicks an element.
    fn context_click(sb, selector: String) { sb.context_click(&selector).await }
        "Context click {selector:string}", "Context click element {selector:string}",
        "Right click {selector:string}", "Right click element {selector:string}",
        "User right clicks {selector:string}", "User right clicks element {selector:string}";

    /// Types text with JavaScript.
    fn js_type(sb, text: String, selector: String) {
        sb.js_type(&selector, &normalize_text(&text)).await
    }
        "JS type {text:string} in {selector:string}", "JS type {text:string} into {selector:string}",
        "JS type text {text:string} in {selector:string}",
        "JS type text {text:string} into {selector:string}",
        "Use JS to type {text:string} in {selector:string}",
        "Use JS to type {text:string} into {selector:string}";

    /// Clicks an element with jQuery.
    fn jquery_click(sb, selector: String) { sb.jquery_click(&selector).await }
        "jQuery click {selector:string}", "jQuery click element {selector:string}",
        "Use jQuery to click {selector:string}";

    /// Clicks every matching element with jQuery.
    fn jquery_click_all(sb, selector: String) { sb.jquery_click_all(&selector).await }
        "jQuery click all {selector:string}", "Use jQuery to click all {selector:string}";

    /// Types text with jQuery.
    fn jquery_type(sb, text: String, selector: String) {
        sb.jquery_type(&selector, &normalize_text(&text)).await
    }
        "jQuery type {text:string} in {selector:string}",
        "jQuery type {text:string} into {selector:string}",
        "jQuery type text {text:string} in {selector:string}",
        "jQuery type text {text:string} into {selector:string}",
        "Use jQuery to type {text:string} in {selector:string}",
        "Use jQuery to type {text:string} into {selector:string}";

    /// Presses keys in an element.
    fn press_keys(sb, text: String, selector: String) {
        sb.press_keys(&selector, &normalize_text(&text)).await
    }
        "Press keys {text:string} in {selector:string}", "Press keys {text:string} into {selector:string}",
        "User presses keys {text:string} in {selector:string}",
        "User presses keys {text:string} into {selector:string}";

    /// `press_keys` with the selector first.
    fn press_keys_selector_first as press_keys(sb, selector: String, text: String) {
        press_keys(sb, text, selector).await
    }
        "In {selector:string} press keys {text:string}",
        "Into {selector:string} press keys {text:string}",
        "Find {selector:string} and press keys {text:string}";

    /// Sets an attribute of an element.
    fn set_attribute(sb, selector: String, attribute: String, value: String) {
        sb.set_attribute(&selector, trim_quotes(&attribute), &normalize_text(&value)).await
    }
        "Find {selector:string} and set {attribute} to {value:string}";

    /// Sets an attribute of every matching element.
    fn set_attributes(sb, selector: String, attribute: String, value: String) {
        sb.set_attributes(&selector, trim_quotes(&attribute), &normalize_text(&value)).await
    }
        "Find all {selector:string} and set {attribute} to {value:string}";

    /// Types the one-time code of an authenticator secret.
    fn enter_mfa_code(sb, selector: String, totp_key: String) {
        sb.enter_mfa_code(&selector, &totp_key).await
    }
        "Into {selector:string} enter MFA code {totp_key:string}",
        "Into {selector:string} do MFA {totp_key:string}";

    /// `enter_mfa_code` with the key first.
    fn enter_mfa_code_key_first as enter_mfa_code(sb, totp_key: String, selector: String) {
        enter_mfa_code(sb, selector, totp_key).await
    }
        "Do MFA {totp_key:string} into {selector:string}",
        "Enter MFA code {totp_key:string} into {selector:string}",
        "User enters MFA code {totp_key:string} into {selector:string}";
}
