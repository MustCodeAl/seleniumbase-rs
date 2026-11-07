//! Real-Chrome integration tests, one binary.
//!
//! Each module is a former `tests/<name>.rs` test binary. The tests stay
//! `#[ignore]`d and keep their names and `cfg` attributes. Run them with
//! `cargo test -p seleniumbase-rs --test chrome -- --ignored --test-threads=1`.
//! One module: `cargo test -p seleniumbase-rs --test chrome <module>::`.

// One load of each shared file. Clippy rejects the same `#[path]` from two modules.
#[path = "../support/captcha_site.rs"]
mod captcha_site;
#[path = "../support/import_fixtures.rs"]
mod import_fixtures;

mod ad_block_chrome;
mod base_case_capture_chrome;
mod basecase_logs_chrome;
mod bdd_builtin_chrome;
mod bdd_chrome;
mod browser_options_chrome;
mod captcha_basecase_chrome;
mod captcha_live_chrome;
mod cdp_page_chrome;
mod check_window_chrome;
mod client_hints_chrome;
mod crumbs_chrome;
mod debug_prompt_chrome;
mod debug_prompt_terminal_chrome;
mod demo_mode_chrome;
mod downloads_folder_chrome;
mod element_chrome;

mod esc_end_chrome;
mod examples_parity_chrome;
mod fingerprint_consistency_chrome;
mod form_fields_chrome;
mod frames_windows_alerts_chrome;
mod generate_referral_chrome;
mod identity_linking_chrome;
mod jqc_dialogs_chrome;
mod masterqa_chrome;
mod mcp_cdp_chrome;
mod mcp_devtools_chrome;
mod mcp_fingerprint_chrome;
mod mcp_pagetools_chrome;
mod mcp_webdriver_chrome;
mod messenger_chrome;
mod ocr_chrome;
mod offset_click_chrome;
mod parity_honesty_chrome;
mod parity_signatures_chrome;
mod password_prompt_chrome;
mod playwright_import_chrome;
mod plugin_tasks_chrome;
mod presentation_chart_chrome;
mod proxy_auth_chrome;
mod puppeteer_import_chrome;
mod recorder_chrome;
mod recorder_frames_chrome;
mod recorder_replay_chrome;
mod recorder_script_chrome;
mod reporting_chrome;
mod reuse_session_chrome;
mod run_settings_chrome;
mod sb_cdp_captcha_chrome;
mod sb_cdp_capture_chrome;
mod sb_cdp_chrome;
mod sb_cdp_download_chrome;
mod sb_cdp_frames_chrome;
mod sb_cdp_har_chrome;
mod sb_cdp_options_chrome;
mod sb_cdp_page_inject_chrome;
mod sb_cdp_parity_chrome;
mod sb_cdp_plugin_hooks_chrome;
mod sb_cdp_plugin_network_chrome;
mod sb_cdp_proxy_chrome;
mod sb_fixture_chrome;
mod sb_reports_chrome;
mod set_content_to_frame_chrome;
mod shadow_dom_chrome;
mod stealth_audit_chrome;
mod stealth_js_chrome;
mod stealth_providers_chrome;
mod tours_chrome;
mod visual_chrome;
mod webdriver_plugins_chrome;
