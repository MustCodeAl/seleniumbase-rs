//! Real-Chrome command-line tests, one binary.
//!
//! Each module is a former `crates/cli/tests/<name>.rs` test binary. The tests
//! stay `#[ignore]`d and keep their names and `cfg` attributes. Run them with
//! `cargo test -p seleniumbase-rs-cli --test chrome -- --ignored --test-threads=1`.
//! One module: `cargo test -p seleniumbase-rs-cli --test chrome <module>::`.

// One load of each shared file. Clippy rejects the same `#[path]` from two modules.
#[path = "../support/mcp_http.rs"]
mod mcp_http_support;
#[path = "../support/sbrs.rs"]
mod sbrs;

mod cli_e2e_chrome;
mod cli_fingerprint_chrome;
mod cli_mkrec_chrome;
mod cli_plugins_chrome;
mod cli_solve_captcha_chrome;
mod devtools_cli_chrome;
mod mcp_client_chrome;
mod mcp_http_chrome;
mod mcp_stdio_chrome;
