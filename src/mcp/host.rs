//! The server: a set of tools sharing one browser session.

use std::future::Future;
use std::path::{Component, Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, PoisonError};

use rmcp::handler::server::ServerHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, ErrorData,
    Implementation, ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerConfig,
    Tool,
};
use rmcp::service::{RequestContext, RoleServer};
use serde_json::{Map, Value};
use tokio::sync::{Mutex, OwnedMappedMutexGuard, OwnedMutexGuard};

use super::failure::{concerns_page, focus_selector, FailureReport, PROBE_TIMEOUT};
use super::journal::{CallRecord, Journal};
use super::tool::{Args, Output, ToolDef, ToolError};
use crate::config::plugins::PluginSelection;
use crate::error::closest_match;

/// Environment variable naming the directory tools write files into.
pub const OUTPUT_DIR_VAR: &str = "SB_MCP_OUTPUT_DIR";

const DEFAULT_OUTPUT_DIR: &str = "mcp_output";

/// A browser session the server owns and must release on shutdown.
pub trait Closeable: Send + 'static {
    /// Releases the session, ignoring failures: the server is going away.
    fn close(self) -> impl Future<Output = ()> + Send;
}

/// What the operator of the server decides: where tools may write files, and
/// which plugins every browser the server starts gets.
///
/// A model-supplied file name must never reach outside the output directory,
/// so every output path is built here and nowhere else. The plugins are the
/// operator's choice (`sbrs-mcp --plugin`): no tool argument adds one.
#[derive(Debug, Clone)]
pub struct Settings {
    output_dir: PathBuf,
    plugins: PluginSelection,
}

impl Settings {
    /// Writes into `output_dir`, and starts browsers without plugins.
    #[must_use]
    pub fn new(output_dir: impl Into<PathBuf>) -> Self {
        Self {
            output_dir: output_dir.into(),
            plugins: PluginSelection::new(),
        }
    }

    /// Gives every browser the server starts the plugins `plugins` names
    /// (the `[plugins]` table, `SB_PLUGINS`, `--plugin` and `--plugin-opt`).
    ///
    /// A build without the feature `plugins` refuses a browser start when the
    /// selection is not empty; check it first with
    /// [`check_selection`](crate::cli::plugin_cmd::check_selection).
    #[must_use]
    pub fn with_plugin_selection(mut self, plugins: PluginSelection) -> Self {
        self.plugins = plugins;
        self
    }

    /// The plugins every browser the server starts gets.
    #[must_use]
    pub fn plugin_selection(&self) -> &PluginSelection {
        &self.plugins
    }

    /// Reads [`OUTPUT_DIR_VAR`], falling back to `./mcp_output`.
    #[must_use]
    pub fn from_env() -> Self {
        let dir = std::env::var_os(OUTPUT_DIR_VAR)
            .map_or_else(|| PathBuf::from(DEFAULT_OUTPUT_DIR), PathBuf::from);
        Self::new(dir)
    }

    /// The directory output goes to.
    #[must_use]
    pub fn output_dir(&self) -> &Path {
        &self.output_dir
    }

    /// The path for `file`, optionally inside the sub-folder `folder`.
    ///
    /// # Errors
    ///
    /// Returns [`ToolError::Refused`] if either part is absolute or climbs out
    /// of the output directory, or if `file` is empty.
    pub fn output_path(&self, folder: Option<&str>, file: &str) -> Result<PathBuf, ToolError> {
        let mut path = self.output_dir.clone();
        if let Some(folder) = folder.filter(|folder| !folder.is_empty()) {
            path.push(confined(folder, "folder")?);
        }
        if file.is_empty() {
            return Err(ToolError::Refused("the file name is empty".to_owned()));
        }
        path.push(confined(file, "file name")?);
        Ok(path)
    }
}

/// `relative` unchanged if it stays inside whatever directory it is joined to.
fn confined(relative: &str, what: &str) -> Result<PathBuf, ToolError> {
    let path = Path::new(relative);
    let stays_inside = path
        .components()
        .all(|part| matches!(part, Component::Normal(_) | Component::CurDir));
    if stays_inside {
        Ok(path.to_path_buf())
    } else {
        Err(ToolError::Refused(format!(
            "the {what} {relative:?} must be a relative path inside the output directory"
        )))
    }
}

/// Whether [`Ctx::start`] created the session or found one already running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Started {
    /// A new session was created.
    Created,
    /// A session already existed and was left as it is.
    AlreadyRunning,
}

/// A locked view of the running session.
///
/// Holding it keeps other tool calls waiting, which is what a single shared
/// browser needs: two clicks must not interleave.
pub type Session<S> = OwnedMappedMutexGuard<Option<S>, S>;

/// What a tool handler gets: the shared session slot, the settings, and the
/// log of the calls made so far.
pub struct Ctx<S> {
    slot: Arc<Mutex<Option<S>>>,
    settings: Arc<Settings>,
    journal: Arc<std::sync::Mutex<Journal>>,
}

impl<S> Clone for Ctx<S> {
    fn clone(&self) -> Self {
        Self {
            slot: Arc::clone(&self.slot),
            settings: Arc::clone(&self.settings),
            journal: Arc::clone(&self.journal),
        }
    }
}

impl<S> std::fmt::Debug for Ctx<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ctx")
            .field("settings", &self.settings)
            .finish_non_exhaustive()
    }
}

impl<S: Send + 'static> Ctx<S> {
    /// An empty slot: no browser yet.
    #[must_use]
    pub fn new(settings: Settings) -> Self {
        Self {
            slot: Arc::new(Mutex::new(None)),
            settings: Arc::new(settings),
            journal: Arc::default(),
        }
    }

    /// Adds a finished call to the log.
    pub(crate) fn record(&self, record: CallRecord) {
        self.journal
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(record);
    }

    /// The calls in the log, oldest first, and how many older ones were
    /// dropped to keep it bounded.
    pub(crate) fn calls(&self) -> (Vec<CallRecord>, usize) {
        let journal = self.journal.lock().unwrap_or_else(PoisonError::into_inner);
        (journal.calls(), journal.dropped())
    }

    /// Where tools may write files.
    #[must_use]
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// The running session, locked for the caller.
    ///
    /// # Errors
    ///
    /// Returns [`ToolError::NoBrowser`] if none has been started.
    pub async fn session(&self) -> Result<Session<S>, ToolError> {
        self.try_session().await.ok_or(ToolError::NoBrowser)
    }

    /// The running session, or `None` if there is not one.
    pub async fn try_session(&self) -> Option<Session<S>> {
        let guard = Arc::clone(&self.slot).lock_owned().await;
        OwnedMutexGuard::try_map(guard, Option::as_mut).ok()
    }

    /// Creates the session with `create` unless one is already running.
    ///
    /// The slot stays locked while `create` runs, so two racing calls cannot
    /// both launch a browser.
    ///
    /// # Errors
    ///
    /// Returns whatever `create` returns.
    pub async fn start<F>(&self, create: F) -> Result<Started, ToolError>
    where
        F: Future<Output = Result<S, ToolError>>,
    {
        let mut slot = self.slot.lock().await;
        if slot.is_some() {
            return Ok(Started::AlreadyRunning);
        }
        *slot = Some(create.await?);
        Ok(Started::Created)
    }

    /// Removes the session from the slot, leaving the server without one.
    pub async fn take(&self) -> Option<S> {
        self.slot.lock().await.take()
    }
}

/// The future a [`Probe`] returns: what the page looks like, or `None` when
/// there is no page to look at.
pub(crate) type ProbeFuture = Pin<Box<dyn Future<Output = Option<Value>> + Send>>;

/// Reads the page after a failed call, for the failure report. It is given
/// the selector the call was about, if any.
pub(crate) type Probe<S> = fn(Ctx<S>, Option<String>) -> ProbeFuture;

struct Shared<S> {
    name: &'static str,
    instructions: &'static str,
    tools: Vec<ToolDef<S>>,
    ctx: Ctx<S>,
    probe: Option<Probe<S>>,
}

/// A named set of tools and the session they share.
///
/// A cheap handle: clones share the same tools and the same browser.
pub struct Host<S> {
    shared: Arc<Shared<S>>,
}

impl<S> Clone for Host<S> {
    fn clone(&self) -> Self {
        Self {
            shared: Arc::clone(&self.shared),
        }
    }
}

impl<S> std::fmt::Debug for Host<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Host")
            .field("name", &self.shared.name)
            .field("tools", &self.shared.tools.len())
            .finish_non_exhaustive()
    }
}

impl<S: Send + 'static> Host<S> {
    /// Assembles a server from its tools.
    #[must_use]
    pub fn new(
        name: &'static str,
        instructions: &'static str,
        tools: Vec<ToolDef<S>>,
        settings: Settings,
    ) -> Self {
        Self {
            shared: Arc::new(Shared {
                name,
                instructions,
                tools,
                ctx: Ctx::new(settings),
                probe: None,
            }),
        }
    }

    /// Reads the page with `probe` when a call fails for a reason the page
    /// can explain.
    ///
    /// Call it while building the server: a host that has been cloned keeps
    /// the probe it had.
    #[must_use]
    pub(crate) fn with_probe(mut self, probe: Probe<S>) -> Self {
        if let Some(shared) = Arc::get_mut(&mut self.shared) {
            shared.probe = Some(probe);
        } else {
            tracing::warn!(server = self.shared.name, "a shared host keeps its probe");
        }
        self
    }

    /// Whether the server offers a tool called `name`.
    #[must_use]
    pub fn has_tool(&self, name: &str) -> bool {
        self.shared.tools.iter().any(|tool| tool.name() == name)
    }

    /// The server's name, as clients see it.
    #[must_use]
    pub fn name(&self) -> &'static str {
        self.shared.name
    }

    /// The names of the tools, in the order they are listed.
    #[must_use]
    pub fn tool_names(&self) -> Vec<&'static str> {
        self.shared.tools.iter().map(ToolDef::name).collect()
    }

    /// The tools as clients see them: name, description, schema, annotations.
    #[must_use]
    pub fn describe_tools(&self) -> Vec<Tool> {
        self.shared.tools.iter().map(ToolDef::describe).collect()
    }

    /// The shared context, for tests that inspect or seed the session.
    #[must_use]
    pub fn ctx(&self) -> &Ctx<S> {
        &self.shared.ctx
    }

    /// Runs one tool, exactly as a client's `tools/call` would.
    ///
    /// # Errors
    ///
    /// Returns [`ToolError::UnknownTool`] for a name that is not registered,
    /// and whatever the tool itself returns.
    pub async fn call(
        &self,
        name: &str,
        arguments: Map<String, Value>,
    ) -> Result<Output, ToolError> {
        let tool = self
            .shared
            .tools
            .iter()
            .find(|tool| tool.name() == name)
            .ok_or_else(|| ToolError::UnknownTool(name.to_owned()))?;
        let logged = arguments.clone();
        let result = tool
            .call(self.shared.ctx.clone(), Args::new(arguments))
            .await;
        let outcome = match &result {
            Ok(output) => output.render(),
            Err(error) => error.to_string(),
        };
        self.shared.ctx.record(CallRecord::new(
            name,
            tool.effect(),
            &logged,
            result.is_ok(),
            &outcome,
        ));
        result
    }

    /// Runs one tool like [`call`](Self::call), and explains a failure.
    ///
    /// # Errors
    ///
    /// Returns a [`FailureReport`] for any failure; see [`explain`](Self::explain).
    pub async fn call_explained(
        &self,
        name: &str,
        arguments: Map<String, Value>,
    ) -> Result<Output, Box<FailureReport>> {
        let kept = arguments.clone();
        match self.call(name, arguments).await {
            Ok(output) => Ok(output),
            Err(error) => Err(Box::new(self.explain(name, &kept, &error).await)),
        }
    }

    /// Explains why calling `tool` with `arguments` failed with `error`.
    ///
    /// For a failure the page can explain (an element, a wait, an assertion,
    /// a navigation or a script), the page is read first: URL, title, ready
    /// state, recent script errors, and how many elements the selector
    /// matches. Reading it takes at most two seconds and never fails the
    /// report; a page that does not answer in time is reported as not read.
    pub async fn explain(
        &self,
        tool: &str,
        arguments: &Map<String, Value>,
        error: &ToolError,
    ) -> FailureReport {
        let page = match self.shared.probe {
            Some(probe) if concerns_page(error) => {
                let selector = focus_selector(error, arguments);
                tokio::time::timeout(PROBE_TIMEOUT, probe(self.shared.ctx.clone(), selector))
                    .await
                    .unwrap_or_else(|_| {
                        Some(serde_json::json!({
                            "unavailable": "the page did not answer within 2 seconds"
                        }))
                    })
            }
            _ => None,
        };
        FailureReport::new(tool, arguments, error, page, &|name| self.has_tool(name))
    }

    /// The protocol message for a call to a tool that does not exist, with
    /// the nearest real name when the given one looks like a typo of it.
    fn unknown_tool(&self, name: &str) -> String {
        let near = closest_match(name, self.shared.tools.iter().map(ToolDef::name));
        match near {
            Some(near) => format!("unknown tool: {name}; did you mean {near}?"),
            None => format!("unknown tool: {name}"),
        }
    }
}

impl<S: Closeable> Host<S> {
    /// Releases the browser session, if one is running.
    pub async fn shutdown(&self) {
        if let Some(session) = self.shared.ctx.take().await {
            session.close().await;
        }
    }
}

/// A failed call as the client sees it: the report as text, and the same
/// fields as structured content.
fn error_result(report: &FailureReport) -> CallToolResponse {
    let mut result = CallToolResult::error(vec![ContentBlock::text(report.render())]);
    result.structured_content = Some(report.to_json());
    CallToolResponse::Complete(result)
}

#[expect(
    clippy::manual_async_fn,
    reason = "the trait declares these methods as returning `impl Future`"
)]
impl<S: Send + 'static> ServerHandler for Host<S> {
    fn get_info(&self) -> ServerConfig {
        // A client names the server by this: without it, every server of this
        // crate introduces itself as the MCP library.
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                self.shared.name,
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(self.shared.instructions)
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, ErrorData>> + '_ {
        async move {
            Ok(ListToolsResult {
                tools: self.describe_tools(),
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
            let arguments = request.arguments.unwrap_or_default();
            match self.call(&request.name, arguments.clone()).await {
                Ok(output) => Ok(CallToolResponse::Complete(CallToolResult::success(vec![
                    ContentBlock::text(output.render()),
                ]))),
                Err(ToolError::UnknownTool(name)) => {
                    Err(ErrorData::invalid_params(self.unknown_tool(&name), None))
                }
                Err(error) => Ok(error_result(
                    &self.explain(&request.name, &arguments, &error).await,
                )),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_paths_stay_inside_the_output_directory() {
        let settings = Settings::new("out");
        assert_eq!(
            settings.output_path(None, "a.png").unwrap(),
            Path::new("out").join("a.png")
        );
        assert_eq!(
            settings.output_path(Some("shots/day1"), "a.png").unwrap(),
            Path::new("out").join("shots/day1").join("a.png")
        );
        assert_eq!(
            settings.output_path(Some(""), "a.png").unwrap(),
            Path::new("out").join("a.png"),
            "an empty folder means none"
        );
    }

    #[test]
    fn a_mistyped_tool_name_is_answered_with_the_nearest_one() {
        let host: Host<()> = Host::new(
            "test",
            "",
            vec![ToolDef::new(
                "click_element",
                "Click",
                "Clicks.",
                super::super::Effect::Act,
                super::super::Schema::new(),
                |_ctx: Ctx<()>, _args: Args| async { Ok(Output::from("ok")) },
            )],
            Settings::new("out"),
        );

        assert_eq!(
            host.unknown_tool("clik_element"),
            "unknown tool: clik_element; did you mean click_element?"
        );
        assert_eq!(
            host.unknown_tool("make_coffee"),
            "unknown tool: make_coffee"
        );
    }

    #[test]
    fn the_initialize_reply_names_the_server_and_this_crates_version() {
        let host: Host<()> = Host::new(
            "seleniumbase-test",
            "Instructions.",
            Vec::new(),
            Settings::new("out"),
        );

        let info = host.get_info();

        assert_eq!(info.server_info.name, "seleniumbase-test");
        assert_eq!(info.server_info.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(info.instructions.as_deref(), Some("Instructions."));
    }

    #[tokio::test]
    async fn every_call_is_logged_with_how_it_ended() {
        let host: Host<()> = Host::new(
            "test",
            "",
            vec![ToolDef::new(
                "needs_browser",
                "Needs Browser",
                "Fails without a browser.",
                super::super::Effect::Act,
                super::super::Schema::new(),
                |ctx: Ctx<()>, _args: Args| async move {
                    let _session = ctx.session().await?;
                    Ok(Output::from("ok"))
                },
            )],
            Settings::new("out"),
        );
        let arguments = serde_json::json!({ "selector": "#x" })
            .as_object()
            .cloned()
            .unwrap();

        let report = host
            .call_explained("needs_browser", arguments)
            .await
            .unwrap_err();

        assert_eq!(report.kind, "no_browser");
        assert!(report.page.is_none(), "no page is read without a browser");
        let (calls, dropped) = host.ctx().calls();
        assert_eq!((calls.len(), dropped), (1, 0));
        assert!(!calls[0].ok);
        assert_eq!(calls[0].arguments["selector"], "#x");
    }

    #[tokio::test(start_paused = true)]
    async fn a_page_that_never_answers_does_not_hold_up_the_report() {
        fn hangs(_ctx: Ctx<()>, _selector: Option<String>) -> ProbeFuture {
            Box::pin(std::future::pending())
        }
        let host: Host<()> = Host::new(
            "test",
            "",
            vec![ToolDef::new(
                "click_element",
                "Click",
                "Fails the way a missing element does.",
                super::super::Effect::Act,
                super::super::Schema::new(),
                |_ctx: Ctx<()>, _args: Args| async {
                    Err::<Output, _>(ToolError::Failed(
                        crate::error::SeleniumBaseError::element_not_found("#x"),
                    ))
                },
            )],
            Settings::new("out"),
        )
        .with_probe(hangs);
        let started = tokio::time::Instant::now();

        let report = host
            .call_explained("click_element", Map::new())
            .await
            .unwrap_err();

        assert_eq!(started.elapsed(), PROBE_TIMEOUT);
        assert_eq!(report.kind, "element_not_found");
        assert_eq!(
            report.page.unwrap()["unavailable"],
            "the page did not answer within 2 seconds"
        );
    }

    #[test]
    fn paths_that_climb_out_or_are_absolute_are_refused() {
        let settings = Settings::new("out");
        for (folder, file) in [
            (None, "../a.png"),
            (None, "/etc/passwd"),
            (None, "sub/../../a.png"),
            (Some("../elsewhere"), "a.png"),
            (Some("/tmp"), "a.png"),
            (None, ""),
        ] {
            let outcome = settings.output_path(folder, file);
            assert!(
                matches!(outcome, Err(ToolError::Refused(_))),
                "{folder:?}/{file:?} should be refused, got {outcome:?}"
            );
        }
    }
}
