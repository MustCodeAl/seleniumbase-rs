//! Tool handlers shared by the two WebDriver servers.
//!
//! The `driver` server mirrors upstream's `Driver()` tools and the `sb` server
//! its `SB()` tools. Both drive a [`BaseCase`], so most tools are the same
//! handler registered under both.
//!
//! `BaseCase` methods act at once, while the tools promise to wait up to a
//! timeout for the element, so every handler that takes a `timeout` waits for
//! the element to be visible first and then acts.

use std::time::Duration;

use serde_json::{json, Value};

use super::support::{cookie_file, json_output, nonempty, write_file};
use super::{Args, Closeable, Ctx, Output, Started, ToolError};
use crate::api::base_case::BaseCase;
use crate::browser::config::{Browser, BrowserConfig, DriverMode};
use crate::error::SeleniumBaseError;

/// The largest download the server will write to disk, in bytes.
const MAX_DOWNLOAD_BYTES: u64 = 256 * 1024 * 1024;

impl Closeable for BaseCase {
    async fn close(mut self) {
        // The server is shutting down; there is nobody left to report to.
        let _ = self.quit().await;
    }
}

/// `BaseCase` waits in whole seconds; round up so half a second still waits.
fn whole_seconds(timeout: Duration) -> u64 {
    timeout.as_secs() + u64::from(timeout.subsec_nanos() > 0)
}

/// Waits up to `timeout` for `selector` to be visible.
async fn ready(case: &BaseCase, selector: &str, timeout: Duration) -> Result<(), ToolError> {
    case.wait_for_element(selector, whole_seconds(timeout))
        .await?;
    Ok(())
}

/// Runs the common shape of a tool: take `selector` and `timeout`, wait for
/// the element, act on it, and report with the returned message.
async fn on_element<F>(ctx: Ctx<BaseCase>, args: Args, act: F) -> Result<Output, ToolError>
where
    F: AsyncFnOnce(&mut BaseCase, &str, &Args) -> Result<String, SeleniumBaseError>,
{
    let selector = args.str("selector")?;
    let timeout = args.seconds_or("timeout", 5.0)?;
    let mut case = ctx.session().await?;
    ready(&case, selector, timeout).await?;
    Ok(act(&mut case, selector, &args).await?.into())
}

/// Runs a tool that needs the session but no element.
async fn on_case<F>(ctx: Ctx<BaseCase>, act: F) -> Result<Output, ToolError>
where
    F: AsyncFnOnce(&mut BaseCase) -> Result<String, SeleniumBaseError>,
{
    let mut case = ctx.session().await?;
    Ok(act(&mut case).await?.into())
}

// ----------------------------------------------------------------------
// Session
// ----------------------------------------------------------------------

pub(super) async fn start_browser(ctx: Ctx<BaseCase>, args: Args) -> Result<Output, ToolError> {
    let choice = args.choice(
        "browser",
        &["chrome", "edge", "firefox", "chromium"],
        "chrome",
    )?;
    let headless = args
        .opt_bool("headless")?
        .unwrap_or(cfg!(target_os = "linux"));
    let incognito = args.bool_or("incognito", false)?;
    let guest = args.bool_or("guest_mode", false)?;
    if incognito && guest {
        return Err(ToolError::invalid(
            "guest_mode",
            "cannot be combined with incognito",
        ));
    }

    let browser = match choice {
        "edge" => Browser::Edge,
        "firefox" => Browser::Firefox,
        "chromium" => Browser::Chromium,
        _ => Browser::Chrome,
    };
    let chromium_family = !matches!(browser, Browser::Firefox);
    // Undetected mode is a Chromium feature.
    let uc = args.bool_or("uc", true)? && matches!(browser, Browser::Chrome | Browser::Chromium);

    let mut config = BrowserConfig::from_env()?
        .with_browser(browser)
        .with_headless(headless);
    if uc {
        config = config.with_mode(DriverMode::Uc);
    }
    config.ad_block = args.bool_or("ad_block", false)?;
    if chromium_family && incognito {
        config = config.push_extra_arg("--incognito");
    }
    if chromium_family && guest {
        config = config.push_extra_arg("--guest");
    }
    if let Some(proxy) = nonempty(&args, "proxy")? {
        config = config.with_proxy(proxy);
    }

    let started = ctx
        .start(async move { Ok::<_, ToolError>(Box::pin(BaseCase::new(config)).await?) })
        .await?;
    Ok(match started {
        Started::Created => {
            format!("Started WebDriver session (browser={choice}, headless={headless}, uc={uc}).")
        }
        Started::AlreadyRunning => {
            "A browser session is already running. Call close_browser first.".to_owned()
        }
    }
    .into())
}

pub(super) async fn close_browser(ctx: Ctx<BaseCase>, _args: Args) -> Result<Output, ToolError> {
    let Some(mut case) = ctx.take().await else {
        return Ok("No browser session was running.".into());
    };
    case.quit().await?;
    Ok("Browser closed.".into())
}

pub(super) async fn get_page_info(ctx: Ctx<BaseCase>, _args: Args) -> Result<Output, ToolError> {
    let Some(mut case) = ctx.try_session().await else {
        return Ok(json!({ "running": false }).into());
    };
    let info = async {
        Ok::<_, SeleniumBaseError>(json!({
            "running": true,
            "url": case.get_current_url().await?,
            "title": case.get_title().await?,
            "origin": case.get_origin().await?,
            "user_agent": case.get_user_agent().await?,
        }))
    }
    .await;
    Ok(info
        .unwrap_or_else(|error| json!({ "running": false, "error": error.to_string() }))
        .into())
}

// ----------------------------------------------------------------------
// Navigation and page information
// ----------------------------------------------------------------------

pub(super) async fn open_url(ctx: Ctx<BaseCase>, args: Args) -> Result<Output, ToolError> {
    let url = args.str("url")?;
    ctx.session().await?.open(url).await?;
    Ok(format!("Navigated to {url}").into())
}

pub(super) async fn refresh_page(ctx: Ctx<BaseCase>, _args: Args) -> Result<Output, ToolError> {
    on_case(ctx, async |case| {
        case.refresh().await?;
        Ok("Page refreshed.".to_owned())
    })
    .await
}

pub(super) async fn go_back(ctx: Ctx<BaseCase>, _args: Args) -> Result<Output, ToolError> {
    on_case(ctx, async |case| {
        case.go_back().await?;
        Ok("Navigated back.".to_owned())
    })
    .await
}

pub(super) async fn go_forward(ctx: Ctx<BaseCase>, _args: Args) -> Result<Output, ToolError> {
    on_case(ctx, async |case| {
        case.go_forward().await?;
        Ok("Navigated forward.".to_owned())
    })
    .await
}

pub(super) async fn get_current_url(ctx: Ctx<BaseCase>, _args: Args) -> Result<Output, ToolError> {
    on_case(ctx, async |case| case.get_current_url().await).await
}

pub(super) async fn get_title(ctx: Ctx<BaseCase>, _args: Args) -> Result<Output, ToolError> {
    on_case(ctx, async |case| case.get_title().await).await
}

pub(super) async fn get_origin(ctx: Ctx<BaseCase>, _args: Args) -> Result<Output, ToolError> {
    on_case(ctx, async |case| case.get_origin().await).await
}

pub(super) async fn get_user_agent(ctx: Ctx<BaseCase>, _args: Args) -> Result<Output, ToolError> {
    on_case(ctx, async |case| case.get_user_agent().await).await
}

pub(super) async fn get_page_source(ctx: Ctx<BaseCase>, _args: Args) -> Result<Output, ToolError> {
    on_case(ctx, async |case| case.get_page_source().await).await
}

// ----------------------------------------------------------------------
// Reading elements
// ----------------------------------------------------------------------

pub(super) async fn get_text(ctx: Ctx<BaseCase>, args: Args) -> Result<Output, ToolError> {
    let selector = args.str_or("selector", "body")?;
    let mut case = ctx.session().await?;
    Ok(case.get_text(selector).await?.into())
}

pub(super) async fn get_element_html(ctx: Ctx<BaseCase>, args: Args) -> Result<Output, ToolError> {
    let selector = args.str("selector")?;
    let mut case = ctx.session().await?;
    let element = case.find_element(selector).await?;
    Ok(element
        .outer_html()
        .await
        .map_err(SeleniumBaseError::WebDriver)?
        .into())
}

pub(super) async fn get_attribute(ctx: Ctx<BaseCase>, args: Args) -> Result<Output, ToolError> {
    let selector = args.str("selector")?;
    let attribute = args.str("attribute")?;
    let mut case = ctx.session().await?;
    Ok(Output::Json(
        case.get_attribute(selector, attribute)
            .await?
            .map_or(Value::Null, Value::String),
    ))
}

pub(super) async fn find_elements_count(
    ctx: Ctx<BaseCase>,
    args: Args,
) -> Result<Output, ToolError> {
    let selector = args.str("selector")?;
    let mut case = ctx.session().await?;
    Ok(Output::Json(json!(case
        .find_elements(selector)
        .await?
        .len())))
}

pub(super) async fn is_element_present(
    ctx: Ctx<BaseCase>,
    args: Args,
) -> Result<Output, ToolError> {
    let selector = args.str("selector")?;
    let case = ctx.session().await?;
    Ok(Output::Json(Value::Bool(
        case.is_element_present(selector).await?,
    )))
}

pub(super) async fn is_element_visible(
    ctx: Ctx<BaseCase>,
    args: Args,
) -> Result<Output, ToolError> {
    let selector = args.str("selector")?;
    let case = ctx.session().await?;
    Ok(Output::Json(Value::Bool(
        case.is_element_visible(selector).await?,
    )))
}

pub(super) async fn is_element_clickable(
    ctx: Ctx<BaseCase>,
    args: Args,
) -> Result<Output, ToolError> {
    let selector = args.str("selector")?;
    let case = ctx.session().await?;
    Ok(Output::Json(Value::Bool(
        case.is_element_clickable(selector).await?,
    )))
}

pub(super) async fn is_text_visible(ctx: Ctx<BaseCase>, args: Args) -> Result<Output, ToolError> {
    let text = args.str("text")?;
    let selector = args.str_or("selector", "html")?;
    let case = ctx.session().await?;
    Ok(Output::Json(Value::Bool(
        case.is_text_visible(text, selector).await?,
    )))
}

pub(super) async fn is_selected(ctx: Ctx<BaseCase>, args: Args) -> Result<Output, ToolError> {
    let selector = args.str("selector")?;
    let mut case = ctx.session().await?;
    Ok(Output::Json(Value::Bool(case.is_selected(selector).await?)))
}

// ----------------------------------------------------------------------
// Interacting
// ----------------------------------------------------------------------

pub(super) async fn click_element(ctx: Ctx<BaseCase>, args: Args) -> Result<Output, ToolError> {
    on_element(ctx, args, async |case, selector, _| {
        case.click(selector).await?;
        Ok(format!("Clicked {selector}"))
    })
    .await
}

pub(super) async fn click_if_visible(ctx: Ctx<BaseCase>, args: Args) -> Result<Output, ToolError> {
    let selector = args.str("selector")?;
    ctx.session().await?.click_if_visible(selector).await?;
    Ok(format!("click_if_visible ran for {selector}").into())
}

pub(super) async fn click_visible_elements(
    ctx: Ctx<BaseCase>,
    args: Args,
) -> Result<Output, ToolError> {
    let selector = args.str("selector")?;
    let limit = args.opt_usize("limit")?.unwrap_or(0);
    let mut case = ctx.session().await?;
    if limit == 0 {
        case.click_visible_elements(selector).await?;
    } else {
        let mut clicked = 0;
        for element in case.find_elements(selector).await? {
            if clicked == limit {
                break;
            }
            if element
                .is_displayed()
                .await
                .map_err(SeleniumBaseError::WebDriver)?
            {
                element
                    .click()
                    .await
                    .map_err(SeleniumBaseError::WebDriver)?;
                clicked += 1;
            }
        }
    }
    Ok(format!("Clicked visible elements matching {selector}").into())
}

pub(super) async fn click_nth_visible_element(
    ctx: Ctx<BaseCase>,
    args: Args,
) -> Result<Output, ToolError> {
    let selector = args.str("selector")?;
    let number = args
        .opt_usize("number")?
        .and_then(|number| number.checked_sub(1))
        .ok_or_else(|| ToolError::invalid("number", "counts from 1"))?;
    ctx.session()
        .await?
        .click_nth_visible_element(selector, number)
        .await?;
    Ok(format!(
        "Clicked visible element #{} matching {selector}",
        number + 1
    )
    .into())
}

pub(super) async fn click_link(ctx: Ctx<BaseCase>, args: Args) -> Result<Output, ToolError> {
    let text = args.str("link_text")?;
    ctx.session().await?.click_link_text(text).await?;
    Ok(format!("Clicked link with text '{text}'").into())
}

pub(super) async fn double_click(ctx: Ctx<BaseCase>, args: Args) -> Result<Output, ToolError> {
    on_element(ctx, args, async |case, selector, _| {
        case.double_click(selector).await?;
        Ok(format!("Double-clicked {selector}"))
    })
    .await
}

pub(super) async fn context_click(ctx: Ctx<BaseCase>, args: Args) -> Result<Output, ToolError> {
    let selector = args.str("selector")?;
    ctx.session().await?.context_click(selector).await?;
    Ok(format!("Right-clicked {selector}").into())
}

pub(super) async fn type_text(ctx: Ctx<BaseCase>, args: Args) -> Result<Output, ToolError> {
    on_element(ctx, args, async |case, selector, args| {
        let text = args
            .str("text")
            .map_err(|e| SeleniumBaseError::invalid_config(e.to_string()))?;
        if args.bool_or("clear_first", true).unwrap_or(true) {
            case.type_text(selector, text).await?;
            Ok(format!("Typed into {selector} after clearing the field."))
        } else {
            case.send_keys(selector, text).await?;
            Ok(format!("Typed into {selector}"))
        }
    })
    .await
}

pub(super) async fn set_value(ctx: Ctx<BaseCase>, args: Args) -> Result<Output, ToolError> {
    args.str("text")?;
    on_element(ctx, args, async |case, selector, args| {
        let text = args.str("text").unwrap_or_default();
        case.set_value(selector, text).await?;
        Ok(format!("Set value of {selector}"))
    })
    .await
}

pub(super) async fn clear_input(ctx: Ctx<BaseCase>, args: Args) -> Result<Output, ToolError> {
    on_element(ctx, args, async |case, selector, _| {
        case.clear(selector).await?;
        Ok(format!("Cleared {selector}"))
    })
    .await
}

pub(super) async fn submit(ctx: Ctx<BaseCase>, args: Args) -> Result<Output, ToolError> {
    on_element(ctx, args, async |case, selector, _| {
        case.submit(selector).await?;
        Ok(format!("Submitted form via {selector}"))
    })
    .await
}

pub(super) async fn select_option_by_text(
    ctx: Ctx<BaseCase>,
    args: Args,
) -> Result<Output, ToolError> {
    let option = args.str("option")?.to_owned();
    let args = rename(args, "dropdown_selector", "selector");
    on_element(ctx, args, async |case, selector, _| {
        case.select_option_by_text(selector, &option).await?;
        Ok(format!("Selected text '{option}' in {selector}"))
    })
    .await
}

pub(super) async fn select_option_by_value(
    ctx: Ctx<BaseCase>,
    args: Args,
) -> Result<Output, ToolError> {
    let option = args.str("option")?.to_owned();
    let args = rename(args, "dropdown_selector", "selector");
    on_element(ctx, args, async |case, selector, _| {
        case.select_option_by_value(selector, &option).await?;
        Ok(format!("Selected value '{option}' in {selector}"))
    })
    .await
}

pub(super) async fn select_option_by_index(
    ctx: Ctx<BaseCase>,
    args: Args,
) -> Result<Output, ToolError> {
    let option = args.scalar("option")?;
    let index: usize = option
        .trim()
        .parse()
        .ok()
        .ok_or_else(|| ToolError::invalid("option", "an index must be a whole number"))?;
    let args = rename(args, "dropdown_selector", "selector");
    on_element(ctx, args, async |case, selector, _| {
        case.select_option_by_index(selector, index).await?;
        Ok(format!("Selected index {index} in {selector}"))
    })
    .await
}

/// Lets a handler that reads `selector` serve a tool whose argument has a
/// more specific name.
fn rename(mut args: Args, from: &str, to: &str) -> Args {
    args.rename(from, to);
    args
}

pub(super) async fn hover_and_click(ctx: Ctx<BaseCase>, args: Args) -> Result<Output, ToolError> {
    let hover = args.str("hover_selector")?;
    let click = args.str("click_selector")?;
    ctx.session().await?.hover_and_click(hover, click).await?;
    Ok(format!("Hovered {hover} then clicked {click}").into())
}

pub(super) async fn drag_and_drop(ctx: Ctx<BaseCase>, args: Args) -> Result<Output, ToolError> {
    let drag = args.str("drag_selector")?;
    let drop = args.str("drop_selector")?;
    ctx.session().await?.drag_and_drop(drag, drop).await?;
    Ok(format!("Dragged {drag} onto {drop}").into())
}

pub(super) async fn nested_click(ctx: Ctx<BaseCase>, args: Args) -> Result<Output, ToolError> {
    let parent = args.str("parent_selector")?;
    let selector = args.str("selector")?;
    ctx.session().await?.nested_click(parent, selector).await?;
    Ok(format!("Clicked {selector} inside {parent}").into())
}

pub(super) async fn choose_file(ctx: Ctx<BaseCase>, args: Args) -> Result<Output, ToolError> {
    let path = args.str("file_path")?.to_owned();
    if !std::path::Path::new(&path).is_file() {
        return Err(ToolError::invalid(
            "file_path",
            "no such file on this machine",
        ));
    }
    on_element(ctx, args, async |case, selector, _| {
        case.choose_file(selector, &path).await?;
        Ok(format!("Set file input {selector} to {path}"))
    })
    .await
}

// ----------------------------------------------------------------------
// Waiting and asserting
// ----------------------------------------------------------------------

/// `wait_for_element` waits for visibility; the two servers differ only in the
/// default timeout.
pub(super) async fn wait_for_element(
    ctx: Ctx<BaseCase>,
    args: Args,
    default_timeout: f64,
) -> Result<Output, ToolError> {
    let selector = args.str("selector")?;
    let timeout = args.seconds_or("timeout", default_timeout)?;
    ready(&*ctx.session().await?, selector, timeout).await?;
    Ok(format!("Element {selector} appeared.").into())
}

pub(super) async fn wait_for_element_present(
    ctx: Ctx<BaseCase>,
    args: Args,
) -> Result<Output, ToolError> {
    let selector = args.str("selector")?;
    let timeout = whole_seconds(args.seconds_or("timeout", 5.0)?);
    ctx.session()
        .await?
        .wait_for_element_present(selector, timeout)
        .await?;
    Ok(format!("Element {selector} is present.").into())
}

pub(super) async fn wait_for_element_not_visible(
    ctx: Ctx<BaseCase>,
    args: Args,
) -> Result<Output, ToolError> {
    let selector = args.str("selector")?;
    let timeout = whole_seconds(args.seconds_or("timeout", 5.0)?);
    ctx.session()
        .await?
        .wait_for_element_not_visible(selector, timeout)
        .await?;
    Ok(format!("Element {selector} is no longer visible.").into())
}

pub(super) async fn wait_for_element_absent(
    ctx: Ctx<BaseCase>,
    args: Args,
) -> Result<Output, ToolError> {
    let selector = args.str("selector")?;
    let timeout = whole_seconds(args.seconds_or("timeout", 5.0)?);
    ctx.session()
        .await?
        .wait_for_element_absent(selector, timeout)
        .await?;
    Ok(format!("Element {selector} is now absent.").into())
}

pub(super) async fn wait_for_text(ctx: Ctx<BaseCase>, args: Args) -> Result<Output, ToolError> {
    let text = args.str("text")?;
    let selector = args.str_or("selector", "html")?;
    let timeout = whole_seconds(args.seconds_or("timeout", 5.0)?);
    ctx.session()
        .await?
        .wait_for_text(selector, text, timeout)
        .await?;
    Ok(format!("Text '{text}' appeared in {selector}.").into())
}

pub(super) async fn assert_element(ctx: Ctx<BaseCase>, args: Args) -> Result<Output, ToolError> {
    let selector = args.str("selector")?;
    let timeout = args.seconds_or("timeout", 5.0)?;
    let case = ctx.session().await?;
    ready(&case, selector, timeout).await?;
    case.assert_element(selector).await?;
    Ok(format!("Confirmed {selector} is visible.").into())
}

pub(super) async fn assert_element_present(
    ctx: Ctx<BaseCase>,
    args: Args,
) -> Result<Output, ToolError> {
    let selector = args.str("selector")?;
    let timeout = whole_seconds(args.seconds_or("timeout", 5.0)?);
    let mut case = ctx.session().await?;
    case.wait_for_element_present(selector, timeout).await?;
    case.assert_element_present(selector).await?;
    Ok(format!("Confirmed {selector} is present.").into())
}

pub(super) async fn assert_element_not_visible(
    ctx: Ctx<BaseCase>,
    args: Args,
) -> Result<Output, ToolError> {
    let selector = args.str("selector")?;
    let timeout = whole_seconds(args.seconds_or("timeout", 5.0)?);
    let mut case = ctx.session().await?;
    case.wait_for_element_not_visible(selector, timeout).await?;
    case.assert_element_not_visible(selector).await?;
    Ok(format!("Confirmed {selector} is not visible.").into())
}

pub(super) async fn assert_text(ctx: Ctx<BaseCase>, args: Args) -> Result<Output, ToolError> {
    let text = args.str("text")?;
    let selector = nonempty(&args, "selector")?.unwrap_or("html");
    let timeout = whole_seconds(args.seconds_or("timeout", 5.0)?);
    let mut case = ctx.session().await?;
    case.wait_for_text(selector, text, timeout).await?;
    case.assert_text(selector, text).await?;
    Ok(format!("Confirmed '{text}' is present in {selector}.").into())
}

pub(super) async fn assert_exact_text(ctx: Ctx<BaseCase>, args: Args) -> Result<Output, ToolError> {
    let text = args.str("text")?;
    let selector = args.str_or("selector", "html")?;
    let timeout = whole_seconds(args.seconds_or("timeout", 5.0)?);
    let mut case = ctx.session().await?;
    case.wait_for_exact_text_visible(selector, text, timeout)
        .await?;
    case.assert_exact_text(selector, text).await?;
    Ok(format!("Confirmed {selector} text is exactly '{text}'.").into())
}

pub(super) async fn assert_title(ctx: Ctx<BaseCase>, args: Args) -> Result<Output, ToolError> {
    let title = args.str("title")?;
    ctx.session().await?.assert_title(title).await?;
    Ok(format!("Confirmed title is '{title}'.").into())
}

pub(super) async fn assert_url(ctx: Ctx<BaseCase>, args: Args) -> Result<Output, ToolError> {
    let url = args.str("url")?;
    ctx.session().await?.assert_url(url).await?;
    Ok(format!("Confirmed URL is '{url}'.").into())
}

pub(super) async fn assert_url_contains(
    ctx: Ctx<BaseCase>,
    args: Args,
) -> Result<Output, ToolError> {
    let substring = args.str("substring")?;
    ctx.session().await?.assert_url_contains(substring).await?;
    Ok(format!("Confirmed URL contains '{substring}'.").into())
}

pub(super) async fn assert_no_404_errors(
    ctx: Ctx<BaseCase>,
    _args: Args,
) -> Result<Output, ToolError> {
    ctx.session().await?.assert_no_404_errors().await?;
    Ok("Confirmed no broken (404) links.".into())
}

pub(super) async fn assert_no_js_errors(
    ctx: Ctx<BaseCase>,
    _args: Args,
) -> Result<Output, ToolError> {
    ctx.session().await?.assert_no_js_errors().await?;
    Ok("Confirmed no JS errors.".into())
}

// ----------------------------------------------------------------------
// Cookies and storage
// ----------------------------------------------------------------------

pub(super) async fn get_cookies(ctx: Ctx<BaseCase>, _args: Args) -> Result<Output, ToolError> {
    json_output(ctx.session().await?.get_cookies().await?)
}

pub(super) async fn delete_all_cookies(
    ctx: Ctx<BaseCase>,
    _args: Args,
) -> Result<Output, ToolError> {
    ctx.session().await?.delete_all_cookies().await?;
    Ok("All cookies deleted.".into())
}

pub(super) async fn save_cookies(ctx: Ctx<BaseCase>, args: Args) -> Result<Output, ToolError> {
    let (path, name) = cookie_file(ctx.settings(), args.str_or("name", "cookies.txt")?)?;
    if let Some(dir) = path.parent() {
        tokio::fs::create_dir_all(dir)
            .await
            .map_err(SeleniumBaseError::from)?;
    }
    ctx.session()
        .await?
        .save_cookies(&path.to_string_lossy())
        .await?;
    Ok(format!("Cookies saved to {name}").into())
}

pub(super) async fn load_cookies(ctx: Ctx<BaseCase>, args: Args) -> Result<Output, ToolError> {
    let (path, name) = cookie_file(ctx.settings(), args.str_or("name", "cookies.txt")?)?;
    if !path.is_file() {
        return Err(ToolError::Refused(format!(
            "{name} does not exist in the cookie directory"
        )));
    }
    ctx.session()
        .await?
        .load_cookies(&path.to_string_lossy())
        .await?;
    Ok(format!("Cookies loaded from {name}").into())
}

pub(super) async fn get_local_storage_item(
    ctx: Ctx<BaseCase>,
    args: Args,
) -> Result<Output, ToolError> {
    let key = args.str("key")?;
    Ok(Output::Json(
        ctx.session().await?.get_local_storage_item(key).await?,
    ))
}

pub(super) async fn set_local_storage_item(
    ctx: Ctx<BaseCase>,
    args: Args,
) -> Result<Output, ToolError> {
    let (key, value) = (args.str("key")?, args.str("value")?);
    ctx.session()
        .await?
        .set_local_storage_item(key, value)
        .await?;
    Ok(format!("Set localStorage[{key:?}]").into())
}

pub(super) async fn get_session_storage_item(
    ctx: Ctx<BaseCase>,
    args: Args,
) -> Result<Output, ToolError> {
    let key = args.str("key")?;
    Ok(Output::Json(
        ctx.session().await?.get_session_storage_item(key).await?,
    ))
}

pub(super) async fn set_session_storage_item(
    ctx: Ctx<BaseCase>,
    args: Args,
) -> Result<Output, ToolError> {
    let (key, value) = (args.str("key")?, args.str("value")?);
    ctx.session()
        .await?
        .set_session_storage_item(key, value)
        .await?;
    Ok(format!("Set sessionStorage[{key:?}]").into())
}

// ----------------------------------------------------------------------
// Scrolling, windows, tabs and frames
// ----------------------------------------------------------------------

pub(super) async fn scroll_into_view(ctx: Ctx<BaseCase>, args: Args) -> Result<Output, ToolError> {
    let selector = args.str("selector")?;
    ctx.session().await?.scroll_into_view(selector).await?;
    Ok(format!("Scrolled {selector} into view.").into())
}

pub(super) async fn scroll_to_top(ctx: Ctx<BaseCase>, _args: Args) -> Result<Output, ToolError> {
    ctx.session().await?.scroll_to_top().await?;
    Ok("Scrolled to top.".into())
}

pub(super) async fn scroll_to_bottom(ctx: Ctx<BaseCase>, _args: Args) -> Result<Output, ToolError> {
    ctx.session().await?.scroll_to_bottom().await?;
    Ok("Scrolled to bottom.".into())
}

/// Scrolls by `amount` percent of the window height, up when `up` is set.
async fn scroll_by_percent(ctx: Ctx<BaseCase>, args: Args, up: bool) -> Result<Output, ToolError> {
    let amount = args.i64_or("amount", 25)?;
    if amount < 0 {
        return Err(ToolError::invalid("amount", "cannot be negative"));
    }
    let case = ctx.session().await?;
    let height = case
        .execute_script("return window.innerHeight;")
        .await?
        .as_f64()
        .unwrap_or_default();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        reason = "a scroll distance in pixels is far inside both ranges"
    )]
    let pixels = (height * amount as f64 / 100.0).round() as i64;
    case.scroll_by_y(if up { -pixels } else { pixels }).await?;
    let way = if up { "up" } else { "down" };
    Ok(format!("Scrolled {way} {amount}.").into())
}

pub(super) async fn scroll_up(ctx: Ctx<BaseCase>, args: Args) -> Result<Output, ToolError> {
    scroll_by_percent(ctx, args, true).await
}

pub(super) async fn scroll_down(ctx: Ctx<BaseCase>, args: Args) -> Result<Output, ToolError> {
    scroll_by_percent(ctx, args, false).await
}

pub(super) async fn get_window_rect(ctx: Ctx<BaseCase>, _args: Args) -> Result<Output, ToolError> {
    let (x, y, width, height) = ctx.session().await?.get_window_rect().await?;
    Ok(json!({ "x": x, "y": y, "width": width, "height": height }).into())
}

pub(super) async fn maximize_window(ctx: Ctx<BaseCase>, _args: Args) -> Result<Output, ToolError> {
    ctx.session().await?.maximize_window().await?;
    Ok("Window maximized.".into())
}

pub(super) async fn minimize_window(ctx: Ctx<BaseCase>, _args: Args) -> Result<Output, ToolError> {
    ctx.session().await?.minimize_window().await?;
    Ok("Window minimized.".into())
}

pub(super) async fn open_new_tab(ctx: Ctx<BaseCase>, args: Args) -> Result<Output, ToolError> {
    let switch_to = args.bool_or("switch_to", true)?;
    let mut case = ctx.session().await?;
    let before = case.get_current_window_handle().await?;
    case.open_new_tab().await?;
    if !switch_to {
        case.switch_to_window(&before).await?;
    }
    Ok(format!("Opened new tab (switch_to={switch_to})").into())
}

pub(super) async fn switch_to_newest_tab(
    ctx: Ctx<BaseCase>,
    _args: Args,
) -> Result<Output, ToolError> {
    ctx.session().await?.switch_to_newest_window().await?;
    Ok("Switched to newest tab.".into())
}

pub(super) async fn switch_to_default_window(
    ctx: Ctx<BaseCase>,
    _args: Args,
) -> Result<Output, ToolError> {
    ctx.session().await?.switch_to_default_window().await?;
    Ok("Switched to default (first) tab.".into())
}

pub(super) async fn switch_to_frame(ctx: Ctx<BaseCase>, args: Args) -> Result<Output, ToolError> {
    let selector = args.str_or("selector", "iframe")?;
    ctx.session().await?.switch_to_frame(selector).await?;
    Ok(format!("Switched into frame {selector}").into())
}

pub(super) async fn switch_to_default_content(
    ctx: Ctx<BaseCase>,
    _args: Args,
) -> Result<Output, ToolError> {
    ctx.session().await?.switch_to_default_content().await?;
    Ok("Switched back to main page.".into())
}

// ----------------------------------------------------------------------
// Stealth helpers
// ----------------------------------------------------------------------

pub(super) async fn activate_cdp_mode(ctx: Ctx<BaseCase>, args: Args) -> Result<Output, ToolError> {
    let url = nonempty(&args, "url")?;
    let mut case = ctx.session().await?;
    case.activate_cdp_mode().await?;
    if let Some(url) = url {
        case.open(url).await?;
    }
    Ok(format!("CDP Mode activated (url={})", url.unwrap_or("none")).into())
}

pub(super) async fn solve_captcha(ctx: Ctx<BaseCase>, _args: Args) -> Result<Output, ToolError> {
    Ok(match ctx.session().await?.solve_captcha().await? {
        Some(kind) => format!("Attempted to solve a {kind} CAPTCHA."),
        None => "No supported CAPTCHA was found on the page.".to_owned(),
    }
    .into())
}

pub(super) async fn get_mfa_code(ctx: Ctx<BaseCase>, args: Args) -> Result<Output, ToolError> {
    let key = args.str("totp_key")?;
    Ok(ctx.session().await?.get_mfa_code(key)?.into())
}

pub(super) async fn enter_mfa_code(ctx: Ctx<BaseCase>, args: Args) -> Result<Output, ToolError> {
    let (selector, key) = (args.str("selector")?, args.str("totp_key")?);
    ctx.session().await?.enter_mfa_code(selector, key).await?;
    Ok(format!("Entered MFA code into {selector}").into())
}

// ----------------------------------------------------------------------
// Output and scripting
// ----------------------------------------------------------------------

/// Saves a screenshot as `name` in `folder` under the output directory.
async fn save_screenshot_as(
    ctx: &Ctx<BaseCase>,
    folder: Option<&str>,
    name: &str,
) -> Result<Output, ToolError> {
    let path = ctx.settings().output_path(folder, name)?;
    let bytes = ctx.session().await?.screenshot_as_png().await?;
    write_file(&path, &bytes).await?;
    Ok(format!("Screenshot saved as {name}").into())
}

pub(super) async fn save_screenshot(ctx: Ctx<BaseCase>, args: Args) -> Result<Output, ToolError> {
    let name = nonempty(&args, "name")?.unwrap_or("screenshot.png");
    save_screenshot_as(&ctx, nonempty(&args, "folder")?, name).await
}

/// The `driver` server's form, which names the file `filename`.
pub(super) async fn save_screenshot_file(
    ctx: Ctx<BaseCase>,
    args: Args,
) -> Result<Output, ToolError> {
    let name = nonempty(&args, "filename")?.unwrap_or("screenshot.png");
    save_screenshot_as(&ctx, None, name).await
}

pub(super) async fn save_page_source(ctx: Ctx<BaseCase>, args: Args) -> Result<Output, ToolError> {
    let name = nonempty(&args, "name")?.unwrap_or("page_source.html");
    let path = ctx
        .settings()
        .output_path(nonempty(&args, "folder")?, name)?;
    let source = ctx.session().await?.get_page_source().await?;
    write_file(&path, source.as_bytes()).await?;
    Ok(format!("Page source saved as {name}").into())
}

pub(super) async fn print_to_pdf(ctx: Ctx<BaseCase>, args: Args) -> Result<Output, ToolError> {
    let name = nonempty(&args, "name")?.unwrap_or("page.pdf");
    let path = ctx
        .settings()
        .output_path(nonempty(&args, "folder")?, name)?;
    if let Some(dir) = path.parent() {
        tokio::fs::create_dir_all(dir)
            .await
            .map_err(SeleniumBaseError::from)?;
    }
    ctx.session()
        .await?
        .print_to_pdf(&path.to_string_lossy())
        .await?;
    Ok(format!("Page saved as PDF: {name}").into())
}

pub(super) async fn download_file(ctx: Ctx<BaseCase>, args: Args) -> Result<Output, ToolError> {
    let url = args.str("file_url")?;
    let parsed = url::Url::parse(url)
        .ok()
        .ok_or_else(|| ToolError::invalid("file_url", "not a valid URL"))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(ToolError::invalid(
            "file_url",
            "only http and https are supported",
        ));
    }
    let name = crate::cli::commands::download_file_name(url);
    let folder = nonempty(&args, "destination_folder")?.unwrap_or("downloaded_files");
    let path = ctx.settings().output_path(Some(folder), &name)?;

    let mut response = reqwest::get(parsed.clone())
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|e| SeleniumBaseError::download(url, e.to_string()))?;
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| SeleniumBaseError::download(url, e.to_string()))?
    {
        if body.len() as u64 + chunk.len() as u64 > MAX_DOWNLOAD_BYTES {
            return Err(SeleniumBaseError::download(url, "larger than the download limit").into());
        }
        body.extend_from_slice(&chunk);
    }
    write_file(&path, &body).await?;
    Ok(format!("Downloaded {url}").into())
}

/// Evaluates an expression with the DevTools `Runtime.evaluate`, awaiting a
/// returned promise.
pub(super) async fn evaluate(ctx: Ctx<BaseCase>, args: Args) -> Result<Output, ToolError> {
    let expression = args.str("expression")?;
    let response = ctx
        .session()
        .await?
        .execute_cdp_with_params(
            "Runtime.evaluate",
            json!({ "expression": expression, "returnByValue": true, "awaitPromise": true }),
        )
        .await?;
    if let Some(details) = response.get("exceptionDetails") {
        let reason = details["exception"]["description"]
            .as_str()
            .or_else(|| details["text"].as_str())
            .unwrap_or("the expression threw");
        return Err(SeleniumBaseError::AssertionFailed(reason.to_owned()).into());
    }
    Ok(Output::Json(response["result"]["value"].clone()))
}

pub(super) async fn execute_script(ctx: Ctx<BaseCase>, args: Args) -> Result<Output, ToolError> {
    let script = args.str("script")?;
    Ok(Output::Json(
        ctx.session().await?.execute_script(script).await?,
    ))
}

pub(super) async fn highlight(ctx: Ctx<BaseCase>, args: Args) -> Result<Output, ToolError> {
    let loops = args.opt_usize("loops")?.unwrap_or(4);
    on_element(ctx, args, async |case, selector, _| {
        case.flash(selector, loops).await?;
        Ok(format!("Highlighted {selector}"))
    })
    .await
}

pub(super) async fn flash(ctx: Ctx<BaseCase>, args: Args) -> Result<Output, ToolError> {
    let selector = args.str("selector")?;
    let seconds = args.seconds_or("duration", 1.0)?.as_secs_f64();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the duration is bounded by MAX_SECONDS and rounded to a small count"
    )]
    let times = (seconds.round() as usize).max(1);
    ctx.session().await?.flash(selector, times).await?;
    Ok(format!("Flashed {selector}").into())
}

pub(super) async fn sleep(ctx: Ctx<BaseCase>, args: Args) -> Result<Output, ToolError> {
    let seconds = args.seconds_or("seconds", 0.0)?;
    ctx.session().await?.sleep(seconds.as_secs_f64()).await;
    Ok(format!("Slept {}s", seconds.as_secs_f64()).into())
}

// ----------------------------------------------------------------------
// Schema pieces the two tool tables share
// ----------------------------------------------------------------------

pub(super) fn selector_prop() -> super::Prop {
    super::Prop::string("CSS selector or XPath")
}

pub(super) fn timeout_prop(default: f64) -> super::Prop {
    super::Prop::number("Seconds to wait for the element")
        .min(0.0)
        .default(default)
}

/// The arguments of `start_browser`, which both servers accept.
pub(super) fn start_schema() -> super::Schema {
    use super::{Prop, Schema};
    Schema::new()
        .optional(
            "browser",
            Prop::choice(&["chrome", "edge", "firefox", "chromium"], "Which browser")
                .default("chrome"),
        )
        .optional(
            "headless",
            Prop::boolean(
                "Force headless (true) or headed (false); default is headless on Linux only",
            ),
        )
        .optional(
            "uc",
            Prop::boolean(
                "Undetected mode, for sites with bot detection (Chrome and Chromium only)",
            )
            .default(true),
        )
        .optional(
            "incognito",
            Prop::boolean("Incognito window").default(false),
        )
        .optional(
            "guest_mode",
            Prop::boolean("Guest profile; cannot be combined with incognito").default(false),
        )
        .optional(
            "ad_block",
            Prop::boolean("Block common ad requests").default(false),
        )
        .optional(
            "proxy",
            Prop::string("SERVER:PORT or USER:PASS@SERVER:PORT"),
        )
}
