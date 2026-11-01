//! Starting and closing the browser, and what is known about the session.

use super::{close_browser, get_devtools_endpoint, get_page_info, start_browser, Cdp};
use crate::mcp::{Effect, Prop, Schema, ToolDef};

pub(super) fn tools() -> Vec<ToolDef<Cdp>> {
    vec![
        ToolDef::new(
            "start_browser",
            "Start Browser",
            "Launch the Chrome session the other tools use. Does nothing if one is already \
             running. Headless by default on Linux, headed elsewhere.",
            Effect::Set,
            Schema::new()
                .optional("url", Prop::string("Page to open as the browser starts"))
                .optional(
                    "headless",
                    Prop::boolean("Force headless (true) or headed (false)"),
                )
                .optional(
                    "use_chromium",
                    Prop::boolean("Use Chromium instead of Google Chrome").default(false),
                )
                .optional(
                    "incognito",
                    Prop::boolean("Incognito window").default(false),
                )
                .optional(
                    "guest",
                    Prop::boolean("Guest profile; cannot be combined with incognito")
                        .default(false),
                )
                .optional(
                    "ad_block",
                    Prop::boolean("Block common ad requests").default(false),
                )
                .optional(
                    "proxy",
                    Prop::string("SERVER:PORT or USER:PASS@SERVER:PORT"),
                )
                .optional("fingerprint", crate::mcp::fingerprint::launch_inline_prop())
                .optional(
                    "fingerprint_path",
                    crate::mcp::fingerprint::launch_path_prop(),
                )
                .optional(
                    "fingerprint_preset",
                    crate::mcp::fingerprint::launch_preset_prop(),
                ),
            start_browser,
        ),
        ToolDef::new(
            "close_browser",
            "Close Browser",
            "Close the browser session and everything in it: tabs, cookies and history. Safe \
             to call when nothing is running.",
            Effect::Set,
            Schema::new(),
            close_browser,
        ),
        ToolDef::new(
            "get_page_info",
            "Get Page Info",
            "Report whether a browser is running and, if so, the active tab's URL, title, \
             origin and user agent.",
            Effect::Observe,
            Schema::new(),
            get_page_info,
        ),
        ToolDef::new(
            "get_devtools_endpoint",
            "Get DevTools Endpoint",
            "Report the DevTools endpoint of the running browser (HTTP address and browser \
             WebSocket URL) and ready-made MCP client configs that attach Google's \
             chrome-devtools-mcp (--browserUrl) or Microsoft's Playwright MCP \
             (--cdp-endpoint) to the same browser. Reports running=false when no browser runs.",
            Effect::Observe,
            Schema::new(),
            get_devtools_endpoint,
        ),
    ]
}
