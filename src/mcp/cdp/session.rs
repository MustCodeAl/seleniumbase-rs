//! Starting and closing the browser, and what is known about the session.

use serde_json::json;

use super::Cdp;
use crate::error::SeleniumBaseError;
use crate::mcp::fingerprint;
use crate::mcp::support::nonempty;
use crate::mcp::Started;
use crate::mcp::{Args, Ctx, Effect, Output, Prop, Schema, ToolDef, ToolError};
use crate::sb_cdp::{Browser, LaunchOptions};

pub(super) fn tools() -> Vec<ToolDef<Cdp>> {
    vec![
        ToolDef::new(
            "start_browser",
            "Start Browser",
            "Launch the Chrome session the other tools use. Does nothing if one is already \
             running. Headless by default on Linux, headed elsewhere.",
            Effect::Set,
            Schema::new()
                .optional("url", Prop::string("Page to open as the browser starts"))
                .optional(
                    "headless",
                    Prop::boolean("Force headless (true) or headed (false)"),
                )
                .optional(
                    "use_chromium",
                    Prop::boolean("Use Chromium instead of Google Chrome").default(false),
                )
                .optional(
                    "incognito",
                    Prop::boolean("Incognito window").default(false),
                )
                .optional(
                    "guest",
                    Prop::boolean("Guest profile; cannot be combined with incognito")
                        .default(false),
                )
                .optional(
                    "ad_block",
                    Prop::boolean("Block common ad requests").default(false),
                )
                .optional(
                    "proxy",
                    Prop::string("SERVER:PORT or USER:PASS@SERVER:PORT"),
                )
                .optional("fingerprint", crate::mcp::fingerprint::launch_inline_prop())
                .optional(
                    "fingerprint_path",
                    crate::mcp::fingerprint::launch_path_prop(),
                )
                .optional(
                    "fingerprint_preset",
                    crate::mcp::fingerprint::launch_preset_prop(),
                ),
            start_browser,
        ),
        ToolDef::new(
            "close_browser",
            "Close Browser",
            "Close the browser session and everything in it: tabs, cookies and history. Safe \
             to call when nothing is running.",
            Effect::Set,
            Schema::new(),
            close_browser,
        ),
        ToolDef::new(
            "get_page_info",
            "Get Page Info",
            "Report whether a browser is running and, if so, the active tab's URL, title, \
             origin and user agent.",
            Effect::Observe,
            Schema::new(),
            get_page_info,
        ),
        ToolDef::new(
            "get_devtools_endpoint",
            "Get DevTools Endpoint",
            "Report the DevTools endpoint of the running browser (HTTP address and browser \
             WebSocket URL) and ready-made MCP client configs that attach Google's \
             chrome-devtools-mcp (--browserUrl) or Microsoft's Playwright MCP \
             (--cdp-endpoint) to the same browser. Reports running=false when no browser runs.",
            Effect::Observe,
            Schema::new(),
            get_devtools_endpoint,
        ),
    ]
}

async fn start_browser(ctx: Ctx<Cdp>, args: Args) -> Result<Output, ToolError> {
    let headless = args
        .opt_bool("headless")?
        .unwrap_or(cfg!(target_os = "linux"));
    let use_chromium = args.bool_or("use_chromium", false)?;
    let url = nonempty(&args, "url")?.map(str::to_owned);

    let mut options = LaunchOptions::builder()
        .headless(headless)
        .use_chromium(use_chromium)
        .incognito(args.bool_or("incognito", false)?)
        .guest(args.bool_or("guest", false)?)
        .ad_block(args.bool_or("ad_block", false)?);
    // Applied first: the settings made below replace the fingerprint's own.
    let fingerprint = fingerprint::start_fingerprint(&args, ctx.settings())?;
    if let Some(fingerprint) = &fingerprint {
        options = options.fingerprint(fingerprint);
    }
    if let Some(url) = &url {
        options = options.url(url.as_str());
    }
    // The executable is the operator's choice (`SB_CHROME_BIN`), never a tool
    // argument: a client that could name a path could run any program.
    if args.value("browser_executable_path").is_some() {
        return Err(ToolError::invalid(
            "browser_executable_path",
            "not a tool argument: the operator sets the browser with SB_CHROME_BIN for the server",
        ));
    }
    if let Some(path) = ctx.settings().browser_binary() {
        if use_chromium {
            return Err(ToolError::invalid(
                    "use_chromium",
                    "the operator set the browser executable (SB_CHROME_BIN), so use_chromium cannot change it",
                ));
        }
        options = options.executable_path(path);
    }
    if let Some(proxy) = nonempty(&args, "proxy")? {
        options = options.proxy(proxy);
    }
    // The operator's plugins (`sbrs-mcp --plugin`), never a tool argument.
    let plugins = ctx.settings().plugin_selection();
    plugins.ensure_supported()?;
    #[cfg(feature = "plugins")]
    let options = options.plugin_selection(plugins);
    let options = options.build()?;

    let started = ctx
        .start(async move {
            let browser = Browser::launch(options).await?;
            Ok::<_, ToolError>(Cdp::attach(browser).await?)
        })
        .await?;
    Ok(match started {
        Started::Created => format!(
            "Started Pure CDP Mode browser (url={}, headless={headless}, use_chromium={use_chromium}).{}",
            url.as_deref().unwrap_or("none"),
            fingerprint::applied(fingerprint.as_ref()),
        ),
        Started::AlreadyRunning => format!(
            "A browser session is already running.{}",
            fingerprint::ignored(fingerprint.as_ref())
        ),
    }
    .into())
}

async fn close_browser(ctx: Ctx<Cdp>, _args: Args) -> Result<Output, ToolError> {
    let Some(cdp) = ctx.take().await else {
        return Ok("No browser session is currently running.".into());
    };
    cdp.browser.close().await?;
    Ok("The browser session was closed.".into())
}

async fn get_devtools_endpoint(ctx: Ctx<Cdp>, _args: Args) -> Result<Output, ToolError> {
    let Some(session) = ctx.try_session().await else {
        return Ok(json!({
            "running": false,
            "hint": "Call start_browser first; the endpoint exists only while a browser runs.",
        })
        .into());
    };
    let mut answer = session.browser.devtools_endpoint().to_json();
    answer["running"] = json!(true);
    Ok(answer.into())
}

async fn get_page_info(ctx: Ctx<Cdp>, _args: Args) -> Result<Output, ToolError> {
    let Some(session) = ctx.try_session().await else {
        return Ok(json!({ "running": false }).into());
    };
    let page = &session.page;
    let info = async {
        Ok::<_, SeleniumBaseError>(json!({
            "running": true,
            "url": page.url().await?,
            "title": page.title().await?,
            "origin": page.evaluate_as::<String>("location.origin").await?,
            "user_agent": page.user_agent().await?,
        }))
    }
    .await;
    Ok(info
        .unwrap_or_else(|error| json!({ "running": false, "error": error.to_string() }))
        .into())
}
