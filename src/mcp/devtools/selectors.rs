//! Understanding the page and finding selectors that work.

use super::{diagnose_selector, inspect_page, selector_prop, suggest_selectors, DevSession};
use crate::mcp::{Effect, Prop, Schema, ToolDef};

pub(super) fn tools<S: DevSession>() -> Vec<ToolDef<S>> {
    vec![
        ToolDef::new(
            "inspect_page",
            "Inspect Page",
            "Outline the current page for writing a test: URL, title, ready state, headings, \
             forms, iframes, shadow-DOM hosts, and every control with its role, accessible name, \
             a selector that finds it, and whether it is visible and enabled. Bounded by limit; \
             says when it was cut short.",
            Effect::Observe,
            Schema::new()
                .optional(
                    "scope",
                    Prop::string("Only look inside the first element this selector matches"),
                )
                .optional(
                    "limit",
                    Prop::integer("Most controls to list, 1 to 500")
                        .min(1.0)
                        .default(50),
                )
                .optional(
                    "include_hidden",
                    Prop::boolean("Also list controls that are not visible").default(false),
                ),
            inspect_page::<S>,
        ),
        ToolDef::new(
            "suggest_selectors",
            "Suggest Selectors",
            "Find the element meant by its text, its role or a rough selector, and return ranked \
             stable selectors for it (test attributes, id, name, aria-label, link text, text \
             XPath, short CSS path), each checked against the page: how many elements it \
             matches and whether it finds this one.",
            Effect::Observe,
            Schema::new()
                .optional(
                    "text",
                    Prop::string("Text the element shows or is labelled with"),
                )
                .optional(
                    "role",
                    Prop::string("ARIA role such as button, link, textbox or checkbox"),
                )
                .optional("selector", Prop::string("A rough selector that finds it"))
                .optional(
                    "limit",
                    Prop::integer("Most selectors per element, 1 to 20")
                        .min(1.0)
                        .default(5),
                ),
            suggest_selectors::<S>,
        ),
        ToolDef::new(
            "diagnose_selector",
            "Diagnose Selector",
            "Explain why a selector finds nothing or cannot be used: invalid syntax (with the \
             browser's message), no match, matches that are hidden, empty, off screen, disabled \
             or covered by another element, matches inside an iframe or a shadow root, and near \
             misses such as a similar id or the same text in different case. Ends with next \
             steps.",
            Effect::Observe,
            Schema::new().required("selector", selector_prop()),
            diagnose_selector::<S>,
        ),
    ]
}
