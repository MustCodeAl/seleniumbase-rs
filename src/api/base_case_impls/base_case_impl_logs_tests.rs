// Tests of where the `*_to_logs` helpers write: the folder of the test that is
// running (`latest_logs/<test id>` while a recorder records it), found on the
// calling thread or, for a test recorded on another thread such as a Gherkin
// scenario, by the test's id. They run against the mock WebDriver in
// `webdriver_support` (no browser needed). Included (for tests only) at the end
// of `base_case.rs` like the files beside it.

mod tests_to_logs {
    use super::webdriver_support::{MockWebDriver, Reply};
    use super::*;
    use crate::artifacts::test_support::Active;
    use serde_json::{json, Value};

    /// A page that answers a screenshot, its source and a PDF.
    fn page() -> MockWebDriver {
        MockWebDriver::start(json!({}), |request| {
            match (request.method.as_str(), request.path.as_str()) {
                // The signature of a PNG file, in base64.
                ("GET", "/screenshot") => Reply::Value(json!("iVBORw0KGgo=")),
                ("GET", "/source") => Reply::Value(json!("<html><body>hi</body></html>")),
                // "%PDF-" in base64.
                ("POST", "/goog/cdp/execute") => Reply::Value(json!({ "data": "JVBERi0=" })),
                _ => Reply::Value(Value::Null),
            }
        })
    }

    /// The names of the entries of `dir`.
    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .map(|entries| {
                entries
                    .flatten()
                    .map(|entry| entry.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }

    /// Runs every helper that saves "to the logs" and returns the files of the
    /// ones that say where they wrote. The two data helpers write a log file
    /// whose name is the time, so a test looks for it in the folder.
    async fn run_every_helper(sb: &BaseCase) -> Vec<PathBuf> {
        let (json, rust) = sb.save_recording_to_logs().unwrap();
        sb.save_data_to_logs("data").unwrap();
        sb.append_data_to_logs("more data").unwrap();
        vec![
            sb.save_screenshot_to_logs().await.unwrap(),
            sb.save_page_source_to_logs().await.unwrap(),
            sb.save_as_html_to_logs("saved.html").await.unwrap(),
            // The PDF has a test of its own: it needs a CDP session.
            sb.save_teardown_screenshot().await.unwrap(),
            json,
            rust,
        ]
    }

    fn has_a_data_log(folder: &Path) -> bool {
        names(folder)
            .iter()
            .any(|name| name.starts_with("sb_") && name.ends_with(".log"))
    }

    #[tokio::test]
    async fn the_helpers_write_into_the_folder_of_the_running_test() {
        let root = tempfile::tempdir().unwrap();
        let folder = root.path().join("running");
        let _recorded = Active::here(&folder);
        let driver = page();
        let sb = driver.base_case().await;

        let written = run_every_helper(&sb).await;

        for path in &written {
            assert_eq!(path.parent(), Some(folder.as_path()), "{path:?}");
            assert!(path.is_file(), "{path:?} was not written");
        }
        assert!(has_a_data_log(&folder), "{:?}", names(&folder));
        assert_eq!(
            names(root.path()),
            ["running"],
            "nothing was written next to the test's folder"
        );
    }

    #[tokio::test]
    async fn a_test_recorded_on_another_thread_still_gets_its_files() {
        // A scenario's recorder runs on the scenario's own thread, and the
        // steps, which call the helpers, run on this one.
        let root = tempfile::tempdir().unwrap();
        let driver = page();
        let sb = driver.base_case().await;
        let folder = root.path().join(sb.test_id());
        let _recorded = Active::elsewhere(&folder);

        let written = run_every_helper(&sb).await;

        for path in &written {
            assert_eq!(path.parent(), Some(folder.as_path()), "{path:?}");
            assert!(path.is_file(), "{path:?} was not written");
        }
        assert!(has_a_data_log(&folder), "{:?}", names(&folder));
    }

    #[tokio::test]
    async fn the_name_a_case_was_given_finds_the_folder_of_that_test() {
        let root = tempfile::tempdir().unwrap();
        let named = root.path().join("shop.checkout");
        let own = root.path().join("another.test");
        let _named = Active::elsewhere(&named);
        // This thread records a different test; the case belongs to the named one.
        let _own = Active::here(&own);
        let driver = page();
        let sb = driver.base_case().await.with_test_name("shop.checkout");

        let path = sb.save_screenshot_to_logs().await.unwrap();

        assert_eq!(path.parent(), Some(named.as_path()));
        assert!(!own.exists(), "the other test's folder was not used");
    }

    #[tokio::test]
    async fn a_case_that_is_not_recorded_uses_the_folder_of_the_test_on_its_thread() {
        let root = tempfile::tempdir().unwrap();
        let own = root.path().join("recorded.here");
        let _own = Active::here(&own);
        let driver = page();
        let sb = driver.base_case().await.with_test_name("no.recorder.has.this.id");

        let path = sb.save_page_source_to_logs().await.unwrap();

        assert_eq!(path.parent(), Some(own.as_path()));
    }

    #[tokio::test]
    async fn a_pdf_is_saved_in_the_folder_of_the_running_test() {
        // Printing a page needs the DevTools channel of a CDP session.
        let root = tempfile::tempdir().unwrap();
        let folder = root.path().join("running");
        let _recorded = Active::here(&folder);
        let driver = page();
        let mut config = BrowserConfig::default()
            .with_webdriver_url(driver.url().to_owned())
            .with_mode(crate::browser::config::DriverMode::Cdp);
        config.auto_start_driver = false;
        let sb = BaseCase::new(config).await.unwrap();

        let path = sb.save_as_pdf_to_logs("saved.pdf").await.unwrap();

        assert_eq!(path, folder.join("saved.pdf"));
        assert_eq!(std::fs::read(&path).unwrap(), b"%PDF-");
    }

    #[tokio::test]

    async fn the_helpers_named_after_a_file_still_write_to_the_logs_folder_itself() {
        // `save_screenshot("x.png")` and the other helpers that take a file name
        // are not "to the logs": they keep writing beside the test folders, as
        // they did before the helpers above moved into one.
        let root = tempfile::tempdir().unwrap();
        let folder = root.path().join("running");
        let _recorded = Active::here(&folder);
        let driver = page();
        let sb = driver.base_case().await;

        let path = sb.create_folder("named_in_logs").unwrap();

        assert_ne!(path.parent(), Some(folder.as_path()), "{path:?}");
        assert!(path.parent().is_some_and(|dir| dir.ends_with("latest_logs")));
        std::fs::remove_dir(&path).unwrap();
    }
}
