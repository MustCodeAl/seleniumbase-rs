//! Resources: read-only data the server offers next to its tools.
//!
//! A tool does something; a resource is something a client can read, and can
//! put in front of the model without a tool call. The server offers three:
//!
//! - `sbrs://journal`: the tool calls made so far, with typed text, scripts
//!   and results left out, as `capture_debug_bundle` does.
//! - `sbrs://session`: the engine, URL, title and ready state of the
//!   running browser, or `{"browser": false}`.
//! - `sbrs://output/{+path}`: a file a tool wrote under the output
//!   directory (`SB_MCP_OUTPUT_DIR`). Text is returned as text, anything else
//!   as base64. A path that leaves the directory, by `..`, an absolute path
//!   or a symbolic link, is refused.
//!
//! Resources never change and are never subscribed to: the list is fixed
//! when the server starts.

use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::time::Duration;

use base64::Engine as _;
use serde_json::{json, Value};

use super::devtools::DevSession;
use super::host::{Ctx, Settings};
use super::journal::CallRecord;

/// The URI of the call journal.
pub const JOURNAL_URI: &str = "sbrs://journal";

/// The URI of the session summary.
pub const SESSION_URI: &str = "sbrs://session";

/// The URI template of the files under the output directory.
pub const OUTPUT_TEMPLATE: &str = "sbrs://output/{+path}";

/// The largest file `sbrs://output/{+path}` returns, in bytes. A resource
/// travels in one message, so a bigger file is refused instead of read.
pub const MAX_OUTPUT_BYTES: u64 = 8 * 1024 * 1024;

/// How long reading `sbrs://session` waits for a tool call that is using the
/// browser before it says the browser is busy.
const SESSION_WAIT: Duration = Duration::from_secs(1);

/// What a resource read produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceBody {
    /// Text, with its media type.
    Text {
        /// The text.
        text: String,
        /// The media type, such as `application/json`.
        mime_type: String,
    },
    /// Bytes, sent base64-encoded, with their media type.
    Blob {
        /// The bytes before encoding.
        bytes: Vec<u8>,
        /// The media type, such as `image/png`.
        mime_type: String,
    },
}

impl ResourceBody {
    /// Text with media type `mime_type`.
    #[must_use]
    pub fn text(text: impl Into<String>, mime_type: impl Into<String>) -> Self {
        Self::Text {
            text: text.into(),
            mime_type: mime_type.into(),
        }
    }

    /// `value` as indented JSON.
    #[must_use]
    pub fn json(value: &Value) -> Self {
        let text = serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string());
        Self::text(text, "application/json")
    }

    /// Bytes with media type `mime_type`.
    #[must_use]
    pub fn blob(bytes: Vec<u8>, mime_type: impl Into<String>) -> Self {
        Self::Blob {
            bytes,
            mime_type: mime_type.into(),
        }
    }

    /// The bytes as the base64 text a client receives.
    pub(crate) fn encoded(bytes: &[u8]) -> String {
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }
}

/// Why a resource could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ResourceError {
    /// Nothing is at that URI.
    NotFound(String),
    /// The URI is not one the server may serve.
    Invalid(String),
    /// Reading failed.
    Failed(String),
}

impl std::fmt::Display for ResourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(message) | Self::Invalid(message) | Self::Failed(message) => {
                f.write_str(message)
            }
        }
    }
}

impl std::error::Error for ResourceError {}

/// The future a resource reader returns.
pub type ResourceFuture = Pin<Box<dyn Future<Output = Result<ResourceBody, ResourceError>> + Send>>;

type Reader<S> = Box<dyn Fn(Ctx<S>, String) -> ResourceFuture + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    /// One URI.
    Fixed,
    /// Every URI that starts with a prefix.
    Template,
}

/// A resource, or a family of them behind a URI template, and how to read it.
pub struct ResourceDef<S> {
    uri: &'static str,
    name: &'static str,
    title: &'static str,
    description: &'static str,
    mime_type: &'static str,
    shape: Shape,
    read: Reader<S>,
}

impl<S> std::fmt::Debug for ResourceDef<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResourceDef")
            .field("uri", &self.uri)
            .finish_non_exhaustive()
    }
}

impl<S> ResourceDef<S> {
    /// A resource at the single URI `uri`.
    #[must_use]
    pub fn fixed<F, Fut>(
        uri: &'static str,
        name: &'static str,
        title: &'static str,
        description: &'static str,
        mime_type: &'static str,
        read: F,
    ) -> Self
    where
        F: Fn(Ctx<S>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<ResourceBody, ResourceError>> + Send + 'static,
    {
        Self {
            uri,
            name,
            title,
            description,
            mime_type,
            shape: Shape::Fixed,
            read: Box::new(move |ctx, _uri| Box::pin(read(ctx))),
        }
    }

    /// The resources of a URI template `prefix{+name}`: every URI that starts
    /// with `prefix`. The reader is given what follows the prefix, still
    /// percent-encoded.
    #[must_use]
    pub fn template<F, Fut>(
        uri_template: &'static str,
        name: &'static str,
        title: &'static str,
        description: &'static str,
        read: F,
    ) -> Self
    where
        F: Fn(Ctx<S>, String) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<ResourceBody, ResourceError>> + Send + 'static,
    {
        let prefix = uri_template.split('{').next().unwrap_or(uri_template);
        Self {
            uri: uri_template,
            name,
            title,
            description,
            mime_type: "",
            shape: Shape::Template,
            read: Box::new(move |ctx, uri| {
                let rest = uri.strip_prefix(prefix).unwrap_or_default().to_owned();
                Box::pin(read(ctx, rest))
            }),
        }
    }

    /// The URI of a fixed resource, or the URI template of a family.
    #[must_use]
    pub fn uri(&self) -> &'static str {
        self.uri
    }

    /// The short name clients list the resource under.
    #[must_use]
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// The human-readable title.
    #[must_use]
    pub fn title(&self) -> &'static str {
        self.title
    }

    /// What the resource holds.
    #[must_use]
    pub fn description(&self) -> &'static str {
        self.description
    }

    /// The media type of a fixed resource; empty for a template, whose files
    /// differ.
    #[must_use]
    pub fn mime_type(&self) -> &'static str {
        self.mime_type
    }

    /// Whether this is a URI template and not a single resource.
    #[must_use]
    pub fn is_template(&self) -> bool {
        self.shape == Shape::Template
    }

    /// Whether `uri` is one this definition serves.
    pub(crate) fn serves(&self, uri: &str) -> bool {
        match self.shape {
            Shape::Fixed => self.uri == uri,
            Shape::Template => {
                let prefix = self.uri.split('{').next().unwrap_or(self.uri);
                uri.len() > prefix.len() && uri.starts_with(prefix)
            }
        }
    }

    pub(crate) fn read(&self, ctx: Ctx<S>, uri: &str) -> ResourceFuture {
        (self.read)(ctx, uri.to_owned())
    }
}

/// The resources every browser server offers.
#[must_use]
pub(crate) fn standard<S: DevSession>() -> Vec<ResourceDef<S>> {
    vec![
        ResourceDef::fixed(
            JOURNAL_URI,
            "journal",
            "Call journal",
            "The tool calls this server has run, oldest first: tool, arguments, whether it \
             succeeded and when. Typed text and scripts are left out, and so are results.",
            "application/json",
            |ctx: Ctx<S>| async move { Ok(journal(&ctx)) },
        ),
        ResourceDef::fixed(
            SESSION_URI,
            "session",
            "Browser session",
            "Whether a browser is running and, if so, its engine, URL, title and ready state.",
            "application/json",
            |ctx: Ctx<S>| async move { Ok(ResourceBody::json(&session(&ctx).await)) },
        ),
        ResourceDef::template(
            OUTPUT_TEMPLATE,
            "output",
            "Output file",
            "A file a tool wrote under the output directory, such as a screenshot or a saved \
             page. Text is returned as text and anything else base64-encoded.",
            |ctx: Ctx<S>, path: String| async move {
                read_output(ctx.settings(), &path, MAX_OUTPUT_BYTES).await
            },
        ),
    ]
}

/// `sbrs://journal`.
fn journal<S: Send + 'static>(ctx: &Ctx<S>) -> ResourceBody {
    let (calls, dropped) = ctx.calls();
    ResourceBody::json(&json!({
        "calls": calls.iter().map(CallRecord::redacted).collect::<Vec<_>>(),
        "dropped": dropped,
    }))
}

/// `sbrs://session`.
async fn session<S: DevSession>(ctx: &Ctx<S>) -> Value {
    let guard = match tokio::time::timeout(SESSION_WAIT, ctx.try_session()).await {
        Err(_) => {
            return json!({
                "browser": true,
                "busy": "a tool call is using the browser; read this again when it ends",
            })
        }
        Ok(None) => return json!({ "browser": false }),
        Ok(Some(guard)) => guard,
    };
    let mut session = guard;
    let engine = session.engine();
    match session
        .evaluate(
            "({ url: location.href, title: document.title, ready_state: document.readyState })",
        )
        .await
    {
        Ok(Value::Object(mut page)) => {
            page.insert("browser".to_owned(), Value::Bool(true));
            page.insert("engine".to_owned(), Value::from(engine));
            Value::Object(page)
        }
        Ok(other) => json!({ "browser": true, "engine": engine, "unexpected": other }),
        Err(error) => json!({
            "browser": true,
            "engine": engine,
            "unavailable": format!("the page could not be read: {error}"),
        }),
    }
}

/// The media type of a file, from its extension.
fn mime_type(path: &Path, text: bool) -> &'static str {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match extension.as_str() {
        "json" => "application/json",
        "html" | "htm" => "text/html",
        "csv" => "text/csv",
        "md" => "text/markdown",
        "xml" => "application/xml",
        "js" => "text/javascript",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        _ if text => "text/plain",
        _ => "application/octet-stream",
    }
}

/// `path` with its `%XX` escapes decoded.
fn percent_decode(path: &str) -> Result<String, ResourceError> {
    let invalid = || ResourceError::Invalid(format!("{path:?} is not a valid resource path"));
    let bytes = path.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'%' {
            let hex = bytes.get(at + 1..at + 3).ok_or_else(invalid)?;
            if !hex.iter().all(u8::is_ascii_hexdigit) {
                return Err(invalid());
            }
            let hex = std::str::from_utf8(hex).ok().ok_or_else(invalid)?;
            decoded.push(u8::from_str_radix(hex, 16).ok().ok_or_else(invalid)?);
            at += 3;
        } else {
            decoded.push(bytes[at]);
            at += 1;
        }
    }
    String::from_utf8(decoded).ok().ok_or_else(invalid)
}

/// Reads the file `path` (percent-encoded) under the output directory of
/// `settings`, refusing anything that is not a regular file inside it.
async fn read_output(
    settings: &Settings,
    path: &str,
    limit: u64,
) -> Result<ResourceBody, ResourceError> {
    let decoded = percent_decode(path)?;
    if decoded.contains('\0') {
        return Err(ResourceError::Invalid(
            "a resource path cannot hold a NUL".to_owned(),
        ));
    }
    let wanted = settings
        .output_path(None, &decoded)
        .map_err(|error| ResourceError::Invalid(error.to_string()))?;
    let not_found =
        || ResourceError::NotFound(format!("no file {decoded:?} in the output directory"));
    let root = tokio::fs::canonicalize(settings.output_dir())
        .await
        .ok()
        .ok_or_else(not_found)?;
    let real = tokio::fs::canonicalize(&wanted)
        .await
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => not_found(),
            _ => ResourceError::Failed(format!("{decoded:?} could not be read: {error}")),
        })?;
    // A link inside the folder that points out of it ends up outside `root`.
    if !real.starts_with(&root) {
        return Err(ResourceError::Invalid(format!(
            "{decoded:?} leads out of the output directory"
        )));
    }
    let metadata = tokio::fs::metadata(&real).await.map_err(|error| {
        ResourceError::Failed(format!("{decoded:?} could not be read: {error}"))
    })?;
    if !metadata.is_file() {
        return Err(ResourceError::Invalid(format!("{decoded:?} is not a file")));
    }
    if metadata.len() > limit {
        return Err(ResourceError::Invalid(format!(
            "{decoded:?} is {} bytes; resources are limited to {limit}",
            metadata.len()
        )));
    }
    let bytes = tokio::fs::read(&real).await.map_err(|error| {
        ResourceError::Failed(format!("{decoded:?} could not be read: {error}"))
    })?;
    Ok(match String::from_utf8(bytes) {
        Ok(text) => ResourceBody::text(text, mime_type(&real, true)),
        Err(error) => ResourceBody::blob(error.into_bytes(), mime_type(&real, false)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(dir: &Path) -> Settings {
        Settings::new(dir)
    }

    #[test]
    fn escapes_are_decoded_and_bad_ones_refused() {
        assert_eq!(percent_decode("a%20b%2Fc").unwrap(), "a b/c");
        assert_eq!(percent_decode("plain.txt").unwrap(), "plain.txt");
        for bad in ["%", "%2", "%zz", "%+1", "%ff"] {
            assert!(
                matches!(percent_decode(bad), Err(ResourceError::Invalid(_))),
                "{bad:?} should be refused"
            );
        }
    }

    #[tokio::test]
    async fn text_files_come_back_as_text_and_others_as_bytes() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("shots")).unwrap();
        std::fs::write(dir.path().join("note.txt"), "hello").unwrap();
        std::fs::write(dir.path().join("shots/a.png"), [0x89, b'P', 0xff, 0x00]).unwrap();

        let text = read_output(&settings(dir.path()), "note.txt", 1000).await;
        let image = read_output(&settings(dir.path()), "shots/a.png", 1000).await;

        assert_eq!(text, Ok(ResourceBody::text("hello", "text/plain")));
        assert_eq!(
            image,
            Ok(ResourceBody::blob(
                vec![0x89, b'P', 0xff, 0x00],
                "image/png"
            ))
        );
    }

    #[tokio::test]
    async fn paths_that_leave_the_directory_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("out");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(dir.path().join("secret.txt"), "no").unwrap();
        let settings = settings(&root);

        for path in [
            "../secret.txt",
            "%2e%2e/secret.txt",
            "a/../../secret.txt",
            "/etc/passwd",
            "%2Fetc%2Fpasswd",
            "",
        ] {
            let outcome = read_output(&settings, path, 1000).await;
            assert!(
                matches!(outcome, Err(ResourceError::Invalid(_))),
                "{path:?} should be refused, got {outcome:?}"
            );
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_link_that_leaves_the_directory_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("out");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(dir.path().join("secret.txt"), "no").unwrap();
        std::os::unix::fs::symlink(dir.path().join("secret.txt"), root.join("link.txt")).unwrap();
        std::os::unix::fs::symlink(dir.path(), root.join("up")).unwrap();
        std::fs::write(root.join("inside.txt"), "yes").unwrap();
        std::os::unix::fs::symlink(root.join("inside.txt"), root.join("ok.txt")).unwrap();
        let settings = settings(&root);

        for path in ["link.txt", "up/secret.txt"] {
            let outcome = read_output(&settings, path, 1000).await;
            assert!(
                matches!(outcome, Err(ResourceError::Invalid(_))),
                "{path:?} should be refused, got {outcome:?}"
            );
        }
        assert_eq!(
            read_output(&settings, "ok.txt", 1000).await,
            Ok(ResourceBody::text("yes", "text/plain")),
            "a link that stays inside is fine"
        );
    }

    #[tokio::test]
    async fn missing_big_and_non_file_paths_are_told_apart() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("folder")).unwrap();
        std::fs::write(dir.path().join("big.txt"), "0123456789").unwrap();
        let settings = settings(dir.path());

        assert!(matches!(
            read_output(&settings, "missing.txt", 1000).await,
            Err(ResourceError::NotFound(_))
        ));
        assert!(matches!(
            read_output(&settings, "folder", 1000).await,
            Err(ResourceError::Invalid(_))
        ));
        assert!(matches!(
            read_output(&settings, "big.txt", 4).await,
            Err(ResourceError::Invalid(_))
        ));
        assert!(matches!(
            read_output(&Settings::new(dir.path().join("never made")), "a.txt", 4).await,
            Err(ResourceError::NotFound(_))
        ));
    }

    #[test]
    fn a_template_serves_what_follows_its_prefix() {
        let template: ResourceDef<()> = ResourceDef::template(
            OUTPUT_TEMPLATE,
            "output",
            "Output",
            "Files.",
            |_ctx, _path| async { Ok(ResourceBody::text("", "text/plain")) },
        );

        assert!(template.is_template());
        assert!(template.serves("sbrs://output/a.png"));
        assert!(
            !template.serves("sbrs://output/"),
            "nothing follows the prefix"
        );
        assert!(!template.serves("sbrs://journal"));
    }

    #[test]
    fn a_fixed_resource_serves_only_its_own_uri() {
        let journal: ResourceDef<()> = ResourceDef::fixed(
            JOURNAL_URI,
            "journal",
            "Journal",
            "Calls.",
            "application/json",
            |_ctx| async { Ok(ResourceBody::text("[]", "application/json")) },
        );

        assert!(!journal.is_template());
        assert!(journal.serves("sbrs://journal"));
        assert!(!journal.serves("sbrs://journal/x"));
    }
}
