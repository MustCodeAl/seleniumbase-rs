//! `mcp::client::McpClient` against the hosts of this crate, over an in-memory pipe: no process,
//! no network, no browser. The process and HTTP targets are tested in `crates/cli/tests`.

#![cfg(feature = "mcp-client")]

use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use seleniumbase_rs::mcp::cdp;
use seleniumbase_rs::mcp::client::{ClientProgress, McpClient};
use seleniumbase_rs::mcp::{Args, Ctx, Effect, Host, Output, Schema, Settings, ToolDef, ToolError};
use serde_json::{json, Map, Value};

/// How long a test waits for the server before it calls the server stuck.
const PATIENCE: Duration = Duration::from_secs(20);

fn tool<F, Fut>(name: &'static str, handler: F) -> ToolDef<()>
where
    F: Fn(Ctx<()>, Args) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<Output, ToolError>> + Send + 'static,
{
    ToolDef::new(
        name,
        "A test tool",
        "A tool for the client tests.",
        Effect::Inspect,
        Schema::new(),
        handler,
    )
}

fn host_with(tools: Vec<ToolDef<()>>) -> Host<()> {
    Host::new("host-test", "Instructions.", tools, Settings::new("out"))
}

/// Serves `host` on one end of a pipe and connects a client to the other end.
async fn connect_to<S: Send + 'static>(host: Host<S>, timeout: Duration) -> McpClient {
    let (server_end, client_end) = tokio::io::duplex(1 << 20);
    tokio::spawn(async move {
        if let Ok(running) = rmcp::serve_server(host, server_end).await {
            let _ = running.waiting().await;
        }
    });
    McpClient::connect_with("test host", client_end, timeout)
        .await
        .expect("the handshake works")
}

async fn connect<S: Send + 'static>(host: Host<S>) -> McpClient {
    connect_to(host, PATIENCE).await
}

fn object(value: &Value) -> Map<String, Value> {
    value.as_object().cloned().unwrap_or_default()
}

#[tokio::test]
async fn the_handshake_tells_what_the_server_offers() {
    let tools_only = connect(host_with(vec![])).await;
    let dir = tempfile::tempdir().unwrap();
    let browser = connect(cdp::host(Settings::new(dir.path()))).await;

    let plain = tools_only.server_info();
    let rich = browser.server_info();

    assert_eq!(plain.name, "host-test");
    assert!(plain.tools && !plain.resources && !plain.prompts, "{plain:?}");
    assert_eq!(plain.instructions.as_deref(), Some("Instructions."));
    assert!(!plain.protocol_version.is_empty());
    assert!(rich.tools && rich.resources && rich.prompts, "{rich:?}");
}

#[tokio::test]
async fn tools_are_listed_with_their_schemas() {
    let declared = tool("declared", |_, _| async {
        Ok(Output::Json(json!({ "title": "Home" })))
    })
    .with_output_schema(json!({ "type": "object", "required": ["title"] }));
    let client = connect(host_with(vec![
        declared,
        tool("plain", |_, _| async { Ok(Output::from("ok")) }),
    ]))
    .await;

    let tools = client.list_tools().await.expect("tools/list works");

    let names: Vec<_> = tools.iter().map(|tool| tool.name.as_str()).collect();
    assert_eq!(names, ["declared", "plain"]);
    assert_eq!(tools[0].input_schema["type"], "object");
    assert_eq!(
        tools[0].output_schema,
        Some(json!({ "type": "object", "required": ["title"] }))
    );
    assert_eq!(tools[1].output_schema, None);
    assert_eq!(tools[1].description.as_deref(), Some("A tool for the client tests."));
}

#[tokio::test]
async fn a_call_returns_the_text_and_the_structured_content() {
    let client = connect(host_with(vec![
        tool("object", |_, args: Args| async move {
            Ok(Output::Json(json!({ "got": args.str("x").unwrap_or_default() })))
        }),
        tool("text", |_, _| async { Ok(Output::from("done")) }),
    ]))
    .await;

    let object_result = client
        .call("object", object(&json!({ "x": "hello" })), None)
        .await
        .expect("the call works");
    let text_result = client
        .call("text", Map::new(), None)
        .await
        .expect("the call works");

    assert!(!object_result.is_error);
    assert_eq!(object_result.structured, Some(json!({ "got": "hello" })));
    let parsed: Value = serde_json::from_str(&object_result.text).expect("the text is JSON");
    assert_eq!(Some(&parsed), object_result.structured.as_ref());
    assert_eq!(text_result.text, "done");
    assert_eq!(text_result.structured, None);
    assert!(text_result.images.is_empty());
}

#[tokio::test]
async fn a_tool_that_fails_is_an_outcome_and_not_an_error() {
    let client = connect(host_with(vec![tool("refuses", |_, _| async {
        Err::<Output, _>(ToolError::Refused("not on this server".to_owned()))
    })]))
    .await;

    let outcome = client
        .call("refuses", Map::new(), None)
        .await
        .expect("a failing tool is not a protocol error");

    assert!(outcome.is_error);
    assert!(outcome.text.starts_with("refuses failed:"), "{}", outcome.text);
    let report = outcome.structured.expect("the failure report");
    assert_eq!(report["kind"], "refused");
}

#[tokio::test]
async fn progress_reaches_the_callback_of_the_call() {
    let client = connect(host_with(vec![tool("slow", |ctx: Ctx<()>, _| async move {
        let progress = ctx.progress();
        progress.report(1, Some(3), "one").await;
        progress.report(2, Some(3), "two").await;
        progress.report(3, Some(3), "three").await;
        Ok(Output::from("done"))
    })]))
    .await;
    let seen: Arc<Mutex<Vec<ClientProgress>>> = Arc::default();
    let sink = Arc::clone(&seen);

    let outcome = client
        .call(
            "slow",
            Map::new(),
            Some(Box::new(move |report| sink.lock().unwrap().push(report))),
        )
        .await
        .expect("the call works");

    assert_eq!(outcome.text, "done");
    let seen = seen.lock().unwrap();
    let messages: Vec<_> = seen.iter().map(|report| report.message.as_deref()).collect();
    assert_eq!(messages, [Some("one"), Some("two"), Some("three")], "{seen:?}");
    assert_eq!(seen[2].current, 3.0);
    assert_eq!(seen[2].total, Some(3.0));
}

#[tokio::test]
async fn a_call_without_a_callback_runs_the_same() {
    let client = connect(host_with(vec![tool("slow", |ctx: Ctx<()>, _| async move {
        ctx.progress().report(1, Some(1), "only").await;
        Ok(Output::from("done"))
    })]))
    .await;

    let outcome = client.call("slow", Map::new(), None).await.expect("works");

    assert_eq!(outcome.text, "done");
}

#[tokio::test]
async fn an_unknown_tool_is_the_callers_mistake_and_names_the_nearest() {
    let client = connect(host_with(vec![tool("click_element", |_, _| async {
        Ok(Output::from("clicked"))
    })]))
    .await;

    let error = client
        .call("clik_element", Map::new(), None)
        .await
        .expect_err("there is no such tool");

    assert_eq!(error.exit_code(), 2, "{error}");
    assert!(
        error.to_string().contains("did you mean click_element?"),
        "{error}"
    );
}

#[tokio::test]
async fn a_call_that_takes_too_long_is_a_timeout() {
    let client = connect_to(
        host_with(vec![tool("stuck", |_, _| async {
            tokio::time::sleep(Duration::from_secs(3600)).await;
            Ok(Output::from("never"))
        })]),
        Duration::from_secs(1),
    )
    .await;

    let error = client
        .call("stuck", Map::new(), None)
        .await
        .expect_err("the call times out");

    assert_eq!(error.exit_code(), 7, "{error}");
    assert!(error.to_string().contains("took longer than 1 s"), "{error}");
}

#[tokio::test]
async fn resources_are_listed_and_read() {
    let dir = tempfile::tempdir().unwrap();
    let client = connect(cdp::host(Settings::new(dir.path()))).await;

    let listed = client.list_resources().await.expect("resources/list works");
    let journal = client
        .read_resource("sbrs://journal")
        .await
        .expect("the journal is readable");

    let fixed: Vec<_> = listed.iter().filter(|r| !r.template).map(|r| r.uri.as_str()).collect();
    assert!(fixed.contains(&"sbrs://journal"), "{listed:?}");
    let template = listed.iter().find(|r| r.template).expect("a template");
    assert_eq!(template.uri, "sbrs://output/{+path}");
    assert_eq!(journal.len(), 1);
    let body: Value = serde_json::from_str(journal[0].text.as_deref().expect("text")).unwrap();
    assert!(body["calls"].is_array(), "{body}");
    assert_eq!(journal[0].blob, None);
}

#[tokio::test]
async fn a_resource_the_server_refuses_or_does_not_know_is_the_callers_mistake() {
    let dir = tempfile::tempdir().unwrap();
    let client = connect(cdp::host(Settings::new(dir.path()))).await;

    let refused = client
        .read_resource("sbrs://output/../Cargo.toml")
        .await
        .expect_err("a path that leaves the folder is refused");
    let unknown = client
        .read_resource("sbrs://nothing/here")
        .await
        .expect_err("there is no such resource");

    assert_eq!(refused.exit_code(), 2, "{refused}");
    assert_eq!(unknown.exit_code(), 2, "{unknown}");
}

#[tokio::test]
async fn prompts_are_listed_and_rendered() {
    let dir = tempfile::tempdir().unwrap();
    let client = connect(cdp::host(Settings::new(dir.path()))).await;

    let prompts = client.list_prompts().await.expect("prompts/list works");
    let rendered = client
        .get_prompt(
            "write_test",
            object(&json!({ "url": "https://shop.test", "goal": "buy a mug" })),
        )
        .await
        .expect("the prompt renders");
    let missing = client
        .get_prompt("write_test", object(&json!({ "url": "https://a.test" })))
        .await
        .expect_err("an argument is missing");

    let write_test = prompts.iter().find(|p| p.name == "write_test").expect("write_test");
    assert!(write_test.arguments.iter().any(|a| a.name == "url" && a.required));
    assert_eq!(rendered.messages.len(), 1);
    assert_eq!(rendered.messages[0].0, "user");
    assert!(rendered.messages[0].1.contains("buy a mug"), "{rendered:?}");
    assert_eq!(missing.exit_code(), 2, "{missing}");
}

#[tokio::test]
async fn a_client_can_be_closed() {
    let client = connect(host_with(vec![])).await;

    client.close().await;
}
