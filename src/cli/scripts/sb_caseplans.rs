//! `sbase caseplans`: a starter test-plan checklist.

use crate::cli::commands::write_new_file;
use std::path::{Path, PathBuf};

/// The file `sbase caseplans` writes, in the current folder.
pub const FILE_NAME: &str = "CASE_PLANS.md";

const CASE_PLANS: &str = r#"# Test Case Plans

## Smoke Tests
- [ ] Open homepage and verify title
- [ ] Login flow validation
- [ ] Navigation between pages

## Feature Tests
- [ ] Form submission
- [ ] File upload
- [ ] API integration
"#;

/// Writes the starter plan to `CASE_PLANS.md` in the current folder.
///
/// An existing `CASE_PLANS.md` holds the user's plans and is never replaced:
/// that is an [`std::io::ErrorKind::AlreadyExists`] error.
pub fn run_caseplans() -> std::io::Result<PathBuf> {
    let path = PathBuf::from(FILE_NAME);
    write_case_plans(&path)?;
    Ok(path)
}

/// Writes the starter plan to `path`, which must not exist yet.
pub fn write_case_plans(path: &Path) -> std::io::Result<()> {
    write_new_file(path, CASE_PLANS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_run_keeps_the_plans_the_user_has_written() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE_NAME);

        write_case_plans(&path).unwrap();
        assert!(std::fs::read_to_string(&path)
            .unwrap()
            .contains("## Smoke Tests"));
        std::fs::write(&path, "- [x] my own plan\n").unwrap();
        let error = write_case_plans(&path).unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "- [x] my own plan\n"
        );
    }
}
