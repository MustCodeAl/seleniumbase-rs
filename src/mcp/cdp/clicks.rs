//! Clicking, hovering and dragging.

use super::{click_element, hover_action, selector_prop, timeout_prop, Cdp};
use crate::mcp::{Effect, Prop, Schema, ToolDef};

pub(super) fn tools() -> Vec<ToolDef<Cdp>> {
    vec![
        ToolDef::new(
            "click_element",
            "Click Element",
            "Click an element. Use nth to pick one of several matches, all_matches to click \
             every visible one, only_if_visible to skip a missing element quietly, or \
             parent_selector to search inside a container.",
            Effect::Act,
            Schema::new()
                .required("selector", selector_prop())
                .optional(
                    "nth",
                    Prop::integer("Which match to click, counting from 1").min(1.0),
                )
                .optional(
                    "all_matches",
                    Prop::boolean("Click every visible match, in page order").default(false),
                )
                .optional(
                    "only_if_visible",
                    Prop::boolean("Click only if the element is already visible; never wait")
                        .default(false),
                )
                .optional(
                    "parent_selector",
                    Prop::string("Look for the selector inside this container"),
                )
                .optional("timeout", timeout_prop(5.0)),
            click_element,
        ),
        ToolDef::new(
            "hover_action",
            "Hover / Click / Drag",
            "Hover an element, hover one and then click another (for menus), or drag one \
             element onto another.",
            Effect::Mixed,
            Schema::new()
                .required("selector", selector_prop())
                .optional(
                    "secondary_selector",
                    Prop::string("The element to click after hovering, or to drop onto"),
                )
                .optional(
                    "action",
                    Prop::choice(&["hover", "hover_and_click", "drag_and_drop"], "What to do")
                        .default("hover"),
                )
                .optional("timeout", timeout_prop(5.0)),
            hover_action,
        ),
    ]
}
