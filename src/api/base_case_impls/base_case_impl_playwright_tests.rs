// Tests of how `activate_playwright_mode` configures the second browser. The
// launch itself needs a real Chromium: see `tests/playwright_chrome.rs`.
// Included (for tests only, with the `playwright` feature) at the end of
// `base_case.rs` like the files beside it.

mod tests_playwright_config {
    use super::*;

    #[test]
    fn the_playwright_browser_gets_the_case_s_config_but_not_its_profile_folder() {
        let sb = BaseCase::without_session(
            BrowserConfig::default()
                .with_headless(true)
                .with_user_agent("Agent/1.0")
                .with_locale("de-DE")
                .with_user_data_dir("/tmp/profile"),
        );

        let config = sb.playwright_config();

        // Chrome locks a profile folder, and the WebDriver browser holds it.
        assert_eq!(config.user_data_dir, None);
        assert!(config.headless);
        assert_eq!(config.user_agent.as_deref(), Some("Agent/1.0"));
        assert_eq!(config.locale.as_deref(), Some("de-DE"));
        // The case keeps its own configuration.
        assert_eq!(sb.config.user_data_dir.as_deref(), Some("/tmp/profile"));
    }
}
