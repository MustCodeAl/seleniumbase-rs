//! The server: a set of tools sharing one browser session.

use std::future::Future;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use rmcp::handler::server::ServerHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, ErrorData,
    ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerConfig, Tool,
};
use rmcp::service::{RequestContext, RoleServer};
use serde_json::{Map, Value};
use tokio::sync::{Mutex, OwnedMappedMutexGuard, OwnedMutexGuard};

use super::tool::{Args, Output, ToolDef, ToolError};

/// Environment variable naming the directory tools write files into.
pub const OUTPUT_DIR_VAR: &str = "SB_MCP_OUTPUT_DIR";

const DEFAULT_OUTPUT_DIR: &str = "mcp_output";

/// A browser session the server owns and must release on shutdown.
pub trait Closeable: Send + 'static {
    /// Releases the session, ignoring failures: the server is going away.
    fn close(self) -> impl Future<Output = ()> + Send;
}

/// Where tools may write files.
///
/// A model-supplied file name must never reach outside this directory, so
/// every output path is built here and nowhere else.
#[derive(Debug, Clone)]
pub struct Settings {
    output_dir: PathBuf,
}

impl Settings {
    /// Writes into `output_dir`.
    #[must_use]
    pub fn new(output_dir: impl Into<PathBuf>) -> Self {
        Self {
            output_dir: output_dir.into(),
        }
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

/// What a tool handler gets: the shared session slot and the settings.
pub struct Ctx<S> {
    slot: Arc<Mutex<Option<S>>>,
    settings: Arc<Settings>,
}

impl<S> Clone for Ctx<S> {
    fn clone(&self) -> Self {
        Self {
            slot: Arc::clone(&self.slot),
            settings: Arc::clone(&self.settings),
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
        }
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

struct Shared<S> {
    name: &'static str,
    instructions: &'static str,
    tools: Vec<ToolDef<S>>,
    ctx: Ctx<S>,
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
            }),
        }
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
        tool.call(self.shared.ctx.clone(), Args::new(arguments))
            .await
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

fn error_result(message: String) -> CallToolResponse {
    CallToolResponse::Complete(CallToolResult::error(vec![ContentBlock::text(message)]))
}

#[expect(
    clippy::manual_async_fn,
    reason = "the trait declares these methods as returning `impl Future`"
)]
impl<S: Send + 'static> ServerHandler for Host<S> {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
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
            match self.call(&request.name, arguments).await {
                Ok(output) => Ok(CallToolResponse::Complete(CallToolResult::success(vec![
                    ContentBlock::text(output.render()),
                ]))),
                Err(ToolError::UnknownTool(name)) => Err(ErrorData::invalid_params(
                    format!("unknown tool: {name}"),
                    None,
                )),
                Err(error) => Ok(error_result(error.message())),
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
