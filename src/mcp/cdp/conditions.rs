//! Waiting for a condition, and asserting one.

use super::{assert_condition, timeout_prop, wait_for_condition, Cdp};
use crate::mcp::{Effect, Prop, Schema, ToolDef};

pub(super) fn tools() -> Vec<ToolDef<Cdp>> {
    vec![
        ToolDef::new(
            "wait_for_condition",
            "Wait For Condition",
            "Block until an element is present, visible, not visible or absent, until text \
             appears or disappears, or for a fixed number of seconds. Fails if the timeout \
             passes first.",
            Effect::Inspect,
            Schema::new()
                .optional(
                    "state",
                    Prop::choice(
                        &[
                            "present",
                            "visible",
                            "not_visible",
                            "absent",
                            "seconds_passed",
                        ],
                        "The condition to wait for",
                    )
                    .default("visible"),
                )
                .optional("selector", Prop::string("Element to watch"))
                .optional(
                    "text",
                    Prop::string(
                        "Wait for this text instead; present/visible wait for it to \
                                  appear, absent/not_visible for it to go",
                    ),
                )
                .optional(
                    "timeout",
                    Prop::number("Most seconds to wait; the exact wait for seconds_passed")
                        .min(0.0)
                        .default(5),
                ),
            wait_for_condition,
        ),
        ToolDef::new(
            "assert_condition",
            "Assert Condition",
            "Verify that an element is present or visible, that text is visible, or that the \
             title or URL matches. A failed check is an error. Element and text checks wait \
             up to the timeout; title and URL checks are immediate.",
            Effect::Inspect,
            Schema::new()
                .optional(
                    "check",
                    Prop::choice(
                        &[
                            "element_present",
                            "element_visible",
                            "text_visible",
                            "title",
                            "url",
                            "url_contains",
                        ],
                        "What to verify",
                    )
                    .default("element_visible"),
                )
                .optional(
                    "selector",
                    Prop::string("Element for element and text checks"),
                )
                .optional("expected", Prop::string("Expected text, title or URL"))
                .optional(
                    "exact",
                    Prop::boolean("For text_visible, require the whole text to match")
                        .default(false),
                )
                .optional("timeout", timeout_prop(5.0)),
            assert_condition,
        ),
    ]
}
