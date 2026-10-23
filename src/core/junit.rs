//! JUnit XML for a run (`--junit-xml`).
//!
//! The renderer is the work of the renderers lane. Until it lands,
//! [`write_junit_xml`] reports that it cannot write, and a run records that as
//! a warning (see [`FinalizeOutcome`](super::test_run::FinalizeOutcome)) instead of
//! failing a test.

use std::io;
use std::path::Path;

use crate::core::report_helper::RunSnapshot;

/// Writes the JUnit XML of `run` to `path`.
///
/// # Errors
///
/// Returns an [`Unsupported`](io::ErrorKind::Unsupported) error: the renderer is
/// not built yet.
pub fn write_junit_xml(_path: &Path, _run: &RunSnapshot) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "the JUnit XML renderer is not built yet",
    ))
}
