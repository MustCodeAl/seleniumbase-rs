//! Hovering, dragging, clicking inside a container, and file inputs.

use crate::api::base_case::BaseCase;
use crate::mcp::webdriver::{self as w, selector_prop, timeout_prop};
use crate::mcp::{Effect, Prop, Schema, ToolDef};

pub(super) fn tools() -> Vec<ToolDef<BaseCase>> {
    vec![
        ToolDef::new(
            "hover_and_click",
            "Hover And Click",
            "Hover one element, then click another; for menus that open on hover.",
            Effect::Act,
            Schema::new()
                .required("hover_selector", Prop::string("Element to hover"))
                .required(
                    "click_selector",
                    Prop::string("Element to click afterwards"),
                )
                .optional("timeout", timeout_prop(5.0)),
            w::hover_and_click,
        ),
        ToolDef::new(
            "drag_and_drop",
            "Drag And Drop",
            "Drag one element onto another.",
            Effect::Act,
            Schema::new()
                .required("drag_selector", Prop::string("Element to drag"))
                .required("drop_selector", Prop::string("Element to drop it on"))
                .optional("timeout", timeout_prop(5.0)),
            w::drag_and_drop,
        ),
        ToolDef::new(
            "nested_click",
            "Nested Click",
            "Click an element inside a parent element, or inside a parent iframe.",
            Effect::Act,
            Schema::new()
                .required("parent_selector", Prop::string("The container or iframe"))
                .required("selector", selector_prop()),
            w::nested_click,
        ),
        ToolDef::new(
            "choose_file",
            "Choose File",
            "Fill a file input with a file from this machine.",
            Effect::Act,
            Schema::new()
                .required("selector", selector_prop())
                .required(
                    "file_path",
                    Prop::string("Path of the file on this machine"),
                )
                .optional("timeout", timeout_prop(5.0)),
            w::choose_file,
        ),
    ]
}
