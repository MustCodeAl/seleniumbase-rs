//! Writing what a session found to the output directory: a debug bundle and the
//! session as a Rust test.

use super::{capture_debug_bundle, export_session_as_test, DevSession};
use crate::mcp::{Effect, Prop, Schema, ToolDef};

pub(super) fn tools<S: DevSession>() -> Vec<ToolDef<S>> {
    vec![
        ToolDef::new(
            "capture_debug_bundle",
            "Capture Debug Bundle",
            "Save what a bug report needs in a new folder under the output directory: a \
             screenshot, the page source (password and hidden input values removed), the URL, \
             title, viewport and user agent, console messages, failed requests, cookie and \
             storage names (never their values) and the last tool calls (typed text and scripts \
             redacted).",
            Effect::Overwrite,
            Schema::new()
                .optional(
                    "name",
                    Prop::string("A label for the folder; letters, digits, - and _ are kept"),
                )
                .optional(
                    "calls",
                    Prop::integer("How many recent tool calls to include, 1 to 200")
                        .min(1.0)
                        .default(20),
                ),
            capture_debug_bundle::<S>,
        ),
        ToolDef::new(
            "export_session_as_test",
            "Export Session As Test",
            "Write the successful steps of this browser session (since the last start_browser) \
             as a Rust #[tokio::test] using BaseCase, into the output directory. Steps with no \
             BaseCase equivalent become // skipped: comments and are listed in the result.",
            Effect::Overwrite,
            Schema::new().optional(
                "test_name",
                Prop::string("Name of the test function and file, such as checkout_works")
                    .default("exported_session"),
            ),
            export_session_as_test::<S>,
        ),
    ]
}
