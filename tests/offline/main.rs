//! Mock and offline integration tests, one binary.
//!
//! Each module is a former `tests/<name>.rs` test binary. The test functions
//! keep their names, `#[ignore]` and `cfg` attributes. Run one module with
//! `cargo test -p seleniumbase-rs --test offline <module>::`.

// One load of each shared file. Clippy rejects the same `#[path]` from two modules.
#[path = "../support/import_fixtures.rs"]
mod import_fixtures;

mod allure_results;
mod base_case_limits;
mod bdd_builtin_link;
mod bdd_builtin_steps;
mod bdd_scenarios;
mod confined_file_names;
mod dockerfile_skeleton;
mod docs_current;
mod driver_repair_macos;
mod fingerprint_companion;
mod fingerprint_randomized;
mod grid_server_ended;
mod install_addon_firefox;
mod library_manifest;
mod mcp_cdp;
mod mcp_client;
mod mcp_devtools;
mod mcp_email;
mod mcp_fingerprint;
mod mcp_host;
mod mcp_http;
mod mcp_pagetools;
mod mcp_tool_lists;
mod mcp_webdriver;
mod no_deprecated;
mod parity;
mod playwright_import;
mod plugin_features;
mod plugin_switch_policy;
mod plugin_tasks;
mod plugins;
mod puppeteer_import;
mod recorder_cdp_compiles;
mod recorder_pipeline;
mod recorder_rstest_compiles;
mod reporting_renderers;
mod reporting_run;
mod sb_cdp_attach;
mod sb_cdp_captcha_mock;
mod sb_cdp_capture;
mod sb_cdp_channel;
mod sb_cdp_domain_hygiene;
mod sb_cdp_download;
mod sb_cdp_extension_ids;
mod sb_cdp_frames;
mod sb_cdp_har;
mod sb_cdp_human;
mod sb_cdp_intercept;
mod sb_cdp_launch_layers;
mod sb_cdp_mobile;
mod sb_cdp_mock;
mod sb_cdp_page_inject;
mod sb_cdp_parity_mock;
mod sb_cdp_plugin_hooks;

mod sb_cdp_plugins;
mod sb_cdp_pool;
mod sb_cdp_proxy;
mod sb_fixture;
mod sb_fixture_plugins;
mod sb_reports;
mod sb_reports_env;
mod settings_layers;
mod settings_pins;

mod shutdown_pool;
mod stealth_audit_network;
mod test_options_table;
mod translate_vocabulary;
