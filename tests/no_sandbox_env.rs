//! `SB_NO_SANDBOX` is fail-safe: only `true`, `1`, `yes` or `on` (in any letter case) turn
//! Chrome's sandbox off; `false`, `0`, `no`, `off` and an empty value keep it on; any other
//! value is an error that names the variable (`docs/design/settings_v2.md`, finding 6).
//!
//! Before the change the builder turned the sandbox off whenever the variable existed, so
//! `SB_NO_SANDBOX=0` and `SB_NO_SANDBOX=false` did the opposite of what they say.
//!
//! This file is its own test binary, with one test, because it changes the process
//! environment: no other test runs beside it.

use std::path::Path;

use seleniumbase_rs::sb_cdp::{LaunchOptions, LaunchOptionsBuilder};

/// Whether the browser would start without its sandbox, or the builder's error.
fn sandbox_off(builder: LaunchOptionsBuilder) -> Result<bool, String> {
    builder
        .build()
        .map(|options| {
            options
                .browser_args(Path::new("/tmp/sbrs-no-sandbox-profile"))
                .iter()
                .any(|arg| arg == "--no-sandbox")
        })
        .map_err(|error| error.to_string())
}

/// With `SB_NO_SANDBOX` set to `value` (or removed), what the builder decides.
fn with_variable(value: Option<&str>, builder: LaunchOptionsBuilder) -> Result<bool, String> {
    // SAFETY: this binary has one test, and nothing else reads the environment meanwhile.
    unsafe {
        match value {
            Some(value) => std::env::set_var("SB_NO_SANDBOX", value),
            None => std::env::remove_var("SB_NO_SANDBOX"),
        }
    }
    sandbox_off(builder)
}

#[test]
fn only_a_true_value_turns_the_sandbox_off() {
    for on in ["true", "1", "yes", "on", "TRUE", "Yes", "ON"] {
        assert_eq!(
            with_variable(Some(on), LaunchOptions::builder()),
            Ok(true),
            "SB_NO_SANDBOX={on}"
        );
    }
    // The variable that says no, and the one that says nothing, keep the sandbox on.
    for off in ["false", "0", "no", "off", "FALSE", "Off", "", "  "] {
        assert_eq!(
            with_variable(Some(off), LaunchOptions::builder()),
            Ok(false),
            "SB_NO_SANDBOX={off:?}"
        );
    }
    assert_eq!(with_variable(None, LaunchOptions::builder()), Ok(false));

    // Any other value is an error that names the variable, not a guess in either direction.
    for wrong in ["maybe", "2", "enable"] {
        let error = with_variable(Some(wrong), LaunchOptions::builder())
            .expect_err("a value that is no boolean is refused");
        assert!(error.contains("SB_NO_SANDBOX"), "{wrong}: {error}");
        assert!(error.contains(wrong), "{wrong}: {error}");
    }

    // The builder's own setting beats the variable either way, and then the variable is not read.
    assert_eq!(
        with_variable(Some("true"), LaunchOptions::builder().no_sandbox(false)),
        Ok(false)
    );
    assert_eq!(
        with_variable(Some("0"), LaunchOptions::builder().no_sandbox(true)),
        Ok(true)
    );
    assert_eq!(
        with_variable(Some("maybe"), LaunchOptions::builder().no_sandbox(true)),
        Ok(true)
    );
}
