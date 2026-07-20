#![allow(deprecated)]

use clap::{Parser, Subcommand, ValueEnum};
use seleniumbase_rs::artifacts::{artifact_path, ensure_latest_logs_dir};
use seleniumbase_rs::cli::scripts::*;
// use seleniumbase_rs::dashboard::write_dashboard_html;
use seleniumbase_rs::api::scenario::{run_scenario, write_dashboard_html, Scenario};
use seleniumbase_rs::config::settings::Settings;
use seleniumbase_rs::{BaseCase, Browser, DriverMode};
use serde_json::{json, Value};
use thirtyfour::extensions::cdp::NetworkConditions;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum BrowserArg {
    Chrome,
    Chromium,
    Edge,
    Firefox,
}

impl From<BrowserArg> for Browser {
    fn from(value: BrowserArg) -> Self {
        match value {
            BrowserArg::Chrome => Browser::Chrome,
            BrowserArg::Chromium => Browser::Chromium,
            BrowserArg::Edge => Browser::Edge,
            BrowserArg::Firefox => Browser::Firefox,
        }
    }
}

#[derive(Debug, Parser)]
#[command(name = "sbase", version, about = "SeleniumBase Rust CLI")]
struct Cli {
    #[arg(long, default_value = "http://localhost:4444")]
    webdriver: String,
    #[arg(long, value_enum, default_value_t = BrowserArg::Chrome)]
    browser: BrowserArg,
    #[arg(long, default_value_t = false)]
    headed: bool,
    #[arg(long, default_value_t = false)]
    headless: bool,
    #[arg(long, default_value_t = false)]
    cdp: bool,
    #[arg(long, default_value_t = false)]
    uc: bool,
    #[arg(long)]
    user_agent: Option<String>,
    #[arg(short = 'a', long)]
    agent: Option<String>,
    #[arg(long)]
    locale: Option<String>,
    #[arg(long, default_value_t = false)]
    ad_block: bool,
    #[arg(long)]
    proxy: Option<String>,
    #[arg(long)]
    proxy_pac_url: Option<String>,
    #[arg(long)]
    user_data_dir: Option<String>,
    #[arg(long)]
    extension_dir: Option<String>,
    #[arg(long)]
    reuse_session: bool,
    #[arg(long)]
    rs: bool,
    #[arg(long, default_value_t = false)]
    mobile: bool,
    #[arg(short = 'n', long)]
    threads: Option<usize>,
    #[arg(short = 'c', long)]
    config: Option<String>,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Open {
        url: String,
    },
    Smoke {
        url: String,
        #[arg(long)]
        title_contains: Option<String>,
    },
    Cdp {
        #[arg(long)]
        cmd: String,
        #[arg(long)]
        params: Option<String>,
    },
    CacheClear,
    Throttle3g,
    Screenshot {
        #[arg(long)]
        path: Option<String>,
    },
    SaveSource {
        #[arg(long)]
        path: Option<String>,
    },
    AssertElement {
        #[arg(long)]
        css: String,
    },
    WaitForText {
        #[arg(long)]
        css: String,
        #[arg(long)]
        text: String,
        #[arg(long, default_value_t = 10)]
        timeout: u64,
    },
    Hover {
        #[arg(long)]
        css: String,
    },
    HoverAndClick {
        #[arg(long)]
        hover_css: String,
        #[arg(long)]
        click_css: String,
    },
    SelectOption {
        #[arg(long)]
        css: String,
        #[arg(long)]
        text: Option<String>,
        #[arg(long)]
        value: Option<String>,
    },
    DragAndDrop {
        #[arg(long)]
        source_css: String,
        #[arg(long)]
        target_css: String,
    },
    CdpClickElement {
        #[arg(long)]
        css: String,
    },
    CdpTypeText {
        #[arg(long)]
        text: String,
    },

    GoBack,
    GoForward,
    Refresh,

    GetText {
        #[arg(long)]
        css: String,
    },
    GetAttribute {
        #[arg(long)]
        css: String,
        #[arg(long)]
        attribute: String,
    },
    GetProperty {
        #[arg(long)]
        css: String,
        #[arg(long)]
        property: String,
    },

    GetTitle,
    GetCurrentUrl,

    ClearCookies,

    AcceptAlert,
    DismissAlert,
    GetAlertText,
    TypeAlertText {
        #[arg(long)]
        text: String,
    },
    ClearLocalStorage,
    GetLocalStorageItem {
        #[arg(long)]
        key: String,
    },
    SetLocalStorageItem {
        #[arg(long)]
        key: String,
        #[arg(long)]
        value: String,
    },
    RemoveLocalStorageItem {
        #[arg(long)]
        key: String,
    },
    SwitchToWindow {
        #[arg(long)]
        handle: String,
    },
    SwitchToFrame {
        #[arg(long)]
        css: String,
    },
    SwitchToDefaultContent,

    GetCookies,
    ExportRecording,
    PatchChromedriver {
        #[arg(long)]
        path: String,
    },
    AssertTextVisible {
        #[arg(long)]
        css: String,
        #[arg(long)]
        text: String,
    },
    AssertTextNotVisible {
        #[arg(long)]
        css: String,
        #[arg(long)]
        text: String,
    },
    AssertAttribute {
        #[arg(long)]
        css: String,
        #[arg(long)]
        attribute: String,
        #[arg(long)]
        value: String,
    },
    AssertTitle {
        #[arg(long)]
        text: String,
    },
    WaitForReadyStateComplete,
    GetWindowPosition,
    SetWindowPosition {
        #[arg(long)]
        x: u32,
        #[arg(long)]
        y: u32,
    },
    CloseWindow,
    SwitchToParentFrame,
    IsElementVisible {
        #[arg(long)]
        css: String,
    },
    IsTextVisible {
        #[arg(long)]
        css: String,
        #[arg(long)]
        text: String,
    },
    WaitForElementNotVisible {
        #[arg(long)]
        css: String,
        #[arg(long, default_value_t = 10)]
        timeout: u64,
    },
    SaveCookies {
        #[arg(long)]
        file: String,
    },
    LoadCookies {
        #[arg(long)]
        file: String,
    },
    HighlightClick {
        #[arg(long)]
        css: String,
    },
    IsChecked {
        #[arg(long)]
        css: String,
    },
    CheckIfUnchecked {
        #[arg(long)]
        css: String,
    },
    UncheckIfChecked {
        #[arg(long)]
        css: String,
    },
    OpenNewWindow,
    OpenNewTab,
    SwitchToNewestWindow,
    SwitchToDefaultWindow,
    GetActiveElementCss,
    WaitForElementPresent {
        #[arg(long)]
        css: String,
        #[arg(long, default_value_t = 10)]
        timeout: u64,
    },
    AddText {
        #[arg(long)]
        css: String,
        #[arg(long)]
        text: String,
    },
    SendKeys {
        #[arg(long)]
        css: String,
        #[arg(long)]
        text: String,
    },
    GetValue {
        #[arg(long)]
        css: String,
    },
    ClickVisibleElements {
        #[arg(long)]
        css: String,
    },
    WaitForAndAcceptAlert {
        #[arg(long, default_value_t = 10)]
        timeout: u64,
    },
    WaitForAndDismissAlert {
        #[arg(long, default_value_t = 10)]
        timeout: u64,
    },
    IsLinkTextVisible {
        #[arg(long)]
        text: String,
    },
    IsPartialLinkTextVisible {
        #[arg(long)]
        text: String,
    },
    AssertLinkText {
        #[arg(long)]
        text: String,
    },
    ClickPartialLinkText {
        #[arg(long)]
        text: String,
    },
    HumanType {
        #[arg(long)]
        css: String,
        #[arg(long)]
        text: String,
    },
    HumanClick {
        #[arg(long)]
        css: String,
    },
    SmoothScrollTo {
        #[arg(long)]
        css: String,
    },
    UcClick {
        #[arg(long)]
        css: String,
    },
    UcType {
        #[arg(long)]
        css: String,
        #[arg(long)]
        text: String,
    },
    Install,
    Mkdir {
        #[arg(long)]
        dir: String,
    },
    Mkfile {
        #[arg(long)]
        file: String,
    },
    Commander,
    Caseplans,
    BehaveGui,
    Print {
        #[arg(long)]
        file: String,
    },
    Objectify,
    Mkpres {
        #[arg(long)]
        file: String,
    },
    Mkchart {
        #[arg(long)]
        file: String,
    },
    Mkrec {
        #[arg(long)]
        file: String,
    },
    RunScenario {
        #[arg(long)]
        file: String,
        #[arg(long)]
        dashboard: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Cli::parse();
    if args.cdp && args.uc {
        return Err("Choose either --cdp or --uc, not both.".into());
    }

    // Start from global config file (if any) and apply CLI overrides.
    let mut settings = match args.config.as_deref() {
        Some(path) => Settings::load(Some(path))?,
        None => Settings::load_global()?,
    };
    settings.browser = match args.browser {
        BrowserArg::Chrome => "chrome".to_owned(),
        BrowserArg::Chromium => "chromium".to_owned(),
        BrowserArg::Edge => "edge".to_owned(),
        BrowserArg::Firefox => "firefox".to_owned(),
    };
    if args.headless {
        settings.headless = true;
    } else if args.headed {
        settings.headless = false;
    }
    if args.cdp {
        settings.mode = Some("cdp".to_owned());
    } else if args.uc {
        settings.mode = Some("uc".to_owned());
    }
    if let Some(v) = args.user_agent.as_ref().or(args.agent.as_ref()) {
        settings.user_agent = Some(v.clone());
    }
    if let Some(v) = args.locale {
        settings.locale = Some(v);
    }
    if args.ad_block {
        settings.ad_block = true;
    }
    if let Some(v) = args.proxy {
        settings.proxy = Some(v);
    }
    if let Some(v) = args.proxy_pac_url {
        settings.proxy_pac_url = Some(v);
    }
    if let Some(v) = args.user_data_dir {
        settings.user_data_dir = Some(v);
    }
    if let Some(v) = args.extension_dir {
        settings.extension_dir = Some(v);
    }
    if args.reuse_session || args.rs {
        settings.reuse_session = true;
    }
    if args.mobile {
        settings.mobile = true;
    }
    if let Some(v) = args.threads {
        settings.threads = Some(v);
    }

    let mode = settings
        .mode
        .as_deref()
        .map(|m| match m.to_lowercase().as_str() {
            "uc" => DriverMode::Uc,
            "cdp" => DriverMode::Cdp,
            _ => DriverMode::WebDriver,
        })
        .unwrap_or(DriverMode::WebDriver);
    let mut config = settings.to_browser_config();
    config.webdriver_url = args.webdriver;
    config.mode = mode;
    config.auto_start_driver = true;

    match args.command {
        Commands::Open { url } => {
            let mut sb = BaseCase::new(config).await?;
            sb.open(&url).await?;
            let title = sb.get_title().await?;
            println!("Title: {title}");
            sb.quit().await?;
        }
        Commands::Smoke {
            url,
            title_contains,
        } => {
            let mut sb = BaseCase::new(config).await?;
            sb.open(&url).await?;
            if let Some(expected) = title_contains.as_deref() {
                sb.assert_title_contains(expected).await?;
                println!("Assertion passed: title contains '{expected}'");
            } else {
                println!("Loaded: {}", sb.get_title().await?);
            }
            sb.quit().await?;
        }
        Commands::Cdp { cmd, params } => {
            let sb = BaseCase::new(config).await?;
            let result = if let Some(raw_params) = params.as_deref() {
                let parsed: Value = serde_json::from_str(raw_params)?;
                sb.execute_cdp_with_params(&cmd, parsed).await?
            } else {
                sb.execute_cdp(&cmd).await?
            };
            println!("{result}");
            sb.quit().await?;
        }
        Commands::CacheClear => {
            let sb = BaseCase::new(config).await?;
            sb.clear_browser_cache().await?;
            println!("CDP cache clear command sent.");
            sb.quit().await?;
        }
        Commands::Throttle3g => {
            let sb = BaseCase::new(config).await?;
            let mut conditions = NetworkConditions::new();
            conditions.offline = false;
            conditions.latency = 200;
            conditions.download_throughput = 256 * 1024;
            conditions.upload_throughput = 64 * 1024;
            sb.set_network_conditions(&conditions).await?;
            let result = sb
                .execute_cdp_with_params("Network.setCacheDisabled", json!({"cacheDisabled": true}))
                .await?;
            println!("3G throttle enabled. {result}");
            sb.quit().await?;
        }
        Commands::Screenshot { path } => {
            let sb = BaseCase::new(config).await?;
            if let Some(target_path) = path.as_deref() {
                sb.save_screenshot(target_path).await?;
                println!("Saved screenshot: {target_path}");
            } else {
                let out = sb.save_screenshot_to_logs().await?;
                println!("Saved screenshot: {}", out.display());
            }
            sb.quit().await?;
        }
        Commands::SaveSource { path } => {
            let sb = BaseCase::new(config).await?;
            if let Some(target_path) = path.as_deref() {
                sb.save_page_source(target_path).await?;
                println!("Saved page source: {target_path}");
            } else {
                let out = sb.save_page_source_to_logs().await?;
                println!("Saved page source: {}", out.display());
            }
            sb.quit().await?;
        }
        Commands::AssertElement { css } => {
            let sb = BaseCase::new(config).await?;
            sb.assert_element(&css).await?;
            println!("Assertion passed: element exists for selector '{css}'");
            sb.quit().await?;
        }
        Commands::WaitForText { css, text, timeout } => {
            let sb = BaseCase::new(config).await?;
            sb.wait_for_text(&css, &text, timeout).await?;
            println!("Text found for selector '{css}': {text}");
            sb.quit().await?;
        }
        Commands::Hover { css } => {
            let mut sb = BaseCase::new(config).await?;
            sb.hover(&css).await?;
            println!("Hovered over element '{css}'");
            sb.quit().await?;
        }
        Commands::HoverAndClick {
            hover_css,
            click_css,
        } => {
            let mut sb = BaseCase::new(config).await?;
            sb.hover_and_click(&hover_css, &click_css).await?;
            println!("Hovered '{hover_css}' and clicked '{click_css}'");
            sb.quit().await?;
        }
        Commands::SelectOption { css, text, value } => {
            let mut sb = BaseCase::new(config).await?;
            if let Some(t) = text {
                sb.select_option_by_text(&css, &t).await?;
                println!("Selected option by text '{t}' on '{css}'");
            } else if let Some(v) = value {
                sb.select_option_by_value(&css, &v).await?;
                println!("Selected option by value '{v}' on '{css}'");
            } else {
                println!("Must provide either --text or --value for SelectOption");
            }
            sb.quit().await?;
        }
        Commands::DragAndDrop {
            source_css,
            target_css,
        } => {
            let mut sb = BaseCase::new(config).await?;
            sb.drag_and_drop(&source_css, &target_css).await?;
            println!("Dragged '{source_css}' and dropped on '{target_css}'");
            sb.quit().await?;
        }
        Commands::CdpClickElement { css } => {
            let mut sb = BaseCase::new(config).await?;
            sb.cdp_click_element(&css).await?;
            println!("CDP clicked element '{css}'");
            sb.quit().await?;
        }
        Commands::CdpTypeText { text } => {
            let sb = BaseCase::new(config).await?;
            sb.cdp_type_text(&text).await?;
            println!("CDP typed text '{text}'");
            sb.quit().await?;
        }

        Commands::GoBack => {
            let sb = BaseCase::new(config).await?;
            sb.go_back().await?;
            println!("Went back");
            sb.quit().await?;
        }
        Commands::GoForward => {
            let sb = BaseCase::new(config).await?;
            sb.go_forward().await?;
            println!("Went forward");
            sb.quit().await?;
        }
        Commands::Refresh => {
            let sb = BaseCase::new(config).await?;
            sb.refresh().await?;
            println!("Refreshed page");
            sb.quit().await?;
        }

        Commands::GetText { css } => {
            let mut sb = BaseCase::new(config).await?;
            let text = sb.get_text(&css).await?;
            println!("Text for '{}': {}", css, text);
            sb.quit().await?;
        }
        Commands::GetAttribute { css, attribute } => {
            let mut sb = BaseCase::new(config).await?;
            let val = sb.get_attribute(&css, &attribute).await?;
            if let Some(v) = val {
                println!("Attribute '{}' for '{}': {}", attribute, css, v);
            } else {
                println!("Attribute '{}' not found for '{}'", attribute, css);
            }
            sb.quit().await?;
        }
        Commands::GetProperty { css, property } => {
            let mut sb = BaseCase::new(config).await?;
            let val = sb.get_property(&css, &property).await?;
            if let Some(v) = val {
                println!("Property '{}' for '{}': {}", property, css, v);
            } else {
                println!("Property '{}' not found for '{}'", property, css);
            }
            sb.quit().await?;
        }

        Commands::GetTitle => {
            let mut sb = BaseCase::new(config).await?;
            let title = sb.get_title().await?;
            println!("Title: {}", title);
            sb.quit().await?;
        }
        Commands::GetCurrentUrl => {
            let mut sb = BaseCase::new(config).await?;
            let url = sb.get_current_url().await?;
            println!("Current URL: {}", url);
            sb.quit().await?;
        }

        Commands::ClearCookies => {
            let sb = BaseCase::new(config).await?;
            sb.clear_browser_cookies().await?;
            println!("Cookies cleared");
            sb.quit().await?;
        }

        Commands::AcceptAlert => {
            let sb = BaseCase::new(config).await?;
            sb.accept_alert().await?;
            println!("Accepted alert");
            sb.quit().await?;
        }
        Commands::DismissAlert => {
            let sb = BaseCase::new(config).await?;
            sb.dismiss_alert().await?;
            println!("Dismissed alert");
            sb.quit().await?;
        }
        Commands::GetAlertText => {
            let sb = BaseCase::new(config).await?;
            let text = sb.get_alert_text().await?;
            println!("Alert text: {}", text);
            sb.quit().await?;
        }
        Commands::TypeAlertText { text } => {
            let sb = BaseCase::new(config).await?;
            sb.type_alert_text(&text).await?;
            println!("Typed alert text: {}", text);
            sb.quit().await?;
        }
        Commands::ClearLocalStorage => {
            let sb = BaseCase::new(config).await?;
            sb.clear_local_storage().await?;
            println!("Cleared local storage");
            sb.quit().await?;
        }
        Commands::GetLocalStorageItem { key } => {
            let sb = BaseCase::new(config).await?;
            let val = sb.get_local_storage_item(&key).await?;
            println!("Local storage item '{}': {}", key, val);
            sb.quit().await?;
        }
        Commands::SetLocalStorageItem { key, value } => {
            let sb = BaseCase::new(config).await?;
            sb.set_local_storage_item(&key, &value).await?;
            println!("Set local storage item '{}' to '{}'", key, value);
            sb.quit().await?;
        }
        Commands::RemoveLocalStorageItem { key } => {
            let sb = BaseCase::new(config).await?;
            sb.remove_local_storage_item(&key).await?;
            println!("Removed local storage item '{}'", key);
            sb.quit().await?;
        }
        Commands::SwitchToWindow { handle } => {
            let sb = BaseCase::new(config).await?;
            sb.switch_to_window(&handle).await?;
            println!("Switched to window '{}'", handle);
            sb.quit().await?;
        }
        Commands::SwitchToFrame { css } => {
            let mut sb = BaseCase::new(config).await?;
            sb.switch_to_frame(&css).await?;
            println!("Switched to frame '{}'", css);
            sb.quit().await?;
        }
        Commands::SwitchToDefaultContent => {
            let mut sb = BaseCase::new(config).await?;
            sb.switch_to_default_content().await?;
            println!("Switched to default content");
            sb.quit().await?;
        }

        Commands::GetCookies => {
            let sb = BaseCase::new(config).await?;
            let cookies = sb.get_cookies().await?;
            println!("Cookies: {:?}", cookies);
            sb.quit().await?;
        }
        Commands::ExportRecording => {
            let sb = BaseCase::new(config).await?;
            let (json_file, rust_file) = sb.save_recording_to_logs()?;
            println!("Saved recording json: {}", json_file.display());
            println!("Saved recording script: {}", rust_file.display());
            sb.quit().await?;
        }
        Commands::PatchChromedriver { path } => {
            seleniumbase_rs::stealth::patcher::patch_chromedriver(&path)?;
            println!("Successfully patched chromedriver binary at: {path}");
        }
        Commands::AssertTextVisible { css, text } => {
            let mut sb = BaseCase::new(config).await?;
            sb.assert_text_visible(&text, &css).await?;
            println!("Text '{}' is visible in '{}'", text, css);
            sb.quit().await?;
        }
        Commands::AssertTextNotVisible { css, text } => {
            let mut sb = BaseCase::new(config).await?;
            sb.assert_text_not_visible(&text, &css).await?;
            println!("Text '{}' is not visible in '{}'", text, css);
            sb.quit().await?;
        }
        Commands::AssertAttribute {
            css,
            attribute,
            value,
        } => {
            let mut sb = BaseCase::new(config).await?;
            sb.assert_attribute(&css, &attribute, &value).await?;
            println!("Attribute '{}' of '{}' is '{}'", attribute, css, value);
            sb.quit().await?;
        }
        Commands::AssertTitle { text } => {
            let mut sb = BaseCase::new(config).await?;
            sb.assert_title(&text).await?;
            println!("Title is '{}'", text);
            sb.quit().await?;
        }
        Commands::WaitForReadyStateComplete => {
            let sb = BaseCase::new(config).await?;
            sb.wait_for_ready_state_complete().await?;
            println!("Ready state complete");
            sb.quit().await?;
        }
        Commands::GetWindowPosition => {
            let sb = BaseCase::new(config).await?;
            let (x, y) = sb.get_window_position().await?;
            println!("Window position: x={}, y={}", x, y);
            sb.quit().await?;
        }
        Commands::SetWindowPosition { x, y } => {
            let sb = BaseCase::new(config).await?;
            sb.set_window_position(x, y).await?;
            println!("Set window position to x={}, y={}", x, y);
            sb.quit().await?;
        }
        Commands::CloseWindow => {
            let mut sb = BaseCase::new(config).await?;
            sb.close_window().await?;
            println!("Closed window");
            sb.quit().await?;
        }
        Commands::SwitchToParentFrame => {
            let mut sb = BaseCase::new(config).await?;
            sb.switch_to_parent_frame().await?;
            println!("Switched to parent frame");
            sb.quit().await?;
        }
        Commands::IsElementVisible { css } => {
            let sb = BaseCase::new(config).await?;
            let visible = sb.is_element_visible(&css).await?;
            println!("Element '{}' is visible: {}", css, visible);
            sb.quit().await?;
        }
        Commands::IsTextVisible { css, text } => {
            let sb = BaseCase::new(config).await?;
            let visible = sb.is_text_visible(&text, &css).await?;
            println!("Text '{}' in '{}' is visible: {}", text, css, visible);
            sb.quit().await?;
        }
        Commands::WaitForElementNotVisible { css, timeout } => {
            let mut sb = BaseCase::new(config).await?;
            sb.wait_for_element_not_visible(&css, timeout).await?;
            println!("Element '{}' is not visible", css);
            sb.quit().await?;
        }
        Commands::SaveCookies { file } => {
            let sb = BaseCase::new(config).await?;
            sb.save_cookies(&file).await?;
            println!("Saved cookies to '{}'", file);
        }
        Commands::LoadCookies { file } => {
            let sb = BaseCase::new(config).await?;
            sb.load_cookies(&file).await?;
            println!("Loaded cookies from '{}'", file);
        }
        Commands::HighlightClick { css } => {
            let mut sb = BaseCase::new(config).await?;
            sb.highlight_click(&css).await?;
            println!("Highlighted and clicked '{}'", css);
        }
        Commands::IsChecked { css } => {
            let mut sb = BaseCase::new(config).await?;
            let checked = sb.is_checked(&css).await?;
            println!("Element '{}' is checked: {}", css, checked);
        }
        Commands::CheckIfUnchecked { css } => {
            let mut sb = BaseCase::new(config).await?;
            sb.check_if_unchecked(&css).await?;
            println!("Checked '{}'", css);
        }
        Commands::UncheckIfChecked { css } => {
            let mut sb = BaseCase::new(config).await?;
            sb.uncheck_if_checked(&css).await?;
            println!("Unchecked '{}'", css);
        }
        Commands::OpenNewWindow => {
            let mut sb = BaseCase::new(config).await?;
            sb.open_new_window().await?;
            println!("Opened new window");
        }
        Commands::OpenNewTab => {
            let mut sb = BaseCase::new(config).await?;
            sb.open_new_tab().await?;
            println!("Opened new tab");
        }
        Commands::SwitchToNewestWindow => {
            let mut sb = BaseCase::new(config).await?;
            sb.switch_to_newest_window().await?;
            println!("Switched to newest window");
        }
        Commands::SwitchToDefaultWindow => {
            let mut sb = BaseCase::new(config).await?;
            sb.switch_to_default_window().await?;
            println!("Switched to default window");
        }
        Commands::GetActiveElementCss => {
            let sb = BaseCase::new(config).await?;
            let css = sb.get_active_element_css().await?;
            println!("Active element CSS: {}", css);
            sb.quit().await?;
        }
        Commands::WaitForElementPresent { css, timeout } => {
            let mut sb = BaseCase::new(config).await?;
            sb.wait_for_element_present(&css, timeout).await?;
            println!("Element '{}' is present", css);
        }
        Commands::AddText { css, text } => {
            let mut sb = BaseCase::new(config).await?;
            sb.add_text(&css, &text).await?;
            println!("Added text to '{}'", css);
        }
        Commands::SendKeys { css, text } => {
            let mut sb = BaseCase::new(config).await?;
            sb.send_keys(&css, &text).await?;
            println!("Sent keys to '{}'", css);
        }
        Commands::GetValue { css } => {
            let mut sb = BaseCase::new(config).await?;
            let value = sb.get_value(&css).await?;
            println!("Value of '{}': {}", css, value);
        }
        Commands::ClickVisibleElements { css } => {
            let mut sb = BaseCase::new(config).await?;
            sb.click_visible_elements(&css).await?;
            println!("Clicked visible elements matching '{}'", css);
        }
        Commands::WaitForAndAcceptAlert { timeout } => {
            let sb = BaseCase::new(config).await?;
            sb.wait_for_and_accept_alert(timeout).await?;
            println!("Waited for and accepted alert");
        }
        Commands::WaitForAndDismissAlert { timeout } => {
            let sb = BaseCase::new(config).await?;
            sb.wait_for_and_dismiss_alert(timeout).await?;
            println!("Waited for and dismissed alert");
        }
        Commands::IsLinkTextVisible { text } => {
            let sb = BaseCase::new(config).await?;
            let visible = sb.is_link_text_visible(&text).await?;
            println!("Link text '{}' is visible: {}", text, visible);
        }
        Commands::IsPartialLinkTextVisible { text } => {
            let sb = BaseCase::new(config).await?;
            let visible = sb.is_partial_link_text_visible(&text).await?;
            println!("Partial link text '{}' is visible: {}", text, visible);
        }
        Commands::AssertLinkText { text } => {
            let sb = BaseCase::new(config).await?;
            sb.assert_link_text(&text).await?;
            println!("Link text '{}' asserted", text);
        }
        Commands::ClickPartialLinkText { text } => {
            let mut sb = BaseCase::new(config).await?;
            sb.click_partial_link_text(&text).await?;
            println!("Clicked partial link text '{}'", text);
        }
        Commands::HumanType { css, text } => {
            let mut sb = BaseCase::new(config).await?;
            sb.human_type(&css, &text).await?;
            println!("Human typed into '{}'", css);
        }
        Commands::HumanClick { css } => {
            let mut sb = BaseCase::new(config).await?;
            sb.human_click(&css).await?;
            println!("Human clicked '{}'", css);
        }
        Commands::SmoothScrollTo { css } => {
            let mut sb = BaseCase::new(config).await?;
            sb.smooth_scroll_to(&css).await?;
            println!("Smooth scrolled to '{}'", css);
        }
        Commands::UcClick { css } => {
            let mut sb = BaseCase::new(config).await?;
            sb.uc_click(&css).await?;
            println!("UC clicked '{}'", css);
        }
        Commands::UcType { css, text } => {
            let mut sb = BaseCase::new(config).await?;
            sb.uc_type(&css, &text).await?;
            println!("UC typed into '{}'", css);
        }
        Commands::Install => match sb_install::install_drivers().await {
            Ok(path) => println!("Drivers installed successfully at {}", path.display()),
            Err(e) => eprintln!("Failed to install driver: {}", e),
        },
        Commands::Mkdir { dir } => {
            sb_mkdir::create_test_dir(&dir);
        }
        Commands::Mkfile { file } => {
            sb_mkfile::create_test_file(&file);
        }
        Commands::Commander => {
            if let Err(e) = sb_commander::run_commander() {
                eprintln!("Failed to run commander: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Caseplans => match sb_caseplans::run_caseplans() {
            Ok(path) => println!("Generated {}", path.display()),
            Err(e) => eprintln!("Failed to generate case plans: {}", e),
        },
        Commands::BehaveGui => match sb_behave_gui::run_gui() {
            Ok(path) => println!("Created BDD feature file at {}", path.display()),
            Err(e) => eprintln!("Failed to create BDD feature file: {}", e),
        },
        Commands::Print { file } => match sb_print::print_file(&file) {
            Ok(content) => println!("{}", content),
            Err(e) => eprintln!("Failed to read file {}: {}", file, e),
        },
        Commands::Objectify => match sb_objectify::objectify_page() {
            Ok(path) => println!("Generated {}", path.display()),
            Err(e) => eprintln!("Failed to generate page object: {}", e),
        },
        Commands::Mkpres { file } => match sb_mkpres::make_presentation(&file) {
            Ok(path) => println!("Created presentation at {}", path.display()),
            Err(e) => eprintln!("Failed to create presentation: {}", e),
        },
        Commands::Mkchart { file } => match sb_mkchart::make_chart(&file) {
            Ok(path) => println!("Created chart at {}", path.display()),
            Err(e) => eprintln!("Failed to create chart: {}", e),
        },
        Commands::Mkrec { file } => match sb_recorder::make_recorder_file(&file) {
            Ok(path) => println!("Created recorder file at {}", path.display()),
            Err(e) => eprintln!("Failed to create recorder file: {}", e),
        },
        Commands::RunScenario { file, dashboard } => {
            let scenario_json = std::fs::read_to_string(&file)?;
            let scenario: Scenario = serde_json::from_str(&scenario_json)?;
            let mut sb = BaseCase::new(config).await?;
            let summary = run_scenario(&mut sb, &scenario).await?;

            if let Some(dashboard_path) = dashboard.as_deref() {
                write_dashboard_html(&summary, dashboard_path)?;
                println!("Dashboard written: {dashboard_path}");
            } else {
                let logs_dir = ensure_latest_logs_dir()?;
                let dashboard_path = artifact_path(&logs_dir, "dashboard", "html");
                write_dashboard_html(&summary, &dashboard_path)?;
                println!("Dashboard written: {}", dashboard_path.display());
            }

            println!(
                "Scenario '{}' steps: total={}, passed={}, failed={}",
                summary.scenario_name,
                summary.total_steps,
                summary.passed_steps,
                summary.failed_steps
            );
            if !summary.errors.is_empty() {
                println!("Errors:");
                for error in &summary.errors {
                    println!("- {error}");
                }
            }

            let (json_file, rust_file) = sb.save_recording_to_logs()?;
            println!("Recording json: {}", json_file.display());
            println!("Recording script: {}", rust_file.display());
            sb.quit().await?;
        }
    }

    Ok(())
}
