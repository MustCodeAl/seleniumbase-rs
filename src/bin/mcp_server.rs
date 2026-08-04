//! SeleniumBase MCP server.
//!
//! Exposes a subset of the `BaseCase` browser-automation API through the Model
//! Context Protocol. Build and run with the `mcp-server` feature enabled:
//!
//! ```bash
//! cargo run --bin seleniumbase-mcp --features mcp-server
//! ```
//!
//! Configure your MCP client to launch the built binary over stdio.

use std::future::Future;

use rmcp::handler::server::ServerHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, ErrorData,
    ListToolsResult, ServerInfo, Tool,
};
use rmcp::serve_server;
use rmcp::service::{RequestContext, RoleServer};
use rmcp::transport::io::stdio;
use seleniumbase_rs::{
    init_tracing_from_runtime, BaseCase, BrowserConfig, ChromedriverPatcher, EnginePatch,
    Fingerprint, RuntimeConfig, SeleniumBaseError,
};
use serde_json::{json, Value};

/// Convert a crate error into a rich MCP `ErrorData` message, including a
/// remediation hint when one is available.
fn sb_error_to_mcp(tool: &str, err: SeleniumBaseError) -> ErrorData {
    err.log_in_context(format!("mcp/{tool}"));
    let mut message = format!("tool '{tool}' failed: {err}");
    if let Some(hint) = err.hint() {
        message.push_str(&format!("\n\nHint: {hint}"));
    }
    ErrorData::internal_error(message, None)
}
use tokio::sync::Mutex;

/// Shared server state. The browser session is created lazily on the first
/// tool call so that listing tools does not require a running WebDriver.
struct SeleniumBaseMcp {
    case: Mutex<Option<BaseCase>>,
    config: BrowserConfig,
}

impl SeleniumBaseMcp {
    fn new() -> Self {
        Self {
            case: Mutex::new(None),
            config: BrowserConfig::default(),
        }
    }

    /// Ensure a browser session exists and return a mutable guard to it.
    async fn case(&self) -> Result<tokio::sync::MutexGuard<'_, Option<BaseCase>>, ErrorData> {
        let mut guard = self.case.lock().await;
        if guard.is_none() {
            let case = BaseCase::new(self.config.clone())
                .await
                .map_err(|e| sb_error_to_mcp("start_session", e))?;
            *guard = Some(case);
        }
        Ok(guard)
    }
}

fn make_tool(name: &str, description: &str, schema: Value) -> Tool {
    let schema = schema.as_object().cloned().unwrap_or_default();
    Tool::new(name.to_string(), description.to_string(), schema)
}

fn preset_fingerprint(preset: &str) -> Option<Fingerprint> {
    match preset {
        "windows" => Some(Fingerprint::windows_desktop()),
        "macos" => Some(Fingerprint::macos_desktop()),
        "android" => Some(Fingerprint::android_mobile()),
        _ => None,
    }
}

fn tools() -> Vec<Tool> {
    vec![
        make_tool(
            "open_url",
            "Open a URL in the browser",
            json!({
                "type": "object",
                "properties": { "url": { "type": "string" } },
                "required": ["url"]
            }),
        ),
        make_tool(
            "get_title",
            "Return the current page title",
            json!({"type": "object"}),
        ),
        make_tool(
            "get_url",
            "Return the current page URL",
            json!({"type": "object"}),
        ),
        make_tool(
            "click",
            "Click the element matching the CSS selector",
            json!({
                "type": "object",
                "properties": { "selector": { "type": "string" } },
                "required": ["selector"]
            }),
        ),
        make_tool(
            "type_text",
            "Type text into the element matching the CSS selector",
            json!({
                "type": "object",
                "properties": {
                    "selector": { "type": "string" },
                    "text": { "type": "string" }
                },
                "required": ["selector", "text"]
            }),
        ),
        make_tool(
            "get_text",
            "Return the visible text of the element matching the CSS selector",
            json!({
                "type": "object",
                "properties": { "selector": { "type": "string" } },
                "required": ["selector"]
            }),
        ),
        make_tool(
            "assert_text",
            "Assert that the element matching the CSS selector contains the expected text",
            json!({
                "type": "object",
                "properties": {
                    "selector": { "type": "string" },
                    "expected": { "type": "string" }
                },
                "required": ["selector", "expected"]
            }),
        ),
        make_tool(
            "execute_script",
            "Execute JavaScript in the current page",
            json!({
                "type": "object",
                "properties": { "script": { "type": "string" } },
                "required": ["script"]
            }),
        ),
        make_tool(
            "quit",
            "Close the browser session",
            json!({"type": "object"}),
        ),
        make_tool(
            "screenshot",
            "Save a screenshot of the current page",
            json!({
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"]
            }),
        ),
        make_tool(
            "patch_chromedriver",
            "Patch a chromedriver binary to remove automation markers",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "backup": { "type": "boolean" }
                },
                "required": ["path"]
            }),
        ),
        make_tool(
            "list_engine_spoofing_args",
            "Return Chromium flags that reduce engine-level automation fingerprints",
            json!({"type": "object"}),
        ),
        make_tool(
            "list_fingerprint_presets",
            "Return the names of built-in fingerprint presets",
            json!({"type": "object"}),
        ),
        make_tool(
            "build_fingerprint",
            "Build a Fingerprint profile from a named preset",
            json!({
                "type": "object",
                "properties": {
                    "preset": { "type": "string", "enum": ["windows", "macos", "android"] },
                    "user_agent": { "type": "string" },
                    "screen_width": { "type": "integer" },
                    "screen_height": { "type": "integer" }
                },
                "required": ["preset"]
            }),
        ),
        make_tool(
            "get_stealth_bootstrap_script",
            "Return the JavaScript evasion bootstrap for a fingerprint preset",
            json!({
                "type": "object",
                "properties": {
                    "preset": { "type": "string", "enum": ["windows", "macos", "android"] }
                },
                "required": ["preset"]
            }),
        ),
        make_tool(
            "list_evasion_providers",
            "List the built-in stealth evasion providers in application order",
            json!({"type": "object"}),
        ),
        make_tool(
            "build_stealth_bootstrap",
            "Assemble the combined stealth bootstrap script for a fingerprint preset \
             using the evasion provider registry",
            json!({
                "type": "object",
                "properties": {
                    "preset": { "type": "string", "enum": ["windows", "macos", "android"] }
                },
                "required": ["preset"]
            }),
        ),
        make_tool(
            "validate_fingerprint",
            "Run profile coherence validation on a fingerprint preset and report \
             errors and warnings",
            json!({
                "type": "object",
                "properties": {
                    "preset": { "type": "string", "enum": ["windows", "macos", "android"] }
                },
                "required": ["preset"]
            }),
        ),
        make_tool(
            "list_macros",
            "Return the names of convenience macros exported by seleniumbase_rs",
            json!({"type": "object"}),
        ),
    ]
}

#[allow(clippy::manual_async_fn)]
impl ServerHandler for SeleniumBaseMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(Default::default())
            .with_instructions("Browser automation MCP server powered by SeleniumBase for Rust.")
    }

    fn list_tools(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, ErrorData>> + '_ {
        async move {
            Ok(ListToolsResult {
                tools: tools(),
                ..Default::default()
            })
        }
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<CallToolResponse, ErrorData>> + '_ {
        async move {
            let args = request.arguments.unwrap_or_default();
            let text = |s: &str| {
                CallToolResponse::Complete(CallToolResult::success(vec![ContentBlock::text(s)]))
            };
            let error = |s: &str| {
                CallToolResponse::Complete(CallToolResult::error(vec![ContentBlock::text(s)]))
            };

            let result = match request.name.as_ref() {
                "open_url" => {
                    let url = args
                        .get("url")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| ErrorData::invalid_params("missing 'url' argument", None))?;
                    let mut guard = self.case().await?;
                    let case = guard.as_mut().ok_or_else(|| {
                        ErrorData::internal_error("browser session not available", None)
                    })?;
                    case.open(url)
                        .await
                        .map_err(|e| sb_error_to_mcp("open_url", e))?;
                    text(&format!("Opened {}", url))
                }
                "get_title" => {
                    let mut guard = self.case().await?;
                    let case = guard.as_mut().ok_or_else(|| {
                        ErrorData::internal_error("browser session not available", None)
                    })?;
                    let title = case
                        .get_title()
                        .await
                        .map_err(|e| sb_error_to_mcp("get_title", e))?;
                    text(&title)
                }
                "get_url" => {
                    let mut guard = self.case().await?;
                    let case = guard.as_mut().ok_or_else(|| {
                        ErrorData::internal_error("browser session not available", None)
                    })?;
                    let url = case
                        .get_url()
                        .await
                        .map_err(|e| sb_error_to_mcp("get_url", e))?;
                    text(&url)
                }
                "click" => {
                    let selector =
                        args.get("selector")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| {
                                ErrorData::invalid_params("missing 'selector' argument", None)
                            })?;
                    let mut guard = self.case().await?;
                    let case = guard.as_mut().ok_or_else(|| {
                        ErrorData::internal_error("browser session not available", None)
                    })?;
                    case.click(selector)
                        .await
                        .map_err(|e| sb_error_to_mcp("click", e))?;
                    text(&format!("Clicked {}", selector))
                }
                "type_text" => {
                    let selector =
                        args.get("selector")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| {
                                ErrorData::invalid_params("missing 'selector' argument", None)
                            })?;
                    let value = args.get("text").and_then(|v| v.as_str()).ok_or_else(|| {
                        ErrorData::invalid_params("missing 'text' argument", None)
                    })?;
                    let mut guard = self.case().await?;
                    let case = guard.as_mut().ok_or_else(|| {
                        ErrorData::internal_error("browser session not available", None)
                    })?;
                    case.type_text(selector, value)
                        .await
                        .map_err(|e| sb_error_to_mcp("type_text", e))?;
                    text(&format!("Typed '{}' into {}", value, selector))
                }
                "get_text" => {
                    let selector =
                        args.get("selector")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| {
                                ErrorData::invalid_params("missing 'selector' argument", None)
                            })?;
                    let mut guard = self.case().await?;
                    let case = guard.as_mut().ok_or_else(|| {
                        ErrorData::internal_error("browser session not available", None)
                    })?;
                    let t = case
                        .get_text(selector)
                        .await
                        .map_err(|e| sb_error_to_mcp("get_text", e))?;
                    text(&t)
                }
                "assert_text" => {
                    let selector =
                        args.get("selector")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| {
                                ErrorData::invalid_params("missing 'selector' argument", None)
                            })?;
                    let expected =
                        args.get("expected")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| {
                                ErrorData::invalid_params("missing 'expected' argument", None)
                            })?;
                    let mut guard = self.case().await?;
                    let case = guard.as_mut().ok_or_else(|| {
                        ErrorData::internal_error("browser session not available", None)
                    })?;
                    match case.assert_text(selector, expected).await {
                        Ok(()) => text(&format!("'{}' contains '{}'", selector, expected)),
                        Err(e) => error(&format!("{e}\n\nHint: {}", e.hint().unwrap_or_default())),
                    }
                }
                "execute_script" => {
                    let script = args.get("script").and_then(|v| v.as_str()).ok_or_else(|| {
                        ErrorData::invalid_params("missing 'script' argument", None)
                    })?;
                    let mut guard = self.case().await?;
                    let case = guard.as_mut().ok_or_else(|| {
                        ErrorData::internal_error("browser session not available", None)
                    })?;
                    let value = case
                        .execute_script(script)
                        .await
                        .map_err(|e| sb_error_to_mcp("execute_script", e))?;
                    let value = serde_json::to_string(&value)
                        .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
                    text(&value)
                }
                "quit" => {
                    let mut guard = self.case.lock().await;
                    if let Some(mut case) = guard.take() {
                        case.quit().await.map_err(|e| sb_error_to_mcp("quit", e))?;
                    }
                    text("Browser session closed")
                }
                "screenshot" => {
                    let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| {
                        ErrorData::invalid_params("missing 'path' argument", None)
                    })?;
                    let mut guard = self.case().await?;
                    let case = guard.as_mut().ok_or_else(|| {
                        ErrorData::internal_error("browser session not available", None)
                    })?;
                    case.save_screenshot(path)
                        .await
                        .map_err(|e| sb_error_to_mcp("screenshot", e))?;
                    text(&format!("Screenshot saved to {}", path))
                }
                "patch_chromedriver" => {
                    let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| {
                        ErrorData::invalid_params("missing 'path' argument", None)
                    })?;
                    let backup = args.get("backup").and_then(|v| v.as_bool()).unwrap_or(true);
                    let mut spec = EnginePatch::all();
                    spec.backup = backup;
                    ChromedriverPatcher::new(path)
                        .patch(spec)
                        .map_err(|e| sb_error_to_mcp("patch_chromedriver", e))?;
                    text(&format!("Patched chromedriver at {}", path))
                }
                "list_engine_spoofing_args" => {
                    let args = seleniumbase_rs::engine_spoofing_args();
                    text(
                        &serde_json::to_string_pretty(&args)
                            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?,
                    )
                }
                "list_fingerprint_presets" => text("windows, macos, android"),
                "build_fingerprint" => {
                    let preset = args.get("preset").and_then(|v| v.as_str()).ok_or_else(|| {
                        ErrorData::invalid_params("missing 'preset' argument", None)
                    })?;
                    let mut fp = preset_fingerprint(preset).ok_or_else(|| {
                        ErrorData::invalid_params("preset must be windows, macos, or android", None)
                    })?;
                    if let Some(ua) = args.get("user_agent").and_then(|v| v.as_str()) {
                        fp.user_agent = Some(ua.to_owned());
                    }
                    if let Some(w) = args.get("screen_width").and_then(|v| v.as_u64()) {
                        fp.screen_width = Some(w as u32);
                    }
                    if let Some(h) = args.get("screen_height").and_then(|v| v.as_u64()) {
                        fp.screen_height = Some(h as u32);
                    }
                    let value = serde_json::to_string_pretty(&fp)
                        .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
                    text(&value)
                }
                "get_stealth_bootstrap_script" => {
                    let preset = args.get("preset").and_then(|v| v.as_str()).ok_or_else(|| {
                        ErrorData::invalid_params("missing 'preset' argument", None)
                    })?;
                    let fp = preset_fingerprint(preset).ok_or_else(|| {
                        ErrorData::invalid_params("preset must be windows, macos, or android", None)
                    })?;
                    let script = seleniumbase_rs::stealth::evasions::bootstrap_script(&fp);
                    text(&script)
                }
                "list_evasion_providers" => {
                    let names = seleniumbase_rs::default_registry().provider_names();
                    text(
                        &serde_json::to_string_pretty(&names)
                            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?,
                    )
                }
                "build_stealth_bootstrap" => {
                    let preset = args.get("preset").and_then(|v| v.as_str()).ok_or_else(|| {
                        ErrorData::invalid_params("missing 'preset' argument", None)
                    })?;
                    let fp = preset_fingerprint(preset).ok_or_else(|| {
                        ErrorData::invalid_params("preset must be windows, macos, or android", None)
                    })?;
                    let ctx = seleniumbase_rs::EvasionContext::new(&fp);
                    let script = seleniumbase_rs::default_registry().bootstrap(&ctx);
                    text(&script)
                }
                "validate_fingerprint" => {
                    let preset = args.get("preset").and_then(|v| v.as_str()).ok_or_else(|| {
                        ErrorData::invalid_params("missing 'preset' argument", None)
                    })?;
                    let fp = preset_fingerprint(preset).ok_or_else(|| {
                        ErrorData::invalid_params("preset must be windows, macos, or android", None)
                    })?;
                    let report = fp.validate();
                    let value = json!({
                        "coherent": report.is_coherent(),
                        "errors": report.errors,
                        "warnings": report.warnings,
                    });
                    text(
                        &serde_json::to_string_pretty(&value)
                            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?,
                    )
                }
                "list_macros" => text(
                    "selector!, sb_test!, sb_open!, sb_click!, sb_type!, sb_hover!, \
                         sb_scroll_to!, sb_wait_for!, sb_select!, sb_assert_text!, \
                         sb_assert_title!, sb_assert_url!, sb_screenshot!, sb_js!, sb_quit!, \
                         assert_visible!, fingerprint!, uc_config!",
                ),
                _ => {
                    return Err(ErrorData::invalid_params(
                        format!("unknown tool: {}", request.name),
                        None,
                    ))
                }
            };

            Ok(result)
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = RuntimeConfig::from_env().unwrap_or_default();
    init_tracing_from_runtime(&runtime);

    let service = SeleniumBaseMcp::new();
    let transport = stdio();
    let running = serve_server(service, transport).await?;
    running.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_catalog_includes_open_url() {
        assert!(tools().iter().any(|tool| tool.name == "open_url"));
    }

    #[test]
    fn tool_catalog_includes_stealth_tools() {
        let catalog = tools();
        let names: Vec<&str> = catalog.iter().map(|t| t.name.as_ref()).collect();
        for expected in [
            "list_evasion_providers",
            "build_stealth_bootstrap",
            "validate_fingerprint",
        ] {
            assert!(names.contains(&expected), "missing tool {expected}");
        }
    }
}
