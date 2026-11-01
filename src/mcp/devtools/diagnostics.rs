//! What the page reports about itself: console messages, failed requests and
//! an accessibility audit.

use super::{audit_page, get_console_messages, get_failed_requests, DevSession};
use crate::mcp::{Effect, Prop, Schema, ToolDef};

pub(super) fn tools<S: DevSession>() -> Vec<ToolDef<S>> {
    vec![
        ToolDef::new(
            "get_console_messages",
            "Get Console Messages",
            "Read the page's console messages and uncaught script errors. Messages are recorded \
             from the moment start is true, which replaces the page's console functions (a page \
             script can notice that); the Pure CDP engine also reports uncaught errors since the \
             page loaded without it.",
            Effect::Mixed,
            Schema::new()
                .optional(
                    "level",
                    Prop::choice(
                        &["all", "error", "warning", "info", "log"],
                        "The least serious level to include",
                    )
                    .default("all"),
                )
                .optional(
                    "start",
                    Prop::boolean("Start recording console messages in this page").default(false),
                )
                .optional(
                    "clear",
                    Prop::boolean("Forget the messages after reading them").default(false),
                )
                .optional(
                    "limit",
                    Prop::integer("Most messages to return, 1 to 500")
                        .min(1.0)
                        .default(100),
                ),
            get_console_messages::<S>,
        )
        .untrusted(),
        ToolDef::new(
            "get_failed_requests",
            "Get Failed Requests",
            "List the page's requests that failed: HTTP status 400 and up from the browser's \
             performance timeline, and, once watch is true on the Pure CDP engine, every failed, \
             blocked or refused request of the active tab with its error.",
            Effect::Mixed,
            Schema::new()
                .optional(
                    "watch",
                    Prop::boolean("Start recording failed requests (Pure CDP engine)")
                        .default(false),
                )
                .optional(
                    "clear",
                    Prop::boolean("Forget the recorded failures after reading them").default(false),
                )
                .optional(
                    "limit",
                    Prop::integer("Most requests to return, 1 to 500")
                        .min(1.0)
                        .default(100),
                ),
            get_failed_requests::<S>,
        ),
        ToolDef::new(
            "audit_page",
            "Audit Page",
            "Check the page as it is now for accessibility and HTML defects: images without alt \
             text, controls without a label or name, duplicate ids, empty links and buttons, \
             heading order, missing lang or title, and more. Each finding names its rule and a \
             selector for the element.",
            Effect::Observe,
            Schema::new()
                .optional(
                    "severity",
                    Prop::choice(&["all", "error"], "Report warnings too, or errors only")
                        .default("all"),
                )
                .optional(
                    "limit",
                    Prop::integer("Most findings to return, 1 to 500")
                        .min(1.0)
                        .default(50),
                ),
            audit_page::<S>,
        ),
    ]
}
