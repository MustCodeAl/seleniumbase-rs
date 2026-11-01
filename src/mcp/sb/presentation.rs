//! Showing where the session is working, and pausing.

use crate::api::base_case::BaseCase;
use crate::mcp::webdriver::{self as w, selector_prop, timeout_prop};
use crate::mcp::{Effect, Prop, Schema, ToolDef};

pub(super) fn tools() -> Vec<ToolDef<BaseCase>> {
    vec![
        ToolDef::new(
            "highlight",
            "Highlight",
            "Briefly outline an element, a few times, to show where it is.",
            Effect::Act,
            Schema::new()
                .required("selector", selector_prop())
                .optional("loops", Prop::integer("How many times to flash").default(4))
                .optional("timeout", timeout_prop(5.0)),
            w::highlight,
        ),
        ToolDef::new(
            "flash",
            "Flash",
            "Briefly flash an element.",
            Effect::Act,
            Schema::new()
                .required("selector", selector_prop())
                .optional(
                    "duration",
                    Prop::number("Roughly how many seconds to flash for")
                        .min(0.0)
                        .default(1),
                ),
            w::flash,
        ),
        ToolDef::new(
            "sleep",
            "Sleep",
            "Pause for a number of seconds.",
            Effect::Inspect,
            Schema::new().required("seconds", Prop::number("How long to pause").min(0.0)),
            w::sleep,
        ),
    ]
}
