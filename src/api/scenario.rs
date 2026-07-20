#[derive(Debug, Default, Clone)]
pub struct RunSummary {
    pub scenario_name: String,
    pub total_steps: usize,
    pub passed_steps: usize,
    pub failed_steps: usize,
    pub errors: Vec<String>,
}

use serde::Deserialize;

// use crate::utils::dashboard::RunSummary;
use crate::api::base_case::BaseCase;
use crate::error::SeleniumBaseError;

#[derive(Debug, Deserialize)]
pub struct Scenario {
    pub name: String,
    pub steps: Vec<ScenarioStep>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ScenarioStep {
    Open {
        url: String,
    },
    Click {
        css: String,
    },
    TypeText {
        css: String,
        text: String,
    },
    AssertText {
        css: String,
        contains: String,
    },
    AssertElement {
        css: String,
    },
    WaitForText {
        css: String,
        text: String,
        timeout: u64,
    },
    Sleep {
        seconds: f64,
    },
    Hover {
        css: String,
    },
    HoverAndClick {
        hover_css: String,
        click_css: String,
    },
    SelectOptionByText {
        css: String,
        text: String,
    },
    SelectOptionByValue {
        css: String,
        value: String,
    },
    DragAndDrop {
        source_css: String,
        target_css: String,
    },
    SwitchToFrame {
        css: String,
    },
    SwitchToDefaultContent,
    Clear {
        css: String,
    },
    ClickLinkText {
        text: String,
    },
    Submit {
        css: String,
    },
    AcceptAlert,
    DismissAlert,
    TypeAlertText {
        text: String,
    },
    ClearLocalStorage,
    RemoveLocalStorageItem {
        key: String,
    },
    SetLocalStorageItem {
        key: String,
        value: String,
    },
    SwitchToWindow {
        handle: String,
    },

    JsClick {
        css: String,
    },
    JsType {
        css: String,
        text: String,
    },
    SetAttribute {
        css: String,
        attribute: String,
        value: String,
    },
    RemoveAttribute {
        css: String,
        attribute: String,
    },
    ChooseFile {
        css: String,
        file_path: String,
    },
    GoBack,
    GoForward,
    Refresh,
    DeleteAllCookies,
    SwitchToNewWindow,

    DoubleClick {
        css: String,
    },
    ContextClick {
        css: String,
    },
    MaximizeWindow,
    ScrollToBottom,
    ScrollToTop,
    ScrollTo {
        css: String,
    },
}

pub async fn run_scenario(
    sb: &mut BaseCase,
    scenario: &Scenario,
) -> Result<RunSummary, SeleniumBaseError> {
    let mut passed_steps = 0_usize;
    let mut errors = Vec::new();

    for step in &scenario.steps {
        let result = match step {
            ScenarioStep::Open { url } => sb.open(url).await,
            ScenarioStep::Click { css } => sb.click(css).await,
            ScenarioStep::TypeText { css, text } => sb.type_text(css, text).await,
            ScenarioStep::AssertText { css, contains } => sb.assert_text(css, contains).await,
            ScenarioStep::AssertElement { css } => sb.assert_element(css).await,
            ScenarioStep::WaitForText { css, text, timeout } => {
                sb.wait_for_text(css, text, *timeout).await
            }
            ScenarioStep::Sleep { seconds } => {
                sb.sleep(*seconds).await;
                Ok(())
            }
            ScenarioStep::Hover { css } => sb.hover(css).await,
            ScenarioStep::HoverAndClick {
                hover_css,
                click_css,
            } => sb.hover_and_click(hover_css, click_css).await,
            ScenarioStep::SelectOptionByText { css, text } => {
                sb.select_option_by_text(css, text).await
            }
            ScenarioStep::SelectOptionByValue { css, value } => {
                sb.select_option_by_value(css, value).await
            }
            ScenarioStep::DragAndDrop {
                source_css,
                target_css,
            } => sb.drag_and_drop(source_css, target_css).await,
            ScenarioStep::SwitchToFrame { css } => sb.switch_to_frame(css).await,
            ScenarioStep::SwitchToDefaultContent => sb.switch_to_default_content().await,
            ScenarioStep::Clear { css } => sb.clear(css).await,
            ScenarioStep::ClickLinkText { text } => sb.click_link_text(text).await,
            ScenarioStep::Submit { css } => sb.submit(css).await,
            ScenarioStep::AcceptAlert => sb.accept_alert().await,
            ScenarioStep::DismissAlert => sb.dismiss_alert().await,
            ScenarioStep::TypeAlertText { text } => sb.type_alert_text(text).await,
            ScenarioStep::ClearLocalStorage => sb.clear_local_storage().await,
            ScenarioStep::RemoveLocalStorageItem { key } => sb.remove_local_storage_item(key).await,
            ScenarioStep::SetLocalStorageItem { key, value } => {
                sb.set_local_storage_item(key, value).await
            }
            ScenarioStep::SwitchToWindow { handle } => sb.switch_to_window(handle).await,

            ScenarioStep::JsClick { css } => sb.js_click(css).await,
            ScenarioStep::JsType { css, text } => sb.js_type(css, text).await,
            ScenarioStep::SetAttribute {
                css,
                attribute,
                value,
            } => sb.set_attribute(css, attribute, value).await,
            ScenarioStep::RemoveAttribute { css, attribute } => {
                sb.remove_attribute(css, attribute).await
            }
            ScenarioStep::ChooseFile { css, file_path } => sb.choose_file(css, file_path).await,
            ScenarioStep::GoBack => sb.go_back().await,
            ScenarioStep::GoForward => sb.go_forward().await,
            ScenarioStep::Refresh => sb.refresh().await,
            ScenarioStep::DeleteAllCookies => sb.delete_all_cookies().await,
            ScenarioStep::SwitchToNewWindow => sb.switch_to_new_window().await,

            ScenarioStep::DoubleClick { css } => sb.double_click(css).await,
            ScenarioStep::ContextClick { css } => sb.context_click(css).await,
            ScenarioStep::MaximizeWindow => sb.maximize_window().await,
            ScenarioStep::ScrollToBottom => sb.scroll_to_bottom().await,
            ScenarioStep::ScrollToTop => sb.scroll_to_top().await,
            ScenarioStep::ScrollTo { css } => sb.scroll_to(css).await,
        };

        match result {
            Ok(()) => passed_steps += 1,
            Err(e) => errors.push(e.to_string()),
        }
    }

    let failed_steps = scenario.steps.len().saturating_sub(passed_steps);
    Ok(RunSummary {
        scenario_name: scenario.name.clone(),
        total_steps: scenario.steps.len(),
        passed_steps,
        failed_steps,
        errors,
    })
}

pub fn write_dashboard_html<P: AsRef<std::path::Path>>(
    summary: &RunSummary,
    path: P,
) -> Result<(), SeleniumBaseError> {
    let error_list = summary
        .errors
        .iter()
        .map(|e| format!("<li>{}</li>", html_escape(e)))
        .collect::<String>();
    let html = format!(
        r#"<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<title>Dashboard: {name}</title>
<style>
body {{ font-family: Arial, sans-serif; margin: 40px; background: #f5f5f5; }}
.card {{ background: #fff; padding: 24px; border-radius: 8px; box-shadow: 0 2px 4px rgba(0,0,0,0.1); max-width: 720px; margin: 0 auto; }}
h1 {{ margin-top: 0; }}
.metric {{ font-size: 1.2em; margin: 8px 0; }}
.pass {{ color: green; }}
.fail {{ color: red; }}
ul {{ margin-top: 8px; }}
</style>
</head>
<body>
<div class="card">
<h1>Scenario Dashboard</h1>
<p class="metric"><strong>Name:</strong> {name}</p>
<p class="metric"><strong>Total steps:</strong> {total}</p>
<p class="metric pass"><strong>Passed:</strong> {passed}</p>
<p class="metric fail"><strong>Failed:</strong> {failed}</p>
<h2>Errors</h2>
<ul>{errors}</ul>
</div>
</body>
</html>"#,
        name = html_escape(&summary.scenario_name),
        total = summary.total_steps,
        passed = summary.passed_steps,
        failed = summary.failed_steps,
        errors = error_list
    );
    std::fs::write(path.as_ref(), html).map_err(|e| {
        SeleniumBaseError::InvalidConfig(format!(
            "failed to write dashboard '{}': {e}",
            path.as_ref().display()
        ))
    })
}

fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
