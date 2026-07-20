use serde::Serialize;
use std::path::{Path, PathBuf};

use crate::SeleniumBaseError;

#[derive(Clone, Debug, Serialize)]
pub struct RecordedAction {
    pub name: String,
    pub target: Option<String>,
    pub value: Option<String>,
    pub timestamp_ms: u128,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct ActionRecorder {
    pub actions: Vec<RecordedAction>,
}

impl ActionRecorder {
    pub fn record(&mut self, name: &str, target: Option<&str>, value: Option<&str>) {
        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        self.actions.push(RecordedAction {
            name: name.to_owned(),
            target: target.map(ToOwned::to_owned),
            value: value.map(ToOwned::to_owned),
            timestamp_ms,
        });
    }

    pub fn to_rust_script(&self) -> String {
        let mut out = String::from(
            "use seleniumbase_rs::{BaseCase, BrowserConfig};\n\n#[tokio::main]\nasync fn main() -> Result<(), Box<dyn std::error::Error>> {\n    let mut sb = BaseCase::new(BrowserConfig::default()).await?;\n",
        );
        self.write_actions(&mut out);
        out.push_str("    sb.quit().await?;\n    Ok(())\n}\n");
        out
    }

    pub fn to_rust_test(&self, test_name: &str) -> String {
        let fn_name = rust_identifier(test_name);
        let mut out =
            String::from("use seleniumbase_rs::{BaseCase, BrowserConfig};\n\n#[tokio::test]\n");
        out.push_str(&format!(
            "async fn {}() -> Result<(), Box<dyn std::error::Error>> {{\n    let mut sb = BaseCase::new(BrowserConfig::default()).await?;\n",
            fn_name
        ));
        self.write_actions(&mut out);
        out.push_str("    sb.quit().await?;\n    Ok(())\n}\n");
        out
    }

    pub fn save_recording_as_test(
        &self,
        dir: &Path,
        name: &str,
    ) -> Result<PathBuf, SeleniumBaseError> {
        std::fs::create_dir_all(dir)?;
        let path = dir.join(format!("{name}.rs"));
        std::fs::write(&path, self.to_rust_test(name))?;
        Ok(path)
    }

    fn write_actions(&self, out: &mut String) {
        for action in &self.actions {
            self.write_action(out, action);
        }
    }

    #[allow(clippy::too_many_lines)]
    fn write_action(&self, out: &mut String, action: &RecordedAction) {
        let target = action.target.as_deref();
        let value = action.value.as_deref();

        match action.name.as_str() {
            "open" => {
                if let Some(url) = target {
                    line(out, &format!("sb.open({:?}).await?;", url));
                }
            }
            "click" => {
                if let Some(css) = target {
                    line(out, &format!("sb.click({:?}).await?;", css));
                }
            }
            "type_text" => {
                if let (Some(css), Some(text)) = (target, value) {
                    line(out, &format!("sb.type_text({:?}, {:?}).await?;", css, text));
                }
            }
            "clear" => {
                if let Some(css) = target {
                    line(out, &format!("sb.clear({:?}).await?;", css));
                }
            }
            "click_link_text" => {
                if let Some(text) = target {
                    line(out, &format!("sb.click_link_text({:?}).await?;", text));
                }
            }
            "submit" => {
                if let Some(css) = target {
                    line(out, &format!("sb.submit({:?}).await?;", css));
                }
            }
            "hover" => {
                if let Some(css) = target {
                    line(out, &format!("sb.hover({:?}).await?;", css));
                }
            }
            "select_option_by_text" => {
                if let (Some(css), Some(text)) = (target, value) {
                    line(
                        out,
                        &format!("sb.select_option_by_text({:?}, {:?}).await?;", css, text),
                    );
                }
            }
            "select_option_by_value" => {
                if let (Some(css), Some(val)) = (target, value) {
                    line(
                        out,
                        &format!("sb.select_option_by_value({:?}, {:?}).await?;", css, val),
                    );
                }
            }
            "switch_to_frame" => {
                if let Some(css) = target {
                    line(out, &format!("sb.switch_to_frame({:?}).await?;", css));
                }
            }
            "switch_to_default_content" => {
                line(out, "sb.switch_to_default_content().await?;");
            }
            "drag_and_drop" => {
                if let (Some(source), Some(dest)) = (target, value) {
                    line(
                        out,
                        &format!("sb.drag_and_drop({:?}, {:?}).await?;", source, dest),
                    );
                }
            }
            "cdp_click_element" => {
                if let Some(css) = target {
                    line(out, &format!("sb.cdp_click_element({:?}).await?;", css));
                }
            }
            "wait_for_element_visible" => {
                if let Some(css) = target {
                    line(
                        out,
                        &format!("sb.wait_for_element_visible({:?}, 10).await?;", css),
                    );
                }
            }
            "wait_for_element_absent" => {
                if let Some(css) = target {
                    line(
                        out,
                        &format!("sb.wait_for_element_absent({:?}, 10).await?;", css),
                    );
                }
            }
            "wait_for_element_not_visible" => {
                if let Some(css) = target {
                    line(
                        out,
                        &format!("sb.wait_for_element_not_visible({:?}, 10).await?;", css),
                    );
                }
            }
            "wait_for_element_present" => {
                if let Some(css) = target {
                    line(
                        out,
                        &format!("sb.wait_for_element_present({:?}, 10).await?;", css),
                    );
                }
            }
            "wait_for_element_clickable" => {
                if let Some(css) = target {
                    line(
                        out,
                        &format!("sb.wait_for_element_clickable({:?}, 10).await?;", css),
                    );
                }
            }
            "wait_for_ready_state_complete" => {
                line(out, "sb.wait_for_ready_state_complete().await?;");
            }
            "assert_text" => {
                if let (Some(css), Some(text)) = (target, value) {
                    line(
                        out,
                        &format!("sb.assert_text({:?}, {:?}).await?;", css, text),
                    );
                }
            }
            "assert_text_visible" => {
                if let (Some(css), Some(text)) = (target, value) {
                    line(
                        out,
                        &format!("sb.assert_text_visible({:?}, {:?}).await?;", text, css),
                    );
                }
            }
            "assert_text_not_visible" => {
                if let (Some(css), Some(text)) = (target, value) {
                    line(
                        out,
                        &format!("sb.assert_text_not_visible({:?}, {:?}).await?;", text, css),
                    );
                }
            }
            "assert_exact_text" => {
                if let (Some(css), Some(text)) = (target, value) {
                    line(
                        out,
                        &format!("sb.assert_exact_text({:?}, {:?}).await?;", css, text),
                    );
                }
            }
            "assert_attribute" => {
                if let (Some(css), Some(pair)) = (target, value) {
                    if let Some((attribute, expected)) = pair.split_once('=') {
                        line(
                            out,
                            &format!(
                                "sb.assert_attribute({:?}, {:?}, {:?}).await?;",
                                css,
                                attribute.trim(),
                                expected.trim()
                            ),
                        );
                    }
                }
            }
            "assert_title" => {
                if let Some(text) = target {
                    line(out, &format!("sb.assert_title({:?}).await?;", text));
                }
            }
            "assert_title_contains" => {
                if let Some(text) = target {
                    line(
                        out,
                        &format!("sb.assert_title_contains({:?}).await?;", text),
                    );
                }
            }
            "assert_link_text" => {
                if let Some(text) = target {
                    line(out, &format!("sb.assert_link_text({:?}).await?;", text));
                }
            }
            "maximize_window" => {
                line(out, "sb.maximize_window().await?;");
            }
            "set_window_size" => {
                if let Some(pair) = target {
                    if let Some((width, height)) = pair.split_once(',') {
                        line(
                            out,
                            &format!(
                                "sb.set_window_size({}, {}).await?;",
                                width.trim(),
                                height.trim()
                            ),
                        );
                    }
                }
            }
            "switch_to_window" => {
                if let Some(handle) = target {
                    line(out, &format!("sb.switch_to_window({:?}).await?;", handle));
                }
            }
            "switch_to_new_window" => {
                line(out, "sb.switch_to_new_window().await?;");
            }
            "switch_to_newest_window" => {
                line(out, "sb.switch_to_newest_window().await?;");
            }
            "switch_to_default_window" => {
                line(out, "sb.switch_to_default_window().await?;");
            }
            "switch_to_parent_frame" => {
                line(out, "sb.switch_to_parent_frame().await?;");
            }
            "go_back" => {
                line(out, "sb.go_back().await?;");
            }
            "go_forward" => {
                line(out, "sb.go_forward().await?;");
            }
            "refresh" => {
                line(out, "sb.refresh().await?;");
            }
            "delete_all_cookies" => {
                line(out, "sb.delete_all_cookies().await?;");
            }
            "delete_cookie" => {
                if let Some(name) = target {
                    line(out, &format!("sb.delete_cookie({:?}).await?;", name));
                }
            }
            "add_cookie" => {
                if let (Some(name), Some(val)) = (target, value) {
                    line(
                        out,
                        &format!("sb.add_cookie({:?}, {:?}).await?;", name, val),
                    );
                }
            }
            "double_click" => {
                if let Some(css) = target {
                    line(out, &format!("sb.double_click({:?}).await?;", css));
                }
            }
            "context_click" => {
                if let Some(css) = target {
                    line(out, &format!("sb.context_click({:?}).await?;", css));
                }
            }
            "scroll_to" => {
                if let Some(css) = target {
                    line(out, &format!("sb.scroll_to({:?}).await?;", css));
                }
            }
            "scroll_to_bottom" => {
                line(out, "sb.scroll_to_bottom().await?;");
            }
            "scroll_to_top" => {
                line(out, "sb.scroll_to_top().await?;");
            }
            "smooth_scroll_to" => {
                if let Some(css) = target {
                    line(out, &format!("sb.smooth_scroll_to({:?}).await?;", css));
                }
            }
            "check_if_unchecked" => {
                if let Some(css) = target {
                    line(out, &format!("sb.check_if_unchecked({:?}).await?;", css));
                }
            }
            "uncheck_if_checked" => {
                if let Some(css) = target {
                    line(out, &format!("sb.uncheck_if_checked({:?}).await?;", css));
                }
            }
            "add_text" => {
                if let (Some(css), Some(text)) = (target, value) {
                    line(out, &format!("sb.add_text({:?}, {:?}).await?;", css, text));
                }
            }
            "send_keys" => {
                if let (Some(css), Some(text)) = (target, value) {
                    line(out, &format!("sb.send_keys({:?}, {:?}).await?;", css, text));
                }
            }
            "js_click" => {
                if let Some(css) = target {
                    line(out, &format!("sb.js_click({:?}).await?;", css));
                }
            }
            "js_type" => {
                if let (Some(css), Some(text)) = (target, value) {
                    line(out, &format!("sb.js_type({:?}, {:?}).await?;", css, text));
                }
            }
            "js_click_if_present" => {
                if let Some(css) = target {
                    line(out, &format!("sb.js_click_if_present({:?}).await?;", css));
                }
            }
            "js_click_all" => {
                if let Some(css) = target {
                    line(out, &format!("sb.js_click_all({:?}).await?;", css));
                }
            }
            "jquery_click" => {
                if let Some(css) = target {
                    line(out, &format!("sb.jquery_click({:?}).await?;", css));
                }
            }
            "set_attribute" => {
                if let (Some(css), Some(pair)) = (target, value) {
                    if let Some((attribute, new_value)) = pair.split_once('=') {
                        line(
                            out,
                            &format!(
                                "sb.set_attribute({:?}, {:?}, {:?}).await?;",
                                css,
                                attribute.trim(),
                                new_value.trim()
                            ),
                        );
                    }
                }
            }
            "remove_attribute" => {
                if let (Some(css), Some(attribute)) = (target, value) {
                    line(
                        out,
                        &format!("sb.remove_attribute({:?}, {:?}).await?;", css, attribute),
                    );
                }
            }
            "choose_file" => {
                if let (Some(css), Some(path)) = (target, value) {
                    line(
                        out,
                        &format!("sb.choose_file({:?}, {:?}).await?;", css, path),
                    );
                }
            }
            "click_partial_link_text" => {
                if let Some(text) = target {
                    line(
                        out,
                        &format!("sb.click_partial_link_text({:?}).await?;", text),
                    );
                }
            }
            "wait_for_and_accept_alert" => {
                line(out, "sb.wait_for_and_accept_alert(10).await?;");
            }
            "wait_for_and_dismiss_alert" => {
                line(out, "sb.wait_for_and_dismiss_alert(10).await?;");
            }
            "accept_alert" => {
                line(out, "sb.accept_alert().await?;");
            }
            "dismiss_alert" => {
                line(out, "sb.dismiss_alert().await?;");
            }
            "type_alert_text" => {
                if let Some(text) = target {
                    line(out, &format!("sb.type_alert_text({:?}).await?;", text));
                }
            }
            "clear_local_storage" => {
                line(out, "sb.clear_local_storage().await?;");
            }
            "remove_local_storage_item" => {
                if let Some(key) = target {
                    line(
                        out,
                        &format!("sb.remove_local_storage_item({:?}).await?;", key),
                    );
                }
            }
            "set_local_storage_item" => {
                if let (Some(key), Some(val)) = (target, value) {
                    line(
                        out,
                        &format!("sb.set_local_storage_item({:?}, {:?}).await?;", key, val),
                    );
                }
            }
            "close_window" => {
                line(out, "sb.close_window().await?;");
            }
            "highlight_click" => {
                if let Some(css) = target {
                    line(out, &format!("sb.highlight_click({:?}).await?;", css));
                }
            }
            "open_new_window" => {
                line(out, "sb.open_new_window().await?;");
            }
            "open_new_tab" => {
                line(out, "sb.open_new_tab().await?;");
            }
            "click_visible_elements" => {
                if let Some(css) = target {
                    line(
                        out,
                        &format!("sb.click_visible_elements({:?}).await?;", css),
                    );
                }
            }
            "find_element" => {
                if let Some(css) = target {
                    line(out, &format!("let _ = sb.find_element({:?}).await?;", css));
                }
            }
            "find_elements" => {
                if let Some(css) = target {
                    line(out, &format!("let _ = sb.find_elements({:?}).await?;", css));
                }
            }
            "get_shadow_root" => {
                if let Some(css) = target {
                    line(
                        out,
                        &format!("let _ = sb.get_shadow_root({:?}).await?;", css),
                    );
                }
            }
            "slow_click" => {
                if let Some(css) = target {
                    line(out, &format!("sb.slow_click({:?}).await?;", css));
                }
            }
            "human_type" => {
                if let (Some(css), Some(text)) = (target, value) {
                    line(
                        out,
                        &format!("sb.human_type({:?}, {:?}).await?;", css, text),
                    );
                }
            }
            "human_click" => {
                if let Some(css) = target {
                    line(out, &format!("sb.human_click({:?}).await?;", css));
                }
            }
            "uc_click" => {
                if let Some(css) = target {
                    line(out, &format!("sb.uc_click({:?}).await?;", css));
                }
            }
            "uc_type" => {
                if let (Some(css), Some(text)) = (target, value) {
                    line(out, &format!("sb.uc_type({:?}, {:?}).await?;", css, text));
                }
            }
            "update_text" => {
                if let (Some(css), Some(text)) = (target, value) {
                    line(
                        out,
                        &format!("sb.update_text({:?}, {:?}).await?;", css, text),
                    );
                }
            }
            "click_xpath" => {
                if let Some(xpath) = target {
                    line(out, &format!("sb.click_xpath({:?}).await?;", xpath));
                }
            }
            _ => {}
        }
    }
}

fn line(out: &mut String, content: &str) {
    out.push_str("    ");
    out.push_str(content);
    out.push('\n');
}

fn rust_identifier(name: &str) -> String {
    let mut ident: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if ident.is_empty() {
        ident.push_str("recorded_test");
    }
    if ident.chars().next().unwrap().is_ascii_digit() {
        ident.insert(0, '_');
    }
    ident
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_recorder() -> ActionRecorder {
        let mut recorder = ActionRecorder::default();
        recorder.record("open", Some("https://example.com"), None);
        recorder.record("click", Some("#login"), None);
        recorder.record("type_text", Some("#username"), Some("user"));
        recorder.record("add_text", Some("#username"), Some("@example.com"));
        recorder.record("clear", Some("#search"), None);
        recorder.record("submit", Some("#form"), None);
        recorder.record("hover", Some("#menu"), None);
        recorder.record(
            "select_option_by_text",
            Some("#country"),
            Some("United States"),
        );
        recorder.record("select_option_by_value", Some("#country"), Some("us"));
        recorder.record("switch_to_frame", Some("#frame"), None);
        recorder.record("switch_to_default_content", None, None);
        recorder.record("drag_and_drop", Some("#source"), Some("#target"));
        recorder.record("cdp_click_element", Some("#shadow-btn"), None);
        recorder.record("wait_for_element_visible", Some("#result"), None);
        recorder.record("wait_for_element_absent", Some("#spinner"), None);
        recorder.record("assert_text", Some("#msg"), Some("Welcome"));
        recorder.record("assert_text_visible", Some("#msg"), Some("Welcome"));
        recorder.record("assert_text_not_visible", Some("#error"), Some("Failed"));
        recorder.record("assert_attribute", Some("#logo"), Some("src=/logo.png"));
        recorder.record("assert_title", Some("Example"), None);
        recorder.record("assert_link_text", Some("Home"), None);
        recorder.record("maximize_window", None, None);
        recorder.record("switch_to_window", Some("handle-123"), None);
        recorder.record("switch_to_new_window", None, None);
        recorder.record("go_back", None, None);
        recorder.record("go_forward", None, None);
        recorder.record("refresh", None, None);
        recorder.record("delete_all_cookies", None, None);
        recorder.record("double_click", Some("#item"), None);
        recorder.record("context_click", Some("#item"), None);
        recorder.record("scroll_to", Some("#footer"), None);
        recorder.record("scroll_to_bottom", None, None);
        recorder.record("scroll_to_top", None, None);
        recorder.record("check_if_unchecked", Some("#agree"), None);
        recorder.record("uncheck_if_checked", Some("#spam"), None);
        recorder.record("send_keys", Some("#search"), Some("hello"));
        recorder.record("js_click", Some("#btn"), None);
        recorder.record("js_type", Some("#hidden"), Some("value"));
        recorder.record("set_attribute", Some("#x"), Some("data-id=42"));
        recorder.record("choose_file", Some("#upload"), Some("/tmp/file.txt"));
        recorder.record("click_partial_link_text", Some("Terms"), None);
        recorder.record("wait_for_and_accept_alert", None, None);
        recorder.record("wait_for_and_dismiss_alert", None, None);
        recorder.record("click_link_text", Some("Logout"), None);
        recorder.record("set_window_size", Some("1280,720"), None);
        recorder.record("add_cookie", Some("session"), Some("abc"));
        recorder.record("delete_cookie", Some("session"), None);
        recorder.record("open_new_tab", None, None);
        recorder.record("click_visible_elements", Some(".btn"), None);
        recorder.record("find_element", Some("#x"), None);
        recorder.record("find_elements", Some(".x"), None);
        recorder.record("get_shadow_root", Some("#host"), None);
        recorder.record("slow_click", Some("#y"), None);
        recorder.record("human_type", Some("#z"), Some("abc"));
        recorder.record("human_click", Some("#z"), None);
        recorder.record("update_text", Some("#w"), Some("new"));
        recorder.record("click_xpath", Some("//button"), None);
        recorder.record("js_click_all", Some(".check"), None);
        recorder.record("jquery_click", Some(".jq"), None);
        recorder
    }

    #[test]
    fn generated_rust_script_is_syntactically_valid() {
        let source = sample_recorder().to_rust_script();
        syn::parse_file(&source).expect("generated main script should parse as Rust");
    }

    #[test]
    fn generated_rust_test_is_syntactically_valid() {
        let recorder = sample_recorder();
        let source = recorder.to_rust_test("login_flow_test");
        assert!(source.contains("#[tokio::test]"));
        assert!(source.contains("async fn login_flow_test()"));
        syn::parse_file(&source).expect("generated test should parse as Rust");
    }

    #[test]
    fn save_recording_as_test_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = sample_recorder()
            .save_recording_as_test(dir.path(), "sample_test")
            .unwrap();
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("async fn sample_test()"));
    }

    #[test]
    fn rust_identifier_sanitizes_names() {
        assert_eq!(rust_identifier("my test"), "my_test");
        assert_eq!(rust_identifier("123-test"), "_123_test");
        assert_eq!(rust_identifier(""), "recorded_test");
    }
}
