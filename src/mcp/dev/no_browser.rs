//! The session of the `dev` server: there is none.

use super::Closeable;

/// The session of the `dev` server: there is none, because the server has no
/// browser. A tool that asked for one would get
/// [`ToolError::NoBrowser`](super::ToolError::NoBrowser).
#[derive(Debug, Clone, Copy, Default)]
pub struct NoBrowser;

impl Closeable for NoBrowser {
    async fn close(self) {}
}
