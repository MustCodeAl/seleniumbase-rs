//! Cookies, `localStorage` and `sessionStorage`.

use crate::api::base_case::BaseCase;
use crate::mcp::webdriver as w;
use crate::mcp::{Effect, Prop, Schema, ToolDef};

pub(super) fn tools() -> Vec<ToolDef<BaseCase>> {
    vec![
        ToolDef::new(
            "get_cookies",
            "Get Cookies",
            "List the session's cookies. They can hold logins; handle them with care.",
            Effect::Observe,
            Schema::new(),
            w::get_cookies,
        ),
        ToolDef::new(
            "delete_all_cookies",
            "Delete All Cookies",
            "Delete every cookie in the session.",
            Effect::Act,
            Schema::new(),
            w::delete_all_cookies,
        ),
        ToolDef::new(
            "save_cookies",
            "Save Cookies",
            "Save the session's cookies to a file in the server's output directory, under \
saved_cookies.",
            Effect::Overwrite,
            Schema::new().optional(
                "name",
                Prop::string("File name; only its last path part is used").default("cookies.txt"),
            ),
            w::save_cookies,
        ),
        ToolDef::new(
            "load_cookies",
            "Load Cookies",
            "Load cookies from a file saved by save_cookies.",
            Effect::Act,
            Schema::new().optional(
                "name",
                Prop::string("File name; only its last path part is used").default("cookies.txt"),
            ),
            w::load_cookies,
        ),
        ToolDef::new(
            "get_local_storage_item",
            "Get Local Storage Item",
            "Read one key of localStorage for the current page's origin.",
            Effect::Observe,
            Schema::new().required("key", Prop::string("The storage key")),
            w::get_local_storage_item,
        ),
        ToolDef::new(
            "set_local_storage_item",
            "Set Local Storage Item",
            "Write one key of localStorage for the current page's origin.",
            Effect::Set,
            Schema::new()
                .required("key", Prop::string("The storage key"))
                .required("value", Prop::string("The value to store")),
            w::set_local_storage_item,
        ),
        ToolDef::new(
            "get_session_storage_item",
            "Get Session Storage Item",
            "Read one key of sessionStorage for the current page's origin.",
            Effect::Observe,
            Schema::new().required("key", Prop::string("The storage key")),
            w::get_session_storage_item,
        ),
        ToolDef::new(
            "set_session_storage_item",
            "Set Session Storage Item",
            "Write one key of sessionStorage for the current page's origin.",
            Effect::Set,
            Schema::new()
                .required("key", Prop::string("The storage key"))
                .required("value", Prop::string("The value to store")),
            w::set_session_storage_item,
        ),
    ]
}
