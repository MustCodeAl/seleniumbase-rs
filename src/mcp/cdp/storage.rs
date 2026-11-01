//! Cookies, `localStorage` and `sessionStorage`.

use super::{manage_cookies, manage_storage, Cdp};
use crate::mcp::{Effect, Prop, Schema, ToolDef};

pub(super) fn tools() -> Vec<ToolDef<Cdp>> {
    vec![
        ToolDef::new(
            "manage_cookies",
            "Manage Cookies",
            "List, clear, save or load the session's cookies. Saved files live in the \
             server's output directory, under saved_cookies. Cookies can hold logins; \
             handle them with care.",
            Effect::Mixed,
            Schema::new()
                .optional(
                    "action",
                    Prop::choice(&["get_all", "clear", "save", "load"], "What to do")
                        .default("get_all"),
                )
                .optional(
                    "filename",
                    Prop::string("File name for save and load; only its last path part is used")
                        .default("cookies.txt"),
                ),
            manage_cookies,
        ),
        ToolDef::new(
            "manage_storage",
            "Manage Storage",
            "Read or write one key of localStorage or sessionStorage for the current page's \
             origin.",
            Effect::Mixed,
            Schema::new()
                .required("key", Prop::string("The storage key"))
                .optional(
                    "value",
                    Prop::string("The value to store; required for set"),
                )
                .optional(
                    "storage",
                    Prop::choice(&["local", "session"], "Which storage").default("local"),
                )
                .optional(
                    "action",
                    Prop::choice(&["get", "set"], "Read or write").default("get"),
                ),
            manage_storage,
        ),
    ]
}
