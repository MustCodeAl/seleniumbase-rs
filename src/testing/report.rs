//! Reports for `cargo test`: what the fixtures hand to the run of the process.
//!
//! When the settings turn a report on (`--dashboard`, `--html`, `--junit-xml`,
//! `--alluredir`, `--with-db-reporting`, `--archive-logs`; the `[test]` and
//! `[report]` tables of the settings file and the `SB_*` variables), the `sb`
//! and `sb_cdp` fixtures and the Gherkin ones record every test in the
//! [`TestRun`] of the process, which every test binary of one `cargo test`
//! shares:
//!
//! - the test is **started** in the run before its browser launches, so a live
//!   `dashboard.html` shows it as untested while it runs;
//! - the **last attempt** ends it: a pass, a skip, or a failure with no rerun
//!   left. An attempt that failed and will be run again records nothing, and a
//!   browser that did not start is a failure;
//! - every end rewrites the outputs that are on (the dashboard, the HTML
//!   report, the JUnit XML), so they are complete after each test.
//!
//! The evidence of a failure (`latest_logs/<test id>/basic_test_info.txt`,
//! `screenshot.png`, `page_source.html`) is the lifecycle's; the run only links
//! it.
//!
//! # Who ends the run
//!
//! libtest has no hook for the end of a session, so a plain `cargo test` leaves
//! the dashboard **live**: it refreshes every twelve seconds, and a test that
//! started and never ended stays untested. [`TestRun::finish`] ends it (the
//! outputs are written a last time, the logs are archived when asked, and the
//! results are imported into the results database when `with_db_reporting` is
//! on); `sbrs test` calls it after cargo, and `sbrs report finish` calls it for
//! a run that was left live.
//!
//! # S3
//!
//! With `--with-s3-logging` (feature `s3`), the folder of a test's logs is
//! uploaded when its last attempt ends, to the bucket named by `SB_S3_BUCKET`
//! (region `SB_S3_REGION`, else `AWS_REGION`, else `us-east-1`; keys under
//! `SB_S3_PREFIX`). The credentials are the plugin's: `AWS_ACCESS_KEY_ID`,
//! `AWS_SECRET_ACCESS_KEY` and optionally `AWS_ENDPOINT_URL`.

use std::path::Path;
use std::sync::Arc;

use super::lifecycle::Finalizer;
use crate::config::settings::Settings;
use crate::core::log_helper::TestId;
use crate::core::test_run::{ReportSettings, RunFinalizer, TestRun};

/// Which run a test is recorded in.
#[derive(Clone, Debug, Default)]
pub(crate) enum Reporting {
    /// The run of the process, when the settings turn reports on.
    #[default]
    Auto,
    /// No run, whatever the settings say.
    Off,
    /// This run.
    Run(Arc<TestRun>),
}

/// Whether `settings` ask for anything a run does.
pub(crate) fn wants_a_run(settings: &ReportSettings) -> bool {
    settings.dashboard
        || settings.html.is_some()
        || settings.junit_xml.is_some()
        || settings.allure_dir.is_some()
        || settings.with_db_reporting
        || settings.archive_logs
}

/// The run a test is recorded in, if any. A run of the process that cannot be
/// made is logged, and the test runs without it.
pub(crate) fn run_for(reporting: &Reporting, settings: &Settings) -> Option<Arc<TestRun>> {
    match reporting {
        Reporting::Off => None,
        Reporting::Run(run) => Some(Arc::clone(run)),
        Reporting::Auto => {
            if !wants_a_run(&ReportSettings::from_settings(settings)) {
                return None;
            }
            match TestRun::current() {
                Ok(run) => Some(run),
                Err(error) => {
                    tracing::warn!(%error, "the test run cannot be reported; the test runs without it");
                    None
                }
            }
        }
    }
}

/// The finalizers of a test: its record in `run` (when there is one), then the
/// upload of its logs to S3 (when asked). `logs_dir` is where the lifecycle
/// writes the test's evidence.
pub(crate) fn finalizers(
    run: Option<&Arc<TestRun>>,
    settings: &Settings,
    test: &TestId,
    logs_dir: &Path,
) -> Vec<Arc<dyn Finalizer>> {
    let mut finalizers: Vec<Arc<dyn Finalizer>> = Vec::new();
    if let Some(run) = run {
        finalizers.push(Arc::new(RunFinalizer::new(Arc::clone(run), test.clone())));
    }
    if settings.test.with_s3_logging {
        finalizers.extend(s3::finalizer(&logs_dir.join(&test.id), &test.id));
    }
    finalizers
}

#[cfg(not(feature = "s3"))]
mod s3 {
    use std::path::Path;
    use std::sync::{Arc, Once};

    use crate::testing::Finalizer;

    pub(super) fn finalizer(_folder: &Path, _id: &str) -> Option<Arc<dyn Finalizer>> {
        static WARNED: Once = Once::new();
        WARNED.call_once(|| {
            tracing::warn!(
                "SB_WITH_S3_LOGGING is on, but seleniumbase-rs was built without the \"s3\" feature; \
                 add features = [\"s3\"] to its dependency"
            );
        });
        None
    }
}

#[cfg(feature = "s3")]
mod s3 {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use async_trait::async_trait;

    use crate::common::debug_prompt::PromptPage;
    use crate::plugins::s3_logging_plugin::S3LoggingPlugin;
    use crate::testing::{AttemptEnd, Finalizer, Outcome};

    /// Uploads the folder of a test's logs when its last attempt ends.
    struct Upload {
        bucket: String,
        region: String,
        prefix: String,
        folder: PathBuf,
        id: String,
    }

    pub(super) fn finalizer(folder: &Path, id: &str) -> Option<Arc<dyn Finalizer>> {
        let var = |name: &str| {
            std::env::var(name)
                .ok()
                .filter(|value| !value.trim().is_empty())
        };
        let Some(bucket) = var("SB_S3_BUCKET") else {
            tracing::warn!("SB_WITH_S3_LOGGING is on, but SB_S3_BUCKET names no bucket");
            return None;
        };
        let region = var("SB_S3_REGION")
            .or_else(|| var("AWS_REGION"))
            .unwrap_or_else(|| "us-east-1".to_owned());
        Some(Arc::new(Upload {
            bucket,
            region,
            prefix: normalized(&var("SB_S3_PREFIX").unwrap_or_default()),
            folder: folder.to_path_buf(),
            id: id.to_owned(),
        }))
    }

    /// `logs` as `logs/`; the empty prefix stays empty.
    fn normalized(prefix: &str) -> String {
        let prefix = prefix.trim().trim_start_matches('/');
        if prefix.is_empty() || prefix.ends_with('/') {
            prefix.to_owned()
        } else {
            format!("{prefix}/")
        }
    }

    fn key(prefix: &str, id: &str, name: &str) -> String {
        format!("{prefix}{id}/{name}")
    }

    /// Whether an attempt that ended this way is the test's result.
    fn is_last(end: &AttemptEnd<'_>) -> bool {
        !matches!(end.outcome, Outcome::Failed(_)) || end.attempt >= end.attempts
    }

    impl Upload {
        async fn upload(&self) {
            // A test that passed leaves no folder.
            let Ok(entries) = std::fs::read_dir(&self.folder) else {
                return;
            };
            let mut names: Vec<String> = entries
                .filter_map(Result::ok)
                .filter(|entry| entry.path().is_file())
                .filter_map(|entry| entry.file_name().into_string().ok())
                .collect();
            names.sort();
            for name in names {
                let key = key(&self.prefix, &self.id, &name);
                let path = self.folder.join(&name);
                if let Err(error) =
                    S3LoggingPlugin::upload_file(&self.bucket, &key, &path, &self.region).await
                {
                    tracing::warn!(%error, %key, "a log was not uploaded");
                }
            }
        }
    }

    #[async_trait]
    impl Finalizer for Upload {
        async fn finish(&self, end: &AttemptEnd<'_>, _page: &dyn PromptPage) {
            if is_last(end) {
                self.upload().await;
            }
        }

        async fn launch_failed(&self, end: &AttemptEnd<'_>) {
            if is_last(end) {
                self.upload().await;
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn keys_sit_under_the_prefix_and_the_test_id() {
            assert_eq!(normalized(""), "");
            assert_eq!(normalized(" /ci/run-7 "), "ci/run-7/");
            assert_eq!(normalized("logs/"), "logs/");
            assert_eq!(
                key(&normalized("ci"), "login.valid", "screenshot.png"),
                "ci/login.valid/screenshot.png"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::test_settings::TestSettings;

    fn on(change: impl FnOnce(&mut ReportSettings)) -> ReportSettings {
        let mut settings = ReportSettings::default();
        change(&mut settings);
        settings
    }

    #[test]
    fn a_report_that_is_on_wants_a_run_and_nothing_else_does() {
        assert!(!wants_a_run(&ReportSettings::default()));
        assert!(wants_a_run(&on(|s| s.dashboard = true)));
        assert!(wants_a_run(&on(|s| s.html = Some("r.html".into()))));
        assert!(wants_a_run(&on(|s| s.junit_xml = Some("r.xml".into()))));
        assert!(wants_a_run(&on(|s| s.allure_dir = Some("allure".into()))));
        assert!(wants_a_run(&on(|s| s.with_db_reporting = true)));
        assert!(wants_a_run(&on(|s| s.archive_logs = true)));
        // These only change what a failure leaves behind.
        assert!(!wants_a_run(&on(|s| s.no_screenshot = true)));
        assert!(!wants_a_run(&on(|s| s.list_fail_pages = true)));
    }

    #[test]
    fn off_and_an_explicit_run_ignore_the_settings() {
        let dashboard = Settings {
            test: TestSettings {
                dashboard: true,
                ..TestSettings::default()
            },
            ..Settings::default()
        };
        assert!(run_for(&Reporting::Off, &dashboard).is_none());
        let root = tempfile::tempdir().unwrap();
        let run = TestRun::open(
            root.path(),
            ReportSettings::default(),
            crate::core::test_run::RunKey::new("report-unit"),
        )
        .unwrap();
        let chosen = run_for(&Reporting::Run(Arc::clone(&run)), &Settings::default()).unwrap();
        assert!(Arc::ptr_eq(&chosen, &run));
        let test = TestId::named("unit::test");
        assert_eq!(
            finalizers(Some(&run), &Settings::default(), &test, root.path()).len(),
            1
        );
        assert!(finalizers(None, &Settings::default(), &test, root.path()).is_empty());
    }

    #[test]
    fn auto_makes_no_run_when_no_report_is_on() {
        // No run is made, so no `latest_logs` is cleared.
        assert!(run_for(&Reporting::Auto, &Settings::default()).is_none());
    }
}
