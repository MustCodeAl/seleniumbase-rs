//! What the host does with the raw protocol commands plugins send.
//!
//! A plugin reaches the browser through [`BrowserHandle::send`] and
//! [`PageHandle::send`], which pass a protocol command on. Two commands cannot
//! be left to each plugin on its own, because a browser has only one of each
//! per tab:
//!
//! * `Network.setBlockedURLs` *replaces* the tab's blocklist. Two plugins that
//!   each send theirs, or one that follows the library's ad-block list, would
//!   each undo the one before. The host keeps the union of everything sent to
//!   a tab and sends that ([`merge_blocked_urls`]).
//! * The `Fetch` domain pauses requests and hands them out to be answered.
//!   Only one client can own it per tab, and the library's proxy
//!   authentication already does. The host refuses `Fetch.*` from plugins
//!   ([`refuse_interception`]); a plugin that wants to block URLs sends
//!   `Network.setBlockedURLs`.
//!
//! Both engines call these from their handles, so a plugin sees the same rules
//! on the pure CDP engine and through chromedriver. They are the whole of the
//! guard for now: the other commands are passed on as they are (see "Limits"
//! in the plugin architecture page).
//!
//! [`BrowserHandle::send`]: seleniumbase_rs_plugin::BrowserHandle::send
//! [`PageHandle::send`]: seleniumbase_rs_plugin::PageHandle::send

use seleniumbase_rs_plugin::PluginError;
use serde_json::{json, Value};

/// The command that sets a tab's blocklist.
pub(crate) const SET_BLOCKED_URLS: &str = "Network.setBlockedURLs";

/// Refuses the commands of the `Fetch` domain.
///
/// # Errors
///
/// [`PluginError::Other`] naming `method` and what to send instead.
pub(crate) fn refuse_interception(method: &str) -> Result<(), PluginError> {
    if method.starts_with("Fetch.") {
        return Err(PluginError::other(format!(
            "{method} is not available to plugins: the host owns request interception (one \
             Fetch.enable per tab, shared with the library's proxy login), so a plugin cannot \
             enable, change or answer it. To stop requests, send Network.setBlockedURLs"
        )));
    }
    Ok(())
}

/// Adds the `urls` of a `Network.setBlockedURLs` command to the tab's
/// `known` blocklist and returns the command to send: the whole union, in the
/// order the patterns first arrived, each once.
///
/// # Errors
///
/// [`PluginError::Other`] when `params` has no `urls` list of strings, or has
/// the newer `urlPatterns` list, which cannot be merged with it.
pub(crate) fn merge_blocked_urls(
    known: &mut Vec<String>,
    params: &Value,
) -> Result<Value, PluginError> {
    if params.get("urlPatterns").is_some() {
        return Err(PluginError::other(
            "Network.setBlockedURLs with `urlPatterns` cannot be merged with the other blocklists \
             of the tab; send the patterns as `urls`",
        ));
    }
    let urls = params
        .get("urls")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            PluginError::other("Network.setBlockedURLs needs a `urls` list of URL patterns")
        })?;
    // Every entry is checked before the first is kept, so a bad command adds nothing.
    let urls = urls
        .iter()
        .map(|url| {
            url.as_str().ok_or_else(|| {
                PluginError::other(
                    "Network.setBlockedURLs needs every entry of `urls` to be a string",
                )
            })
        })
        .collect::<Result<Vec<&str>, PluginError>>()?;
    for url in urls {
        if !known.iter().any(|seen| seen == url) {
            known.push(url.to_owned());
        }
    }
    Ok(json!({ "urls": known }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_fetch_domain_is_refused() {
        for method in [
            "Fetch.enable",
            "Fetch.disable",
            "Fetch.fulfillRequest",
            "Fetch.continueRequest",
            "Fetch.failRequest",
        ] {
            let error = refuse_interception(method).unwrap_err().to_string();
            assert!(error.contains(method), "{error}");
            assert!(error.contains("Network.setBlockedURLs"), "{error}");
        }
        for method in [
            "Network.enable",
            "Network.setBlockedURLs",
            "FetchLike.enable",
            "fetch.enable",
            "Page.navigate",
        ] {
            refuse_interception(method).unwrap_or_else(|error| panic!("{method}: {error}"));
        }
    }

    #[test]
    fn urls_are_added_in_arrival_order_without_repeats() {
        let mut known = Vec::new();

        let first = merge_blocked_urls(&mut known, &json!({ "urls": ["a", "b"] })).unwrap();
        let second = merge_blocked_urls(&mut known, &json!({ "urls": ["b", "c"] })).unwrap();

        assert_eq!(first, json!({ "urls": ["a", "b"] }));
        assert_eq!(second, json!({ "urls": ["a", "b", "c"] }));
        assert_eq!(known, ["a", "b", "c"]);
    }

    #[test]
    fn an_empty_list_blocks_what_is_already_known_and_nothing_more() {
        let mut known = vec!["a".to_owned()];
        assert_eq!(
            merge_blocked_urls(&mut known, &json!({ "urls": [] })).unwrap(),
            json!({ "urls": ["a"] })
        );
        let mut nothing = Vec::new();
        assert_eq!(
            merge_blocked_urls(&mut nothing, &json!({ "urls": [] })).unwrap(),
            json!({ "urls": [] })
        );
    }

    #[test]
    fn a_command_that_cannot_be_merged_is_an_error_and_changes_nothing() {
        let mut known = vec!["a".to_owned()];
        for params in [
            json!({}),
            json!({ "urls": "a" }),
            json!({ "urls": ["b", 3] }),
            json!({ "urlPatterns": [{ "urlPattern": "b", "block": true }] }),
            Value::Null,
        ] {
            let error = merge_blocked_urls(&mut known, &params).unwrap_err();
            assert!(
                error.to_string().contains("Network.setBlockedURLs"),
                "{params}: {error}"
            );
        }
        assert_eq!(known, ["a"]);
    }
}
