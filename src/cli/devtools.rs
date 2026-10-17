//! `sbrs devtools`: a Chrome with its DevTools endpoint open for other tools.
//!
//! The command launches a browser with the Pure CDP engine
//! ([`crate::sb_cdp`]), prints the endpoint and ready-to-paste configs for
//! Google's `chrome-devtools-mcp` and Microsoft's Playwright MCP, and keeps the
//! browser running until it is stopped. This module holds the parts that need
//! no browser: turning the options into [`LaunchOptions`], checking the port,
//! and the text the command prints.

use std::net::{Ipv4Addr, SocketAddrV4, TcpListener};
use std::path::PathBuf;

use crate::config::plugins::PluginSelection;
use crate::error::SeleniumBaseError;
use crate::sb_cdp::{DevToolsEndpoint, LaunchOptions};

/// The port `sbrs devtools` asks Chrome to listen on when none is given: the
/// one the MCP servers' documentation uses.
pub const DEFAULT_DEVTOOLS_PORT: u16 = 9222;

/// What `sbrs devtools` launches, after the command line was read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DevtoolsRequest {
    /// Run without a window (`Some(true)`) or with one (`Some(false)`); `None`
    /// keeps the engine's default, headless on Linux only.
    pub headless: Option<bool>,
    /// The DevTools port on `127.0.0.1`; `0` lets Chrome pick a free one.
    pub port: u16,
    /// The profile directory to keep; a throwaway one when `None`.
    pub user_data_dir: Option<PathBuf>,
    /// `SERVER:PORT`, `USER:PASS@SERVER:PORT` or `scheme://SERVER:PORT`.
    pub proxy: Option<String>,
    /// The browser binary; Google Chrome is searched for when `None`.
    pub executable: Option<PathBuf>,
    /// Look for Chromium instead of Google Chrome.
    pub use_chromium: bool,
    /// A user agent to start the browser with.
    pub user_agent: Option<String>,
    /// The browser language, such as `de-DE`.
    pub locale: Option<String>,
    /// Block common advertising and tracking hosts.
    pub ad_block: bool,
    /// An unpacked extension to load.
    pub extension_dir: Option<PathBuf>,
    /// The page to open once the browser runs.
    pub url: Option<String>,
    /// The built-in plugins to run in the browser, by name, with their options
    /// (the `[plugins]` table, `SB_PLUGINS`, `--plugin` and `--plugin-opt`).
    pub plugins: PluginSelection,
}

impl DevtoolsRequest {
    /// The launch options of the Pure CDP engine for this request.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] when the proxy cannot be
    /// parsed, Chromium is combined with an explicit binary, or the request
    /// names plugins and this build cannot run any; and
    /// [`SeleniumBaseError::Plugin`] for a plugin name this build does not have
    /// or options a plugin refuses.
    pub fn launch_options(&self) -> Result<LaunchOptions, SeleniumBaseError> {
        self.plugins.ensure_supported()?;
        let mut builder = LaunchOptions::builder()
            .debugging_port(self.port)
            .use_chromium(self.use_chromium)
            .ad_block(self.ad_block);
        if let Some(headless) = self.headless {
            builder = builder.headless(headless);
        }
        if let Some(dir) = &self.user_data_dir {
            builder = builder.user_data_dir(dir);
        }
        if let Some(proxy) = &self.proxy {
            builder = builder.proxy(proxy);
        }
        if let Some(path) = &self.executable {
            builder = builder.executable_path(path);
        }
        if let Some(user_agent) = &self.user_agent {
            builder = builder.user_agent(user_agent);
        }
        if let Some(locale) = &self.locale {
            builder = builder.lang(locale);
        }
        if let Some(dir) = &self.extension_dir {
            builder = builder.extension(dir);
        }
        if let Some(url) = &self.url {
            builder = builder.url(url);
        }
        #[cfg(feature = "plugins")]
        {
            builder = builder.plugin_selection(&self.plugins);
        }
        builder.build()
    }
}

/// Checks that `port` on `127.0.0.1` is free, so a taken port is reported at
/// once instead of as a browser that never publishes its endpoint. Port `0`
/// needs no check.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::InvalidConfig`] naming the port when something
/// already listens on it.
pub fn ensure_port_is_free(port: u16) -> Result<(), SeleniumBaseError> {
    if port == 0 {
        return Ok(());
    }
    TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port))
        .map(drop)
        .map_err(|error| {
            SeleniumBaseError::invalid_config(format!(
                "port {port} on 127.0.0.1 is not free ({error}); another browser may already \
                 be listening there. Pass --port 0 to let Chrome pick a free port, or another port"
            ))
        })
}

/// What `sbrs devtools` prints once the browser runs: the endpoint, the client
/// configs, and how to stop it.
#[must_use]
pub fn ready_message(endpoint: &DevToolsEndpoint, browser: &str) -> String {
    let pretty = |value: &serde_json::Value| {
        serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
    };
    let http = endpoint.http_url();
    let mut text = format!(
        "{browser} is running with its DevTools endpoint open.\n\
         \n\
         DevTools HTTP endpoint:  {http}\n\
         Browser WebSocket URL:   {websocket}\n\
         Version document:        {version}\n\
         \n\
         Google chrome-devtools-mcp (attaches with --browserUrl):\n\
         {chrome}\n\
         \n\
         Microsoft Playwright MCP (attaches with --cdp-endpoint):\n\
         {playwright}\n\
         \n\
         Claude Code:\n",
        websocket = endpoint.websocket_url(),
        version = endpoint.version_url(),
        chrome = pretty(&endpoint.chrome_devtools_mcp_config()),
        playwright = pretty(&endpoint.playwright_mcp_config()),
    );
    for command in endpoint.claude_code_commands() {
        text.push_str("  ");
        text.push_str(&command);
        text.push('\n');
    }
    text.push_str(
        "\n\
         Edge: run sbrs devtools --browser edge (or --browser-path pointing at Edge) and use the \
         same configs. chrome-devtools-mcp supports only Chrome officially; Playwright MCP's \
         --cdp-endpoint works with any Chromium browser.\n\
         Firefox and Safari do not speak this protocol: see docs/tutorials/devtools_mcp.md for \
         @mozilla/firefox-devtools-mcp and safaridriver --mcp.\n\
         \n\
         The endpoint listens on 127.0.0.1 only, and any program on this machine can drive the \
         browser through it.\n\
         Press Ctrl+C to close the browser and exit.\n",
    );
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn endpoint() -> DevToolsEndpoint {
        DevToolsEndpoint::new(
            "http://127.0.0.1:9222",
            "ws://127.0.0.1:9222/devtools/browser/id",
        )
    }

    #[test]
    fn the_port_reaches_chrome_and_zero_lets_chrome_pick() {
        let request = DevtoolsRequest {
            headless: Some(true),
            port: 9333,
            ..DevtoolsRequest::default()
        };
        let args = request
            .launch_options()
            .expect("options")
            .browser_args(std::path::Path::new("/tmp/p"));
        assert!(
            args.contains(&"--remote-debugging-port=9333".to_owned()),
            "{args:?}"
        );
        assert!(args.contains(&"--headless=new".to_owned()), "{args:?}");

        let default = DevtoolsRequest::default()
            .launch_options()
            .expect("options");
        assert_eq!(
            default.is_headless(),
            LaunchOptions::default().is_headless(),
            "the engine's default unless asked"
        );
        let any = default.browser_args(std::path::Path::new("/tmp/p"));
        assert!(
            any.contains(&"--remote-debugging-port=0".to_owned()),
            "{any:?}"
        );
        let headed = DevtoolsRequest {
            headless: Some(false),
            ..DevtoolsRequest::default()
        };
        assert!(!headed.launch_options().expect("options").is_headless());
    }

    #[test]
    fn every_option_reaches_the_launch() {
        let request = DevtoolsRequest {
            headless: Some(false),
            port: 0,
            user_data_dir: Some(PathBuf::from("/tmp/devtools-profile")),
            proxy: Some("proxy.example:8080".to_owned()),
            user_agent: Some("Agent/1.0".to_owned()),
            locale: Some("de-DE".to_owned()),
            url: Some("https://example.com".to_owned()),
            ..DevtoolsRequest::default()
        };
        let options = request.launch_options().expect("options");
        assert_eq!(options.url(), Some("https://example.com"));
        assert_eq!(
            options.proxy().map(crate::sb_cdp::Proxy::server),
            Some("proxy.example:8080")
        );
        let args = options.browser_args(std::path::Path::new("/tmp/devtools-profile"));
        for expected in [
            "--user-data-dir=/tmp/devtools-profile",
            "--proxy-server=proxy.example:8080",
            "--user-agent=Agent/1.0",
            "--lang=de-DE",
        ] {
            assert!(args.contains(&expected.to_owned()), "{expected}: {args:?}");
        }
    }

    #[cfg(feature = "plugin-shield")]
    #[test]
    fn the_plugin_selection_becomes_the_plugins_of_the_launch() {
        let request = DevtoolsRequest {
            plugins: PluginSelection::from_list("shield"),
            ..DevtoolsRequest::default()
        };
        let options = request.launch_options().expect("options");
        assert_eq!(options.plugins.names(), ["shield"]);

        let none = DevtoolsRequest::default()
            .launch_options()
            .expect("options");
        assert!(none.plugins.is_empty(), "no selection, no plugin");
    }

    #[cfg(feature = "plugins")]
    #[test]
    fn a_plugin_this_build_does_not_have_is_refused_before_launch() {
        let request = DevtoolsRequest {
            plugins: PluginSelection::from_list("no-such-plugin"),
            ..DevtoolsRequest::default()
        };
        let error = request.launch_options().expect_err("an unknown plugin");
        assert!(error.to_string().contains("no-such-plugin"), "{error}");
    }

    #[cfg(not(feature = "plugins"))]
    #[test]
    fn without_plugin_support_a_selection_is_refused_before_launch() {
        let request = DevtoolsRequest {
            plugins: PluginSelection::from_list("shield"),
            ..DevtoolsRequest::default()
        };
        let error = request.launch_options().expect_err("no plugin support");
        assert!(
            error.to_string().contains("--features plugins-builtin"),
            "{error}"
        );
    }

    #[test]
    fn a_bad_combination_is_refused_before_launch() {
        let request = DevtoolsRequest {
            use_chromium: true,
            executable: Some(PathBuf::from("/usr/bin/chromium")),
            ..DevtoolsRequest::default()
        };
        assert!(request.launch_options().is_err());
        let empty_proxy = DevtoolsRequest {
            proxy: Some("user:pass@".to_owned()),
            ..DevtoolsRequest::default()
        };
        assert!(empty_proxy.launch_options().is_err());
    }

    #[test]
    fn a_taken_port_is_reported_and_zero_is_never_checked() {
        let taken = TcpListener::bind("127.0.0.1:0").expect("a free port");
        let port = taken.local_addr().expect("address").port();
        let error = ensure_port_is_free(port).expect_err("the port is taken");
        assert!(error.to_string().contains(&port.to_string()), "{error}");
        assert!(error.to_string().contains("--port 0"), "{error}");
        drop(taken);
        assert!(ensure_port_is_free(0).is_ok());
    }

    #[test]
    fn the_ready_message_holds_the_endpoint_and_both_configs() {
        let text = ready_message(&endpoint(), "Google Chrome");
        assert!(text.starts_with("Google Chrome is running"), "{text}");
        assert!(text.contains("DevTools HTTP endpoint:  http://127.0.0.1:9222\n"));
        assert!(text.contains("Browser WebSocket URL:   ws://127.0.0.1:9222/devtools/browser/id"));
        assert!(
            text.contains("\"--browserUrl=http://127.0.0.1:9222\""),
            "{text}"
        );
        assert!(
            text.contains("\"--cdp-endpoint=http://127.0.0.1:9222\""),
            "{text}"
        );
        assert!(text.contains("claude mcp add chrome-devtools -- npx -y chrome-devtools-mcp@"));
        assert!(text.contains("Ctrl+C"));
        // The pasted configs must be valid JSON as printed.
        let start = text.find("{\n").expect("a JSON block");
        let block: String = text[start..]
            .lines()
            .take_while(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        let parsed: serde_json::Value = serde_json::from_str(&block).expect("valid JSON");
        assert_eq!(parsed, endpoint().chrome_devtools_mcp_config());
    }
}
