//! Mock and offline command-line tests, one binary.
//!
//! Each module is a former `crates/cli/tests/<name>.rs` test binary. The test
//! functions keep their names, `#[ignore]` and `cfg` attributes. Run one module
//! with `cargo test -p seleniumbase-rs-cli --test offline <module>::`.

// One load of each shared file. Clippy rejects the same `#[path]` from two modules.
#[path = "../support/fake_chrome.rs"]
mod fake_chrome;
#[path = "../support/fake_webdriver.rs"]
mod fake_webdriver;
#[path = "../support/mcp_http.rs"]
mod mcp_http_support;
#[path = "../support/sbrs.rs"]
mod sbrs;

mod cli_aliases;
mod cli_browser_options;
mod cli_commander;
mod cli_crypto;
mod cli_doctor;
mod cli_download;
mod cli_e2e;
mod cli_env;
mod cli_env_vars;
mod cli_failure_pages;
mod cli_fingerprint;
mod cli_generators;
mod cli_grid;
mod cli_info;
mod cli_install;
mod cli_logging;
mod cli_mkrec;
mod cli_objects;
mod cli_pdf_feature;
mod cli_plugins;
mod cli_prompt;
mod cli_proxy;
mod cli_recording;
mod cli_report;
mod cli_report_finish;
mod cli_scaffold;
mod cli_scripts;
mod cli_strict;
mod cli_terminal_apps;
mod cli_test_cmd;
mod cli_test_session;
mod cli_translate;
mod completion_shells;
mod completion_values;
mod mcp_client;
mod mcp_env;
mod mcp_http;
mod mcp_imap;
mod mcp_install;
mod mcp_plugins;
mod mcp_shutdown;
mod mcp_version;
