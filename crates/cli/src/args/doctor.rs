//! What `sbrs doctor --fix` repairs, decided from the check and applied without a browser.
//!
//! The check already names two repairs: install a chromedriver (`sbrs install chromedriver`)
//! and make the patch-cache directory. `--fix` does those and nothing else. It does not install
//! a browser and it does not patch Chrome or chromedriver.

use std::path::{Path, PathBuf};

/// The sentence a version mismatch contributes to the failure list.
const MISMATCH_MARK: &str = "cannot drive Chrome";

/// Printed when Chrome itself is missing. `--fix` does not download a browser.
const CANNOT_INSTALL_BROWSER: &str = "--fix cannot install a browser";

/// What the check found about chromedriver. Majors are `None` when the binary's version
/// could not be read; that is not a mismatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ChromedriverFinding {
    /// No chromedriver lookup would use.
    ///
    /// `configured` means `CHROMEDRIVER_PATH` names a path that is not a file. A download
    /// into `downloaded_drivers/` does not change that variable.
    Missing {
        /// `CHROMEDRIVER_PATH` is set to a path that is not a file.
        configured: bool,
    },
    /// A chromedriver binary was found.
    Found {
        /// Its major version, when `--version` (or the install directory) said one.
        major: Option<u32>,
        /// `CHROMEDRIVER_PATH` names this binary, so a download does not replace it.
        pinned: bool,
    },
}

/// The facts `--fix` decides from. Versions are major numbers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DoctorFindings {
    /// A Chrome or Chromium binary was found and the file exists.
    pub chrome_present: bool,
    /// The chromedriver lookup.
    pub chromedriver: ChromedriverFinding,
    /// Chrome's major version, when a binary was found and its version was read.
    pub chrome_major: Option<u32>,
    /// Whether the patch-cache directory already exists.
    pub patch_cache_exists: bool,
}

/// The repairs `--fix` will try.
///
/// A browser is never installed and Chrome is never patched. `install_chromedriver` is the
/// only download, and it is the one `sbrs install chromedriver` runs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DoctorFixPlan {
    /// Call `sb_install::install` for chromedriver, with no version.
    pub install_chromedriver: bool,
    /// Create the patch-cache directory. The patched binary is not written.
    pub create_patch_cache: bool,
    /// Chrome is missing. `--fix` says so and leaves that failure in the list.
    pub chrome_unfixable: bool,
    /// A download would not become the driver the check uses (`CHROMEDRIVER_PATH` wins).
    pub installed_driver_unused: bool,
}

/// Lines to print before the failing summary, and the problems still outstanding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DoctorFixReport {
    /// Stdout lines, in order. A repair that worked starts with `Fixed:`.
    pub lines: Vec<String>,
    /// Problems that remain. A repaired problem is gone; one that could not be repaired stays.
    pub failures: Vec<String>,
}

/// The folder the check calls the patch cache: `SB_PATCH_CACHE_DIR`, else `sb-chrome-patches`
/// under the platform cache directory (the same default as `sbrs patch-chrome`).
#[must_use]
pub(crate) fn patch_cache_directory(configured: Option<PathBuf>) -> PathBuf {
    configured.unwrap_or_else(|| {
        dirs::cache_dir()
            .unwrap_or_default()
            .join("sb-chrome-patches")
    })
}

/// What `--fix` will try for `found`.
///
/// A missing chromedriver, and a major version that is not Chrome's, ask for one install.
/// A driver whose version could not be read does not. A missing patch-cache directory is
/// created. A missing browser is reported and left in the failure list.
#[must_use]
pub(crate) fn plan_fixes(found: &DoctorFindings) -> DoctorFixPlan {
    let install_chromedriver = match found.chromedriver {
        ChromedriverFinding::Missing { .. } => true,
        ChromedriverFinding::Found { major, .. } => {
            matches!(
                (major, found.chrome_major),
                (Some(driver), Some(chrome)) if driver != chrome
            )
        }
    };
    DoctorFixPlan {
        install_chromedriver,
        create_patch_cache: !found.patch_cache_exists,
        chrome_unfixable: !found.chrome_present,
        installed_driver_unused: download_is_not_the_driver_used(found.chromedriver),
    }
}

/// Whether a download into `downloaded_drivers/` would not be the driver the check uses.
fn download_is_not_the_driver_used(driver: ChromedriverFinding) -> bool {
    match driver {
        ChromedriverFinding::Missing { configured } => configured,
        ChromedriverFinding::Found { pinned, .. } => pinned,
    }
}

/// Applies `plan`.
///
/// `install` is called once when the plan installs a chromedriver, and not at all otherwise,
/// so a test can count the calls without downloading. `failures` are the problems the check
/// already recorded. A repair that fails stays in [`DoctorFixReport::failures`].
/// Creating the directory and installing the driver are independent.
pub(crate) async fn apply_fixes<Install, Fut>(
    plan: &DoctorFixPlan,
    cache_dir: &Path,
    mut failures: Vec<String>,
    install: Install,
) -> DoctorFixReport
where
    Install: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<PathBuf, String>>,
{
    let mut lines = Vec::new();
    if plan.chrome_unfixable {
        lines.push(CANNOT_INSTALL_BROWSER.to_owned());
    }
    if plan.create_patch_cache {
        match std::fs::create_dir_all(cache_dir) {
            Ok(()) => lines.push(format!(
                "Fixed: created the patch cache directory {}",
                cache_dir.display()
            )),
            Err(error) => failures.push(format!(
                "could not create the patch cache directory {}: {error}",
                cache_dir.display()
            )),
        }
    }
    if plan.install_chromedriver {
        match install().await {
            Ok(path) if plan.installed_driver_unused => {
                lines.push(format!(
                    "Installed chromedriver at {}; CHROMEDRIVER_PATH still decides which driver \
                     is used",
                    path.display()
                ));
                let had_mismatch = failures.iter().any(|problem| problem.contains(MISMATCH_MARK));
                if !had_mismatch {
                    failures.push(
                        "CHROMEDRIVER_PATH names no usable chromedriver; --fix installed one in \
                         downloaded_drivers/ but does not change that variable"
                            .to_owned(),
                    );
                }
            }
            Ok(path) => {
                lines.push(format!("Fixed: installed chromedriver at {}", path.display()));
                failures.retain(|problem| !problem.contains(MISMATCH_MARK));
            }
            Err(error) => {
                failures.push(format!("could not install chromedriver: {error}"));
            }
        }
    }
    DoctorFixReport { lines, failures }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    fn found(major: Option<u32>, pinned: bool) -> ChromedriverFinding {
        ChromedriverFinding::Found { major, pinned }
    }

    fn installer(
        calls: &AtomicUsize,
    ) -> impl Fn() -> std::future::Ready<Result<PathBuf, String>> + '_ {
        move || {
            calls.fetch_add(1, Ordering::Relaxed);
            std::future::ready(Ok(PathBuf::from("downloaded_drivers/chromedriver")))
        }
    }

    #[test]
    fn the_patch_cache_directory_is_the_configured_one_or_the_platform_default() {
        assert_eq!(
            patch_cache_directory(Some(PathBuf::from("from-the-variable"))),
            PathBuf::from("from-the-variable")
        );
        assert!(patch_cache_directory(None).ends_with("sb-chrome-patches"));
    }

    #[test]
    fn a_missing_driver_is_installed_and_an_unread_version_is_not_a_mismatch() {
        let missing = plan_fixes(&DoctorFindings {
            chrome_present: true,
            chromedriver: ChromedriverFinding::Missing { configured: false },
            chrome_major: Some(130),
            patch_cache_exists: true,
        });
        assert!(missing.install_chromedriver);
        assert!(!missing.installed_driver_unused);
        assert!(!missing.chrome_unfixable);

        let configured = plan_fixes(&DoctorFindings {
            chrome_present: true,
            chromedriver: ChromedriverFinding::Missing { configured: true },
            chrome_major: None,
            patch_cache_exists: true,
        });
        assert!(configured.install_chromedriver);
        assert!(configured.installed_driver_unused);

        let unread = plan_fixes(&DoctorFindings {
            chrome_present: true,
            chromedriver: found(None, false),
            chrome_major: Some(130),
            patch_cache_exists: false,
        });
        assert!(!unread.install_chromedriver);
        assert!(unread.create_patch_cache);
    }

    #[tokio::test]
    async fn a_version_mismatch_calls_the_installer_once_and_a_match_does_not() {
        let calls = AtomicUsize::new(0);
        let mismatch = plan_fixes(&DoctorFindings {
            chrome_present: true,
            chromedriver: found(Some(129), false),
            chrome_major: Some(130),
            patch_cache_exists: true,
        });
        let problem = "chromedriver 129 cannot drive Chrome 130: run `sbrs install chromedriver`";
        let report = apply_fixes(
            &mismatch,
            Path::new("cache-not-created"),
            vec![problem.to_owned()],
            installer(&calls),
        )
        .await;

        assert_eq!(calls.load(Ordering::Relaxed), 1);
        assert!(report.failures.is_empty(), "{:?}", report.failures);
        assert_eq!(
            report.lines,
            vec!["Fixed: installed chromedriver at downloaded_drivers/chromedriver"]
        );

        let calls = AtomicUsize::new(0);
        let matching = plan_fixes(&DoctorFindings {
            chrome_present: true,
            chromedriver: found(Some(130), false),
            chrome_major: Some(130),
            patch_cache_exists: true,
        });
        let report = apply_fixes(
            &matching,
            Path::new("cache-not-created"),
            Vec::new(),
            installer(&calls),
        )
        .await;

        assert_eq!(calls.load(Ordering::Relaxed), 0);
        assert!(report.lines.is_empty());
        assert!(report.failures.is_empty());
    }

    #[tokio::test]
    async fn a_pinned_mismatch_is_installed_once_and_stays_a_failure() {
        let calls = AtomicUsize::new(0);
        let plan = plan_fixes(&DoctorFindings {
            chrome_present: true,
            chromedriver: found(Some(129), true),
            chrome_major: Some(130),
            patch_cache_exists: true,
        });
        assert!(plan.install_chromedriver);
        assert!(plan.installed_driver_unused);
        let problem = "chromedriver 129 cannot drive Chrome 130: still pinned".to_owned();
        let report = apply_fixes(
            &plan,
            Path::new("cache-not-created"),
            vec![problem.clone()],
            installer(&calls),
        )
        .await;

        assert_eq!(calls.load(Ordering::Relaxed), 1);
        assert_eq!(report.failures, vec![problem]);
        assert!(
            report
                .lines
                .iter()
                .any(|line| line.contains("CHROMEDRIVER_PATH")),
            "{:?}",
            report.lines
        );
        assert!(report.lines.iter().all(|line| !line.starts_with("Fixed:")));
    }

    #[tokio::test]
    async fn a_missing_browser_is_not_installed_and_stays_a_failure() {
        let calls = AtomicUsize::new(0);
        let plan = plan_fixes(&DoctorFindings {
            chrome_present: false,
            chromedriver: found(Some(130), false),
            chrome_major: None,
            patch_cache_exists: true,
        });
        assert!(plan.chrome_unfixable);
        assert!(!plan.install_chromedriver);
        let problem = "no Chrome or Chromium was found: install Google Chrome".to_owned();
        let report = apply_fixes(
            &plan,
            Path::new("cache-not-created"),
            vec![problem.clone()],
            installer(&calls),
        )
        .await;

        assert_eq!(calls.load(Ordering::Relaxed), 0);
        assert_eq!(report.failures, vec![problem]);
        assert_eq!(report.lines, vec![CANNOT_INSTALL_BROWSER.to_owned()]);
    }

    #[tokio::test]
    async fn a_failed_install_leaves_the_mismatch_and_records_the_error() {
        let plan = plan_fixes(&DoctorFindings {
            chrome_present: true,
            chromedriver: found(Some(129), false),
            chrome_major: Some(130),
            patch_cache_exists: true,
        });
        let problem = "chromedriver 129 cannot drive Chrome 130: fetch the matching one";
        let report = apply_fixes(&plan, Path::new("unused"), vec![problem.to_owned()], || {
            std::future::ready(Err("network down".to_owned()))
        })
        .await;

        assert!(report.failures.iter().any(|failure| failure == problem));
        assert!(
            report
                .failures
                .iter()
                .any(|failure| failure.contains("network down"))
        );
    }

    #[tokio::test]
    async fn a_missing_patch_cache_directory_is_created_and_an_existing_one_is_not() {
        let root = tempfile::tempdir().expect("temp dir");
        let missing = root.path().join("patches");
        let plan = plan_fixes(&DoctorFindings {
            chrome_present: true,
            chromedriver: found(Some(130), false),
            chrome_major: Some(130),
            patch_cache_exists: false,
        });
        assert!(plan.create_patch_cache);
        assert!(!plan.install_chromedriver);
        let report = apply_fixes(&plan, &missing, Vec::new(), || {
            std::future::ready(Err("should not install".to_owned()))
        })
        .await;

        assert!(missing.is_dir());
        assert_eq!(
            report.lines,
            vec![format!(
                "Fixed: created the patch cache directory {}",
                missing.display()
            )]
        );
        assert!(report.failures.is_empty(), "{:?}", report.failures);

        let existing = root.path().join("already");
        std::fs::create_dir(&existing).expect("cache dir");
        let plan = plan_fixes(&DoctorFindings {
            chrome_present: true,
            chromedriver: found(Some(130), false),
            chrome_major: Some(130),
            patch_cache_exists: existing.is_dir(),
        });
        assert!(!plan.create_patch_cache);
    }
}
