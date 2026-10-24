//! A real Chrome driven through `sbrs-mcp --server cdp --listen`: the browser
//! session a client starts over HTTP is its own, and closes when it ends.
//! `#[ignore]`d because it launches Chrome; run it alone with
//! `cargo test -p seleniumbase-rs-cli --test mcp_http_chrome -- --ignored`.

#![cfg(feature = "mcp-server")]

#[path = "support/mcp_http.rs"]
mod support;

use serde_json::{json, Value};
use support::{Mcp, Server};

const PAGE: &str = "data:text/html,<title>Over HTTP</title><h1>hello</h1>";

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn the_cdp_server_drives_chrome_for_a_client_over_http() {
    let mut server = Server::start(&["--server", "cdp", "--listen", "127.0.0.1:0"], &[]).await;
    assert!(server.ready.ends_with("(server cdp)"), "{}", server.ready);

    let mut mcp = Mcp::new(&server.url);
    mcp.initialize().await;

    let started = mcp.text("start_browser", json!({ "headless": true })).await;
    assert!(
        started.starts_with("Started Pure CDP Mode browser"),
        "{started}"
    );
    mcp.text("open_url", json!({ "url": PAGE })).await;
    let info: Value =
        serde_json::from_str(&mcp.text("get_page_info", json!({})).await).expect("JSON text");
    assert_eq!(info["running"], true, "{info}");
    assert_eq!(info["title"], "Over HTTP", "{info}");

    // A second client has a browser of its own: it finds none running.
    let mut other = Mcp::new(&server.url);
    other.initialize().await;
    let other_info: Value =
        serde_json::from_str(&other.text("get_page_info", json!({})).await).expect("JSON text");
    assert_eq!(other_info["running"], false, "{other_info}");

    assert_eq!(
        mcp.text("close_browser", json!({})).await,
        "The browser session was closed."
    );

    #[cfg(unix)]
    {
        server.signal("TERM");
        assert_eq!(server.exit_code().await, Some(0), "{}", server.log());
    }
    assert!(!server.log().contains("panicked"), "{}", server.log());
}
