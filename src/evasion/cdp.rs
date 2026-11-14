//! Chrome DevTools Protocol (CDP) helpers.
//!
//! Which protocol domains give an automated browser away. The DevTools client of a WebDriver
//! session is [`CdpClient`](crate::browser::devtools::CdpClient).

/// The commands [`CdpClient::enable_default_domains`](crate::browser::devtools::CdpClient::enable_default_domains) sends, in order.
///
/// None of the [`DETECTABLE_DOMAINS`] may be added: each is an automation tell
/// (see the method's documentation).
pub(crate) const DEFAULT_DOMAIN_COMMANDS: [&str; 2] = ["Page.enable", "Network.enable"];

/// The protocol domains whose `enable` command makes the browser show a page
/// that something is driving it.
///
/// `Runtime.enable` makes the browser report every console call and execution
/// context, which scripts probe by logging an object whose getter fires only
/// when a client listens. `DOM`, `Log`, `Debugger` and `Profiler` switch on
/// agents with the same kind of side effect: events nobody needs, and timing a
/// page can measure. The commands that matter here, such as `Runtime.evaluate`
/// and `Page.addScriptToEvaluateOnNewDocument`, work without any of them.
pub const DETECTABLE_DOMAINS: [&str; 5] = ["Runtime", "DOM", "Log", "Debugger", "Profiler"];

/// Whether `method` is the `enable` command of a domain in
/// [`DETECTABLE_DOMAINS`].
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::evasion::cdp::is_detectable_enable;
///
/// assert!(is_detectable_enable("Runtime.enable"));
/// assert!(!is_detectable_enable("Runtime.evaluate"));
/// assert!(!is_detectable_enable("Page.enable"));
/// ```
#[must_use]
pub fn is_detectable_enable(method: &str) -> bool {
    method
        .strip_suffix(".enable")
        .is_some_and(|domain| DETECTABLE_DOMAINS.contains(&domain))
}

/// Says so in the log when a caller sends one of the detectable `enable`
/// commands. It is still sent: a person who asks for it by name gets it.
pub(crate) fn warn_if_detectable(method: &str) {
    if is_detectable_enable(method) {
        tracing::warn!(
            method,
            "this command makes the browser show that it is driven; most pages do not need it"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{is_detectable_enable, DEFAULT_DOMAIN_COMMANDS, DETECTABLE_DOMAINS};

    #[test]
    fn the_default_domains_never_include_runtime() {
        // `Runtime.enable` makes the browser emit the console and
        // execution-context events that bot-detection scripts probe for.
        for command in DEFAULT_DOMAIN_COMMANDS {
            assert!(
                !command.starts_with("Runtime."),
                "{command} would make the automation detectable"
            );
        }
    }

    #[test]
    fn the_default_domains_include_none_of_the_detectable_ones() {
        for command in DEFAULT_DOMAIN_COMMANDS {
            assert!(
                !is_detectable_enable(command),
                "{command} would make the automation detectable"
            );
        }
        for domain in DETECTABLE_DOMAINS {
            assert!(
                DEFAULT_DOMAIN_COMMANDS
                    .iter()
                    .all(|command| !command.starts_with(&format!("{domain}."))),
                "{domain} is on by default"
            );
        }
    }

    #[test]
    fn exactly_the_enable_commands_of_the_five_domains_are_detectable() {
        for domain in ["Runtime", "DOM", "Log", "Debugger", "Profiler"] {
            assert!(
                is_detectable_enable(&format!("{domain}.enable")),
                "{domain}"
            );
        }
        for method in [
            "Runtime.evaluate",
            "Runtime.disable",
            "DOM.getDocument",
            "Page.enable",
            "Network.enable",
            "Fetch.enable",
            "Emulation.setDeviceMetricsOverride",
            "Page.addScriptToEvaluateOnNewDocument",
            "enable",
            ".enable",
            "Runtime.enabled",
            "NotRuntime.enable",
        ] {
            assert!(!is_detectable_enable(method), "{method}");
        }
    }

    #[test]
    fn the_default_domains_are_page_then_network() {
        assert_eq!(DEFAULT_DOMAIN_COMMANDS, ["Page.enable", "Network.enable"]);
    }
}
