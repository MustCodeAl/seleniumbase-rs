//! Helpers shared by the tool handlers of every server.

use std::path::{Path, PathBuf};

use super::{Args, Output, Settings, ToolError};
use crate::error::SeleniumBaseError;

/// The folder under the output directory that cookie files live in.
const COOKIE_DIR: &str = "saved_cookies";

/// A text argument that counts as unset when empty, as clients often send "".
pub(super) fn nonempty<'a>(args: &'a Args, name: &str) -> Result<Option<&'a str>, ToolError> {
    Ok(args.opt_str(name)?.filter(|text| !text.is_empty()))
}

/// Serializes a value as a JSON tool result.
pub(super) fn json_output(value: impl serde::Serialize) -> Result<Output, ToolError> {
    Ok(Output::Json(
        serde_json::to_value(value).map_err(SeleniumBaseError::from)?,
    ))
}

/// Whether a lookup failed only because nothing matched in time.
pub(super) fn is_missing(error: &SeleniumBaseError) -> bool {
    matches!(
        error,
        SeleniumBaseError::ElementNotFound { .. } | SeleniumBaseError::WaitTimeout { .. }
    )
}

/// Where the cookie file called `requested` lives, and the name it is stored
/// under.
///
/// Only the last path part of `requested` is honoured, and `.txt` is added if
/// missing, so a client cannot name a file outside the cookie directory.
pub(super) fn cookie_file(
    settings: &Settings,
    requested: &str,
) -> Result<(PathBuf, String), ToolError> {
    let name = Path::new(requested)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| ToolError::invalid("filename", "not a usable file name"))?;
    let name = if Path::new(name).extension().is_some_and(|ext| ext == "txt") {
        name.to_owned()
    } else {
        format!("{name}.txt")
    };
    let path = settings.output_path(Some(COOKIE_DIR), &name)?;
    Ok((path, name))
}

/// Writes `bytes` to `path`, creating its folder first.
pub(super) async fn write_file(path: &Path, bytes: &[u8]) -> Result<(), ToolError> {
    if let Some(dir) = path.parent() {
        tokio::fs::create_dir_all(dir)
            .await
            .map_err(SeleniumBaseError::from)?;
    }
    tokio::fs::write(path, bytes)
        .await
        .map_err(SeleniumBaseError::from)?;
    Ok(())
}
