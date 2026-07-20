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
    CallToolRequestParams, CallToolResult, ContentBlock, ErrorData, ListToolsResult, ServerInfo,
    Tool,
};
use rmcp::serve_server;
use rmcp::service::{RequestContext, RoleServer};
use rmcp::transport::io::stdio;
use seleniumbase_rs::{BaseCase, BrowserConfig};
use serde_json::{json, Value};
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
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
            *guard = Some(case);
        }
        Ok(guard)
    }
}

fn make_tool(name: &str, description: &str, schema: Value) -> Tool {
    let schema = schema.as_object().cloned().unwrap_or_default();
    Tool::new(name.to_string(), description.to_string(), schema)
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
    ) -> impl Future<Output = Result<CallToolResult, ErrorData>> + '_ {
        async move {
            let args = request.arguments.unwrap_or_default();
            let text = |s: &str| CallToolResult::success(vec![ContentBlock::text(s)]);
            let error = |s: &str| CallToolResult::error(vec![ContentBlock::text(s)]);

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
                        .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
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
                        .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
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
                        .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
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
                        .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
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
                        .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
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
                        .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
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
                        Err(e) => error(&e.to_string()),
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
                        .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
                    let value = serde_json::to_string(&value)
                        .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
                    text(&value)
                }
                "quit" => {
                    let mut guard = self.case.lock().await;
                    if let Some(case) = guard.take() {
                        case.quit()
                            .await
                            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
                    }
                    text("Browser session closed")
                }
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
}
