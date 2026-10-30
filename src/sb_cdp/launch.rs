//! Describing and starting a Chrome process with a DevTools endpoint.

use std::collections::BTreeSet;

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{Map, Value};
use tempfile::TempDir;
use tokio::process::{Child, Command};

use super::identity::StealthPlan;
use crate::browser::options::{
    chrome_preferences, emulated_phone, DeviceMetrics, LaunchSwitch, PacUrl, ProxyBypassList,
    VirtualDisplay, MOBILE_USER_AGENT,
};
use crate::browser::virtual_display::{self, XvfbProcess};

use crate::common::orphan_guard::OrphanGuard;
use crate::error::SeleniumBaseError;
use crate::evasion::fingerprint::{Dimension, Fingerprint, ProxyConfig, WebRtcPolicy};
use crate::evasion::patcher::find_system_chrome;

mod bridge;
mod builders;

/// How long a freshly launched browser may take to publish its endpoint.
///
/// Cold starts on a busy CI machine routinely take several seconds; twenty
/// leaves ample margin without hiding a browser that has genuinely hung.
const DEFAULT_STARTUP_TIMEOUT: Duration = Duration::from_secs(20);

/// How often to look for the endpoint file while the browser starts.
const STARTUP_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// How long one look at a fixed port's `/json/version` may take.
const VERSION_PROBE_TIMEOUT: Duration = Duration::from_millis(500);

/// An HTTP or SOCKS proxy, with optional credentials.
///
/// Parsed from `SERVER:PORT` or `USER:PASS@SERVER:PORT`, optionally prefixed
/// with a scheme such as `socks5://`. Credentials are never shown by
/// [`Debug`](fmt::Debug) or [`Display`](fmt::Display).
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::sb_cdp::Proxy;
///
/// let proxy = Proxy::parse("alice:s3cret@proxy.example.com:8080")?;
/// assert_eq!(proxy.server(), "proxy.example.com:8080");
/// assert!(proxy.has_credentials());
/// assert!(!format!("{proxy:?}").contains("s3cret"));
/// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
/// ```
#[derive(Clone, PartialEq, Eq)]
pub struct Proxy {
    server: String,
    credentials: Option<(String, String)>,
}

impl Proxy {
    /// Parses a proxy specification.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] if the server part is empty.
    pub fn parse(spec: &str) -> Result<Self, SeleniumBaseError> {
        // A scheme prefix such as "http://" stays on the server part.
        let (scheme, rest) = match spec.split_once("://") {
            Some((scheme, rest)) => (Some(scheme), rest),
            None => (None, spec),
        };
        let (credentials, host) = match rest.rsplit_once('@') {
            Some((auth, host)) => {
                let (user, pass) = auth.split_once(':').unwrap_or((auth, ""));
                (Some((user.to_owned(), pass.to_owned())), host)
            }
            None => (None, rest),
        };
        if host.trim().is_empty() {
            return Err(SeleniumBaseError::invalid_config(
                "a proxy needs a server, such as host:port",
            ));
        }
        let server = scheme.map_or_else(|| host.to_owned(), |scheme| format!("{scheme}://{host}"));
        Ok(Self {
            server,
            credentials,
        })
    }

    /// The `host:port` (with scheme, if one was given) Chrome connects to.
    #[must_use]
    pub fn server(&self) -> &str {
        &self.server
    }

    /// Whether the proxy requires a username and password.
    #[must_use]
    pub fn has_credentials(&self) -> bool {
        self.credentials.is_some()
    }

    /// The username and password, if the proxy has them. They are secrets: pass them on to the
    /// proxy, never print or log them.
    #[must_use]
    pub fn credentials(&self) -> Option<(&str, &str)> {
        self.credentials
            .as_ref()
            .map(|(user, pass)| (user.as_str(), pass.as_str()))
    }
}

impl fmt::Debug for Proxy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Proxy")
            .field("server", &self.server)
            .field(
                "credentials",
                &self.credentials.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

impl fmt::Display for Proxy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.server)
    }
}

/// Which browser binary to launch.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) enum BrowserSource {
    /// Google Chrome, found on this machine.
    #[default]
    Chrome,
    /// Chromium, found on this machine.
    Chromium,
    /// The binary at this path.
    Path(PathBuf),
}

/// How the browser profile is presented.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum ProfileMode {
    /// A regular profile.
    #[default]
    Standard,
    /// An incognito window.
    Incognito,
    /// Guest mode.
    Guest,
}

/// How to launch a Pure CDP browser.
///
/// Build one with [`LaunchOptions::builder`]; the default launches a visible
/// browser on macOS and Windows and a headless one on Linux.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::sb_cdp::LaunchOptions;
///
/// let options = LaunchOptions::builder()
///     .headless(true)
///     .incognito(true)
///     .window_size(1280, 800)
///     .build()?;
/// assert!(options.is_headless());
/// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
/// ```
#[derive(Debug, Clone)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the options mirror the independent on/off switches of the builder"
)]
pub struct LaunchOptions {
    pub(crate) url: Option<String>,
    pub(crate) cdp_url: Option<Redacted>,
    /// The window to drive when attaching: a target id, or part of its URL.
    pub(crate) cdp_target: Option<String>,

    pub(crate) headless: Option<bool>,
    pub(crate) source: BrowserSource,
    pub(crate) mode: ProfileMode,
    pub(crate) ad_block: bool,
    pub(crate) proxy: Option<Proxy>,
    pub(crate) user_data_dir: Option<PathBuf>,
    pub(crate) user_agent: Option<String>,
    pub(crate) lang: Option<String>,
    pub(crate) window_size: Option<(u32, u32)>,
    pub(crate) window_position: Option<(i32, i32)>,
    pub(crate) no_sandbox: bool,
    pub(crate) webrtc: Option<WebRtcPolicy>,
    pub(crate) shield_webrtc: bool,
    /// What every tab is made to look like, if a fingerprint was given.
    pub(crate) identity: Option<Arc<StealthPlan>>,
    pub(crate) extra_args: Vec<String>,
    /// Unpacked extension folders, loaded over the protocol once the browser runs.
    pub(crate) extensions: Vec<PathBuf>,
    /// Preferences written into the profile before the browser starts.
    pub(crate) prefs: Map<String, Value>,
    /// `--host-resolver-rules` entries, joined into the one switch Chrome reads.
    /// Only plugins set these.
    pub(crate) host_resolver_rules: Vec<String>,
    /// Features to turn off, merged with the library's own into one
    /// `--disable-features` switch. Only plugins set these.
    pub(crate) disable_features: Vec<String>,
    /// Features to turn on, as one `--enable-features` switch. Only plugins
    /// set these.
    pub(crate) enable_features: Vec<String>,
    /// Whether to pass `--disable-quic`. Only plugins set it.
    pub(crate) disable_quic: bool,
    /// The on/off settings that are on (block images, dark mode, devtools, ...).
    pub(crate) switches: BTreeSet<LaunchSwitch>,
    /// The phone every tab is made, and the user agent it reports.
    pub(crate) device: Option<(DeviceMetrics, Option<String>)>,
    /// Hosts that skip the proxy.
    pub(crate) proxy_bypass_list: ProxyBypassList,
    /// The proxy auto-config file; a login it asks for is answered over the
    /// protocol.
    pub(crate) proxy_pac_url: Option<PacUrl>,
    /// The virtual screen a headed browser opens its window on (Linux).
    pub(crate) virtual_display: Option<VirtualDisplay>,

    /// The plugins whose hooks run at launch, for every tab and at close.
    #[cfg(feature = "plugins")]
    pub(crate) plugins: seleniumbase_rs_plugin::PluginSet,
    pub(crate) startup_timeout: Duration,
    /// The DevTools port on `127.0.0.1`; `0` lets Chrome pick a free one.
    pub(crate) debugging_port: u16,
}

impl Default for LaunchOptions {
    fn default() -> Self {
        LaunchOptionsBuilder::default().assemble(None, None)
    }
}

impl LaunchOptions {
    /// The external debugger or authenticated Tauri discovery URL, if set.
    #[must_use]
    pub fn cdp_url(&self) -> Option<&str> {
        self.cdp_url.as_ref().map(|url| url.0.as_str())
    }

    /// The window an attached browser is asked to drive, if one was named with
    /// [`cdp_target`](LaunchOptionsBuilder::cdp_target): a target id, or part
    /// of the window's URL.
    #[must_use]
    pub fn cdp_target(&self) -> Option<&str> {
        self.cdp_target.as_deref()
    }

    /// Starts building launch options.
    #[must_use]
    pub fn builder() -> LaunchOptionsBuilder {
        LaunchOptionsBuilder::default()
    }

    /// Whether the browser will run headless once defaults are applied.
    #[must_use]
    pub fn is_headless(&self) -> bool {
        self.headless.unwrap_or(cfg!(target_os = "linux"))
    }

    /// The proxy the browser will use, if any.
    #[must_use]
    pub fn proxy(&self) -> Option<&Proxy> {
        self.proxy.as_ref()
    }

    /// The page the browser opens once it has started, if one was set.
    #[must_use]
    pub fn url(&self) -> Option<&str> {
        self.url.as_deref()
    }

    /// The user agent the browser will be started with (`--user-agent`), if one was set.
    #[must_use]
    pub fn user_agent(&self) -> Option<&str> {
        self.user_agent.as_deref()
    }

    /// The language the browser will be started with (`--lang`), if one was set.
    #[must_use]
    pub fn lang(&self) -> Option<&str> {
        self.lang.as_deref()
    }

    /// The command-line arguments Chrome will be started with.
    ///
    /// `profile_dir` is the profile directory in use, which may be a
    /// throwaway temporary one.
    #[must_use]
    pub fn browser_args(&self, profile_dir: &Path) -> Vec<String> {
        let headless = self.is_headless();
        // A maximized window takes the screen, so no size is asked for.
        let maximized = !headless && self.switches.contains(&LaunchSwitch::Maximize);

        // Chrome keeps only the last of a switch, so the four that plugins set
        // are folded together with the same switch in `extra_args` (a list
        // switch is joined; the WebRTC policy of the options wins) and left out
        // of what follows.
        let mut extra_args = self.extra_args.clone();
        let disable_features =
            with_switch_values(&mut extra_args, "disable-features", &self.disable_features);
        let enable_features =
            with_switch_values(&mut extra_args, "enable-features", &self.enable_features);
        let host_resolver_rules = with_switch_values(
            &mut extra_args,
            "host-resolver-rules",
            &self.host_resolver_rules,
        );
        if self.webrtc.is_some() {
            take_switch_values(&mut extra_args, "force-webrtc-ip-handling-policy");
        }
        let mut args = vec![
            format!("--remote-debugging-port={}", self.debugging_port),
            format!("--user-data-dir={}", profile_dir.display()),
            "--no-first-run".to_owned(),
            "--no-default-browser-check".to_owned(),
            "--disable-search-engine-choice-screen".to_owned(),
            // Chrome keeps only the last `--disable-features`, so there is one:
            // the library's own entry and the ones plugins ask for.
            format!(
                "--disable-features={}",
                feature_list(&["Translate"], &disable_features)
            ),
            // Keeps macOS from prompting for keychain access on launch.
            "--password-store=basic".to_owned(),
            "--use-mock-keychain".to_owned(),
        ];
        if self.is_headless() {
            args.push("--headless=new".to_owned());
        }
        match self.mode {
            ProfileMode::Standard => {}
            ProfileMode::Incognito => args.push("--incognito".to_owned()),
            ProfileMode::Guest => args.push("--guest".to_owned()),
        }
        if self.no_sandbox {
            args.push("--no-sandbox".to_owned());
        }
        if let Some(policy) = self.webrtc {
            args.push(super::webrtc::chrome_flag(policy).to_owned());
        }
        if !enable_features.is_empty() {
            args.push(format!(
                "--enable-features={}",
                feature_list(&[], &enable_features)
            ));
        }
        if !host_resolver_rules.is_empty() {
            args.push(format!(
                "--host-resolver-rules={}",
                feature_list(&[], &host_resolver_rules)
            ));
        }
        if self.disable_quic {
            args.push("--disable-quic".to_owned());
        }
        if let Some(proxy) = &self.proxy {
            // Chrome ignores credentials here; they are supplied over the
            // protocol when the proxy asks for them.
            args.push(format!("--proxy-server={}", proxy.server()));
        }
        if let Some(hosts) = self.proxy_bypass_list.to_chrome_value() {
            args.push(format!("--proxy-bypass-list={hosts}"));
        }
        if let Some(pac) = &self.proxy_pac_url {
            // Chrome is never given the login; the protocol answers it.
            args.push(format!("--proxy-pac-url={}", pac.url()));
        }

        if let Some(user_agent) = &self.user_agent {
            args.push(format!("--user-agent={user_agent}"));
        }
        if let Some(lang) = &self.lang {
            args.push(format!("--lang={lang}"));
        }
        if let Some((width, height)) = self.window_size.filter(|_| !maximized) {
            args.push(format!("--window-size={width},{height}"));
        }
        if let Some((x, y)) = self.window_position {
            args.push(format!("--window-position={x},{y}"));
        }
        args.extend(self.switch_arguments(headless));

        if !self.extensions.is_empty() {
            // Branded Chrome ignores `--load-extension` since version 137; this
            // switch lets `Extensions.loadUnpacked` load them instead.
            args.push("--enable-unsafe-extension-debugging".to_owned());
        }
        args.extend(extra_args);
        // The first page the browser opens.
        args.push("about:blank".to_owned());
        args
    }
}

/// The value of the switch called `name` in `arg`, however it is spelled
/// (`--name=v`, `-name=v` or `name=v`).
fn switch_value<'a>(arg: &'a str, name: &str) -> Option<&'a str> {
    arg.trim_start_matches('-')
        .strip_prefix(name)?
        .strip_prefix('=')
}

/// Removes every `--name=...` switch from `args` and returns the comma-separated
/// entries of their values, in order.
fn take_switch_values(args: &mut Vec<String>, name: &str) -> Vec<String> {
    let mut values = Vec::new();
    args.retain(|arg| match switch_value(arg, name) {
        Some(value) => {
            values.extend(value.split(',').map(str::to_owned));
            false
        }
        None => true,
    });
    values
}

/// `own`, then the entries of the switch `name` found in `args` (which are
/// removed from it), as one list.
fn with_switch_values(args: &mut Vec<String>, name: &str, own: &[String]) -> Vec<String> {
    let mut values = own.to_vec();
    values.extend(take_switch_values(args, name));
    values
}

/// The entries of `own` and then those of `extra`, each once, joined with the
/// comma Chrome separates the values of one switch with.
fn feature_list(own: &[&str], extra: &[String]) -> String {
    let mut items: Vec<&str> = Vec::new();
    for item in own.iter().copied().chain(extra.iter().map(String::as_str)) {
        let item = item.trim();
        if !item.is_empty() && !items.contains(&item) {
            items.push(item);
        }
    }
    items.join(",")
}

/// A string that never prints itself, for a proxy URL that may hold a password.
#[derive(Clone)]
pub(crate) struct Redacted(String);

impl fmt::Debug for Redacted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Redacted(..)")
    }
}

/// The proxy a fingerprint asks for, as a URL, when it asks for one.
fn proxy_of(fingerprint: &Fingerprint) -> Option<String> {
    fingerprint.proxy().value().map(ProxyConfig::to_url)
}

/// Builds [`LaunchOptions`]. Setters never fail; [`build`](Self::build)
/// checks the combination.
#[derive(Debug, Clone)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "a builder mirrors the independent on/off switches the caller sets"
)]
pub struct LaunchOptionsBuilder {
    url: Option<String>,
    cdp_url: Option<Redacted>,
    headless: Option<bool>,
    cdp_target: Option<String>,

    use_chromium: bool,
    executable_path: Option<PathBuf>,
    incognito: bool,
    guest: bool,
    ad_block: bool,
    proxy: Option<Redacted>,
    user_data_dir: Option<PathBuf>,
    user_agent: Option<String>,
    lang: Option<String>,
    window_size: Option<(u32, u32)>,
    window_position: Option<(i32, i32)>,
    no_sandbox: bool,
    webrtc: Option<WebRtcPolicy>,
    shield_webrtc: bool,
    identity: Option<Arc<StealthPlan>>,
    extra_args: Vec<String>,
    extensions: Vec<PathBuf>,
    prefs: Map<String, Value>,
    #[cfg(feature = "plugins")]
    plugins: seleniumbase_rs_plugin::PluginSet,
    /// Plugins named rather than built, resolved by `build`.
    #[cfg(feature = "plugins")]
    selection: crate::config::plugins::PluginSelection,
    startup_timeout: Duration,
    debugging_port: u16,
    mobile: bool,
    device_metrics: Option<DeviceMetrics>,
    switches: BTreeSet<LaunchSwitch>,
    host_resolver_rules: Vec<String>,
    disable_features: Vec<String>,
    enable_features: Vec<String>,
    proxy_bypass_list: ProxyBypassList,
    proxy_pac_url: Option<Redacted>,
    /// Whether `proxy` came from a `BrowserConfig`, and is checked as Python
    /// checks a proxy string.
    checked_proxy: bool,
    virtual_display: Option<VirtualDisplay>,
}

impl Default for LaunchOptionsBuilder {
    fn default() -> Self {
        Self {
            url: None,
            cdp_url: None,
            headless: None,
            cdp_target: None,

            use_chromium: false,
            executable_path: None,
            incognito: false,
            guest: false,
            ad_block: false,
            proxy: None,
            user_data_dir: None,
            user_agent: None,
            lang: None,
            window_size: None,
            window_position: None,
            no_sandbox: std::env::var_os("SB_NO_SANDBOX").is_some(),
            webrtc: None,
            shield_webrtc: false,
            identity: None,
            extra_args: Vec::new(),
            extensions: Vec::new(),
            prefs: Map::new(),
            #[cfg(feature = "plugins")]
            plugins: seleniumbase_rs_plugin::PluginSet::new(),
            #[cfg(feature = "plugins")]
            selection: crate::config::plugins::PluginSelection::new(),
            startup_timeout: DEFAULT_STARTUP_TIMEOUT,
            debugging_port: 0,
            mobile: false,
            device_metrics: None,
            switches: BTreeSet::new(),
            host_resolver_rules: Vec::new(),
            disable_features: Vec::new(),
            enable_features: Vec::new(),
            proxy_bypass_list: ProxyBypassList::default(),
            proxy_pac_url: None,
            checked_proxy: false,
            virtual_display: None,
        }
    }
}

impl LaunchOptionsBuilder {
    /// Attaches instead of launching, reusing the existing connection pipeline.
    /// Accepts an HTTP debugger, a browser WebSocket, or an authenticated Tauri
    /// plugin URL. Launch-only settings cannot change an already-running app.
    /// The fingerprint and attach-compatible plugins apply to the windows the
    /// automation opens or picks, and to no other window of the app (see
    /// [`cdp_target`](Self::cdp_target)).
    ///
    /// The browser belongs to the app, so what cannot be applied to it is
    /// refused by [`build`](Self::build) with
    /// [`SeleniumBaseError::InvalidConfig`]: a proxy or PAC file (a
    /// fingerprint's proxy too), and a user agent or locale without a
    /// fingerprint, which applies both to the windows it prepares.
    ///
    /// ```no_run
    /// use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
    /// # async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    /// let options = LaunchOptions::builder().cdp_url("http://127.0.0.1:9222").build()?;
    /// let browser = Browser::launch(options).await?;
    /// browser.close().await?; // The external app keeps running.
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn cdp_url(mut self, url: impl Into<String>) -> Self {
        self.cdp_url = Some(Redacted(url.into()));
        self
    }

    /// Names the window to drive when attaching with
    /// [`cdp_url`](Self::cdp_url): a target id, or part of the window's URL.
    ///
    /// An app can own several windows, and only the one that is picked gets
    /// the fingerprint and the plugins. Without a target the first window is
    /// the one. Has no effect on a browser the library launches.
    ///
    /// ```no_run
    /// use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
    /// # async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    /// let options = LaunchOptions::builder()
    ///     .cdp_url("http://127.0.0.1:9222")
    ///     .cdp_target("/settings")
    ///     .build()?;
    /// let browser = Browser::launch(options).await?;
    /// let settings = browser.select_page("/settings").await?; // The other windows are left alone.
    /// # drop(settings);
    /// browser.close().await?;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn cdp_target(mut self, target: impl Into<String>) -> Self {
        self.cdp_target = Some(target.into());
        self
    }

    /// Sets Chrome's WebRTC IP-handling policy, the same flag a `Fingerprint`
    /// applies. Chrome's own behaviour is kept unless this is called.
    ///
    /// The flag limits which addresses are offered but does not stop a page
    /// seeing `.local` host candidates; for that, use
    /// [`shield_webrtc`](Self::shield_webrtc).
    #[must_use]
    pub fn webrtc_policy(mut self, policy: WebRtcPolicy) -> Self {
        self.webrtc = Some(policy);
        self
    }

    /// Makes every tab look like `fingerprint` describes.
    ///
    /// The user agent, language, window size, WebRTC policy and proxy it names
    /// become launch settings, and its script and protocol overrides are applied
    /// to every tab as it opens, before the first navigation. A setting made
    /// before this call wins over the fingerprint's; one made after it
    /// replaces it.
    ///
    /// Chrome flags listed in the fingerprint's `cmd_params` are not passed on:
    /// a flag such as `--renderer-cmd-prefix` runs a command, and a fingerprint
    /// often comes from a profile file. Add flags you trust with
    /// [`arg`](Self::arg).
    ///
    /// See [`Page::apply_fingerprint`](super::Page::apply_fingerprint) for what
    /// is applied and what is deliberately left out.
    #[must_use]
    pub fn fingerprint(mut self, fingerprint: &Fingerprint) -> Self {
        self.identity = Some(Arc::new(StealthPlan::new(fingerprint)));
        if self.user_agent.is_none() {
            self.user_agent = fingerprint.user_agent().value().cloned();
        }
        if self.lang.is_none() {
            self.lang = fingerprint.locale().value().cloned();
        }
        if self.window_size.is_none() {
            self.window_size = fingerprint
                .screen()
                .value()
                .map(|size| (size.width, size.height));
        }
        // Only a fingerprint that masks WebRTC passes the policy flag, which a
        // policy, an address or turning WebRTC off makes it; an empty one
        // leaves Chrome's own behaviour alone.
        if self.webrtc.is_none() && fingerprint.is_masked(Dimension::WebRtc) {
            self.webrtc = Some(fingerprint.policy().webrtc_policy.unwrap_or_default());
        }
        if self.proxy.is_none() {
            self.proxy = proxy_of(fingerprint).map(Redacted);
        }
        self
    }

    /// Makes WebRTC relay-only in every tab, so none gathers a candidate or
    /// contacts a STUN server. Pages that need a WebRTC call to connect will
    /// not. See [`Page::shield_webrtc`](super::Page::shield_webrtc).
    #[must_use]
    pub fn shield_webrtc(mut self, shield: bool) -> Self {
        self.shield_webrtc = shield;
        self
    }

    /// Navigates to `url` as soon as the browser is up.
    #[must_use]
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    /// Forces headless (`true`) or headed (`false`) mode.
    ///
    /// Unset, the browser is headless on Linux, where there is usually no
    /// display, and headed elsewhere.
    #[must_use]
    pub fn headless(mut self, headless: bool) -> Self {
        self.headless = Some(headless);
        self
    }

    /// Prefers Chromium over Google Chrome when locating the browser.
    #[must_use]
    pub fn use_chromium(mut self, use_chromium: bool) -> Self {
        self.use_chromium = use_chromium;
        self
    }

    /// Uses the browser at `path` instead of searching for one.
    #[must_use]
    pub fn executable_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.executable_path = Some(path.into());
        self
    }

    /// Launches in incognito mode.
    #[must_use]
    pub fn incognito(mut self, incognito: bool) -> Self {
        self.incognito = incognito;
        self
    }

    /// Launches in guest mode.
    #[must_use]
    pub fn guest(mut self, guest: bool) -> Self {
        self.guest = guest;
        self
    }

    /// Blocks requests to common advertising and tracking hosts.
    ///
    /// A basic block list applied through the protocol, not a full content
    /// blocker.
    #[must_use]
    pub fn ad_block(mut self, ad_block: bool) -> Self {
        self.ad_block = ad_block;
        self
    }

    /// Routes traffic through a proxy: `SERVER:PORT` or
    /// `USER:PASS@SERVER:PORT`.
    #[must_use]
    pub fn proxy(mut self, proxy: impl Into<String>) -> Self {
        self.proxy = Some(Redacted(proxy.into()));
        self
    }

    /// Keeps the browser profile in `dir` instead of a throwaway directory.
    #[must_use]
    pub fn user_data_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.user_data_dir = Some(dir.into());
        self
    }

    /// Overrides the `User-Agent` header.
    #[must_use]
    pub fn user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = Some(user_agent.into());
        self
    }

    /// Sets the browser UI and `Accept-Language` locale, such as `en-US`.
    #[must_use]
    pub fn lang(mut self, lang: impl Into<String>) -> Self {
        self.lang = Some(lang.into());
        self
    }

    /// Sets the initial window size in pixels.
    #[must_use]
    pub fn window_size(mut self, width: u32, height: u32) -> Self {
        self.window_size = Some((width, height));
        self
    }

    /// Sets the initial window position in pixels.
    #[must_use]
    pub fn window_position(mut self, x: i32, y: i32) -> Self {
        self.window_position = Some((x, y));
        self
    }

    /// Passes `--no-sandbox`, which Chrome needs when run as root in a
    /// container. Also enabled by setting the `SB_NO_SANDBOX` variable.
    #[must_use]
    pub fn no_sandbox(mut self, no_sandbox: bool) -> Self {
        self.no_sandbox = no_sandbox;
        self
    }

    /// Appends a raw Chrome command-line argument.
    #[must_use]
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.extra_args.push(arg.into());
        self
    }

    /// Loads the unpacked extension in `dir` once the browser runs.
    ///
    /// Branded Chrome ignores `--load-extension` since version 137, so the
    /// extension is loaded over the protocol (`Extensions.loadUnpacked`), which
    /// needs `--enable-unsafe-extension-debugging`; the launch adds that switch.
    /// A browser attached with [`Browser::connect`](super::Browser::connect)
    /// loads none.
    #[must_use]
    pub fn extension(mut self, dir: impl Into<PathBuf>) -> Self {
        self.extensions.push(dir.into());
        self
    }

    /// Sets a browser preference, as in Chrome's `Preferences` file, before
    /// the browser starts.
    ///
    /// A dotted `name` is a path, so `"intl.accept_languages"` and
    /// `"intl"` with `{"accept_languages": ...}` mean the same; dots inside the
    /// keys of an object `value` are kept. The preferences are merged into the
    /// profile's `Default/Preferences`, the one in
    /// [`user_data_dir`](Self::user_data_dir) included.
    #[must_use]
    pub fn preference(mut self, name: impl Into<String>, value: impl Into<Value>) -> Self {
        self.prefs.insert(name.into(), value.into());
        self
    }

    /// The plugins whose hooks run when the browser starts, for every tab it
    /// opens and when it closes (feature `plugins`, ADR 004).
    ///
    /// [`Browser::launch`](super::Browser::launch) checks the set, lets each
    /// plugin change the start-up settings (arguments, preferences, extensions,
    /// proxy, headless), starts the browser and calls `on_browser`; every tab
    /// the library attaches to gets the plugins' packages and `on_page_created`;
    /// [`Browser::close`](super::Browser::close) calls `on_close`. An error from
    /// any hook but `on_close` stops the launch or the tab.
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::plugin_api::{async_trait, PageHandle, Plugin, PluginSet, Result};
    /// use seleniumbase_rs::sb_cdp::LaunchOptions;
    ///
    /// struct Hello;
    ///
    /// #[async_trait]
    /// impl Plugin for Hello {
    ///     fn name(&self) -> &str {
    ///         "hello"
    ///     }
    ///
    ///     async fn on_page_created(&self, page: &dyn PageHandle) -> Result<()> {
    ///         page.add_init_script("console.log('hello from a plugin')").await
    ///     }
    /// }
    ///
    /// let plugins = PluginSet::new().with(Hello)?;
    /// let options = LaunchOptions::builder().headless(true).plugins(plugins).build()?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[cfg(feature = "plugins")]
    #[must_use]
    pub fn plugins(mut self, plugins: seleniumbase_rs_plugin::PluginSet) -> Self {
        self.plugins = plugins;
        self
    }

    /// Adds the built-in plugins called `names` (feature `plugins`), after the
    /// ones given to [`plugins`](Self::plugins).
    ///
    /// The names are the cargo features `plugin-shield`,
    /// `plugin-block-resources` and `plugin-crx` (`plugins-builtin` for all),
    /// resolved by [`plugin_host::builtin_registry`](crate::plugin_host::builtin_registry)
    /// when [`build`](Self::build) runs. Each plugin gets its default options;
    /// use [`plugin_selection`](Self::plugin_selection) to give options.
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::sb_cdp::LaunchOptions;
    ///
    /// // A name this build does not have is an error, and says which it has.
    /// let error = LaunchOptions::builder()
    ///     .plugins_named(["no-such-plugin"])
    ///     .build()
    ///     .unwrap_err();
    /// assert!(error.to_string().contains("no-such-plugin"));
    /// ```
    #[cfg(feature = "plugins")]
    #[must_use]
    pub fn plugins_named<I, S>(mut self, names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for name in names {
            self.selection = std::mem::take(&mut self.selection).enable(name);
        }
        self
    }

    /// Adds the plugins a [`PluginSelection`](crate::config::plugins::PluginSelection)
    /// names, with their options (feature `plugins`): the `[plugins]` table of
    /// the settings file and `SB_PLUGINS`, as `Settings::plugins` holds them.
    ///
    /// [`build`](Self::build) reports an unknown name, options a plugin
    /// refuses and a plugin named twice as [`SeleniumBaseError::Plugin`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use seleniumbase_rs::config::settings::Settings;
    /// use seleniumbase_rs::sb_cdp::LaunchOptions;
    ///
    /// # fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    /// let settings = Settings::load()?;
    /// let options = LaunchOptions::builder()
    ///     .plugin_selection(&settings.plugins)
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "plugins")]
    #[must_use]
    pub fn plugin_selection(mut self, selection: &crate::config::plugins::PluginSelection) -> Self {
        self.selection.extend(selection);
        self
    }

    /// Sets how long to wait for the browser to become reachable.
    #[must_use]
    pub fn startup_timeout(mut self, timeout: Duration) -> Self {
        self.startup_timeout = timeout;
        self
    }

    /// Makes Chrome listen for the DevTools Protocol on `port` of `127.0.0.1`
    /// instead of a free port it picks (`0`, the default).
    ///
    /// A fixed port lets other DevTools clients, such as `chrome-devtools-mcp
    /// --browserUrl http://127.0.0.1:9222`, find the browser. A port something
    /// already listens on is refused before Chrome starts. Chrome publishes the
    /// `DevToolsActivePort` file only for a port it picks, so on a fixed port
    /// the launch reads the endpoint from `/json/version`. Chrome refuses the
    /// endpoint on its default profile directory, so keep
    /// [`user_data_dir`](Self::user_data_dir) unset or pointed at a profile of
    /// its own.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::Path;
    /// use seleniumbase_rs::sb_cdp::LaunchOptions;
    ///
    /// let options = LaunchOptions::builder().debugging_port(9222).build()?;
    /// let args = options.browser_args(Path::new("/tmp/profile"));
    /// assert!(args.contains(&"--remote-debugging-port=9222".to_owned()));
    /// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
    /// ```
    #[must_use]
    pub fn debugging_port(mut self, port: u16) -> Self {
        self.debugging_port = port;
        self
    }

    /// Checks the combination and produces the options.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] when `incognito` and
    /// `guest` are both set, when `use_chromium` is combined with an explicit
    /// executable path, or when the proxy cannot be parsed, and
    /// [`SeleniumBaseError::Plugin`] when a plugin named with
    /// [`plugins_named`](Self::plugins_named) or
    /// [`plugin_selection`](Self::plugin_selection) is unknown, refuses its
    /// options or is named twice.
    pub fn build(self) -> Result<LaunchOptions, SeleniumBaseError> {
        if self.identity.is_some() && (self.mobile || self.device_metrics.is_some()) {
            return Err(SeleniumBaseError::invalid_config(
                "mobile emulation and a fingerprint both set the screen and the user agent; \
                 use one of them",
            ));
        }
        if self.cdp_url.is_some() {
            self.check_attachable()?;
        }
        let pac = self
            .proxy_pac_url
            .as_ref()
            .map(|text| text.0.parse::<PacUrl>())
            .transpose()?;

        if self.incognito && self.guest {
            return Err(SeleniumBaseError::invalid_config(
                "incognito and guest mode cannot be combined",
            ));
        }
        if self.use_chromium && self.executable_path.is_some() {
            return Err(SeleniumBaseError::invalid_config(
                "use_chromium and an explicit executable path are mutually exclusive",
            ));
        }
        let proxy = self
            .proxy
            .as_ref()
            .map(|proxy| bridge::parse_proxy(&proxy.0, self.checked_proxy))
            .transpose()?;
        #[cfg(feature = "plugins")]
        let this = self.with_named_plugins()?;
        #[cfg(not(feature = "plugins"))]
        let this = self;
        Ok(this.assemble(proxy, pac))
    }

    /// Refuses what a browser that is only attached to cannot be given.
    ///
    /// The browser is the app's. A proxy would need a launch flag (and a login
    /// would turn on request interception in every tab with nothing to answer
    /// it, hanging the app's own requests). A user agent or locale given as a
    /// launch flag would be dropped without a word, so it is only taken when a
    /// fingerprint (or a phone, for the user agent) applies it to the tabs that
    /// are prepared.
    fn check_attachable(&self) -> Result<(), SeleniumBaseError> {
        if self.proxy.is_some() || self.proxy_pac_url.is_some() {
            return Err(SeleniumBaseError::invalid_config(
                "a proxy cannot be set on an attached browser: it belongs to the app, whose \
                 network settings are not changed (this covers a PAC file and the proxy of a \
                 fingerprint); set the proxy where the app starts its browser",
            ));
        }
        let phone = self.mobile || self.device_metrics.is_some();
        let unapplied_agent = self.user_agent.is_some() && !phone;
        if self.identity.is_none() && (unapplied_agent || self.lang.is_some()) {
            return Err(SeleniumBaseError::invalid_config(
                "a user agent or locale needs a fingerprint on an attached browser: they are \
                 launch flags, which cannot reach a running app, and a fingerprint applies \
                 both to the windows it prepares",
            ));
        }
        Ok(())
    }

    /// Adds the plugins named with `plugins_named` and `plugin_selection` to
    /// the set.
    #[cfg(feature = "plugins")]
    fn with_named_plugins(mut self) -> Result<Self, SeleniumBaseError> {
        if !self.selection.is_empty() {
            self.plugins = crate::plugin_host::add_selection(
                std::mem::take(&mut self.plugins),
                &crate::plugin_host::builtin_registry(),
                &self.selection,
            )?;
        }
        Ok(self)
    }

    /// Converts the builder to options without re-checking the combination.
    fn assemble(self, proxy: Option<Proxy>, pac: Option<PacUrl>) -> LaunchOptions {
        // The phone, when one is asked for, and the agent the browser starts with.
        let device = emulated_phone(self.mobile, self.device_metrics, self.user_agent.as_deref())
            .map(|(metrics, agent)| (metrics, agent.map(str::to_owned)));
        let user_agent = self
            .user_agent
            .or_else(|| self.mobile.then(|| MOBILE_USER_AGENT.to_owned()));
        // What the caller sets wins over what a switch implies.
        let mut prefs = chrome_preferences(&self.switches);
        prefs.extend(self.prefs);
        let mut enable_features = self.enable_features;
        if self.switches.contains(&LaunchSwitch::DarkMode)
            && !enable_features
                .iter()
                .any(|feature| feature == "WebContentsForceDark")
        {
            enable_features.push("WebContentsForceDark".to_owned());
        }

        let source = match (self.executable_path, self.use_chromium) {
            (Some(path), _) => BrowserSource::Path(path),
            (None, true) => BrowserSource::Chromium,
            (None, false) => BrowserSource::Chrome,
        };
        let mode = if self.incognito {
            ProfileMode::Incognito
        } else if self.guest {
            ProfileMode::Guest
        } else {
            ProfileMode::Standard
        };
        LaunchOptions {
            url: self.url,
            cdp_url: self.cdp_url,
            cdp_target: self.cdp_target,

            headless: self.headless,
            source,
            mode,
            ad_block: self.ad_block,
            proxy,
            user_data_dir: self.user_data_dir,
            user_agent,
            lang: self.lang,
            window_size: self.window_size,
            window_position: self.window_position,
            no_sandbox: self.no_sandbox,
            webrtc: self.webrtc,
            shield_webrtc: self.shield_webrtc,
            identity: self.identity,
            extra_args: self.extra_args,
            extensions: self.extensions,
            prefs,
            host_resolver_rules: self.host_resolver_rules,
            disable_features: self.disable_features,
            enable_features,
            disable_quic: false,
            #[cfg(feature = "plugins")]
            plugins: self.plugins,
            startup_timeout: self.startup_timeout,
            debugging_port: self.debugging_port,
            switches: self.switches,
            device,
            proxy_bypass_list: self.proxy_bypass_list,
            proxy_pac_url: pac,
            virtual_display: self.virtual_display,
        }
    }
}

/// A running browser and the DevTools endpoint it published.
pub(crate) struct LaunchedBrowser {
    pub(crate) child: Child,
    pub(crate) port: u16,
    pub(crate) ws_url: String,
    /// Keeps a throwaway profile alive until the browser is dropped.
    pub(crate) profile: Option<TempDir>,
    /// Ends the browser if this program ends without closing it.
    pub(crate) guard: OrphanGuard,
    /// The virtual screen the browser's window is on, when one was started.
    pub(crate) display: Option<XvfbProcess>,
}

/// Finds a browser binary according to `options`.
fn resolve_binary(options: &LaunchOptions) -> Result<PathBuf, SeleniumBaseError> {
    match &options.source {
        BrowserSource::Path(path) => {
            if path.exists() {
                Ok(path.clone())
            } else {
                Err(SeleniumBaseError::browser_launch(
                    path.display().to_string(),
                    "the executable does not exist",
                ))
            }
        }
        BrowserSource::Chromium => {
            let on_path = ["chromium", "chromium-browser", "chrome-headless-shell"]
                .into_iter()
                .find_map(|name| which::which(name).ok());
            let app_bundle = Path::new("/Applications/Chromium.app/Contents/MacOS/Chromium");
            on_path
                .or_else(|| app_bundle.exists().then(|| app_bundle.to_path_buf()))
                .ok_or_else(|| {
                    SeleniumBaseError::browser_launch(
                        "chromium",
                        "no Chromium was found; install it or set executable_path",
                    )
                })
        }
        BrowserSource::Chrome => find_system_chrome().ok_or_else(|| {
            SeleniumBaseError::browser_launch(
                "google-chrome",
                "no Chrome was found; install it, enable use_chromium, or set executable_path",
            )
        }),
    }
}

/// Starts the virtual screen the options ask for, where it can run: a headed
/// browser on Linux. Anywhere else the request is left out, and says so.
async fn start_virtual_display(
    options: &LaunchOptions,
) -> Result<Option<XvfbProcess>, SeleniumBaseError> {
    let Some(screen) = options.virtual_display else {
        return Ok(None);
    };
    if !virtual_display::wanted(
        Some(screen),
        options.is_headless(),
        cfg!(target_os = "linux"),
    ) {
        tracing::debug!("no virtual display is started: it needs a headed browser on Linux");
        return Ok(None);
    }
    tokio::task::spawn_blocking(move || XvfbProcess::start(screen))
        .await
        .map_err(|error| SeleniumBaseError::browser_launch("Xvfb", error.to_string()))?
        .map(Some)
}

/// Starts the browser and waits for it to publish its DevTools endpoint.
///
/// Chrome writes the chosen port and browser WebSocket path to
/// `DevToolsActivePort` inside the profile directory, which is more reliable
/// than parsing its log output.
pub(crate) async fn launch(options: &LaunchOptions) -> Result<LaunchedBrowser, SeleniumBaseError> {
    let binary = resolve_binary(options)?;
    let binary_name = binary.display().to_string();

    let (profile_dir, profile) = if let Some(dir) = &options.user_data_dir {
        (dir.clone(), None)
    } else {
        let tmp = tempfile::Builder::new().prefix("sb-cdp-").tempdir()?;
        (tmp.path().to_path_buf(), Some(tmp))
    };
    write_profile_preferences(&profile_dir, profile.is_some(), &options.prefs)?;
    let endpoint_file = profile_dir.join("DevToolsActivePort");
    // A stale file from an earlier run would be read as the new endpoint.
    let _ = std::fs::remove_file(&endpoint_file);
    // Chrome writes `DevToolsActivePort` only for a port it picks itself; on a
    // fixed port the endpoint is read from `/json/version` instead, so that
    // port must not already belong to another program's endpoint.
    let fixed_port = (options.debugging_port != 0).then_some(options.debugging_port);
    let probe = match fixed_port {
        Some(port) => {
            std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).map_err(
                |error| {
                    SeleniumBaseError::browser_launch(
                        binary_name.clone(),
                        format!("the DevTools port {port} on 127.0.0.1 is not free: {error}"),
                    )
                },
            )?;
            crate::install_tls_provider();
            Some(
                reqwest::Client::builder()
                    .timeout(VERSION_PROBE_TIMEOUT)
                    .build()
                    .map_err(|error| {
                        SeleniumBaseError::browser_launch(binary_name.clone(), error.to_string())
                    })?,
            )
        }
        None => None,
    };

    // A headed browser on a virtual screen, where the machine has none.
    let display = start_virtual_display(options).await?;

    let guard = OrphanGuard::start();
    let mut command = Command::new(&binary);
    command
        .args(options.browser_args(&profile_dir))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    if let Some(display) = &display {
        command.env("DISPLAY", display.display());
    }

    guard.enroll_tokio(&mut command);
    let mut child = command
        .spawn()
        .map_err(|e| SeleniumBaseError::browser_launch(binary_name.clone(), e.to_string()))?;

    let deadline = Instant::now() + options.startup_timeout;
    loop {
        let endpoint = match (fixed_port, &probe) {
            (Some(port), Some(probe)) => advertised_endpoint(probe, port).await,
            _ => std::fs::read_to_string(&endpoint_file)
                .ok()
                .and_then(|content| parse_endpoint(&content)),
        };
        if let Some((port, path)) = endpoint {
            return Ok(LaunchedBrowser {
                child,
                port,
                ws_url: format!("ws://127.0.0.1:{port}{path}"),
                profile,
                guard,
                display,
            });
        }
        if let Ok(Some(status)) = child.try_wait() {
            return Err(SeleniumBaseError::browser_launch(
                binary_name,
                format!("the browser exited during startup ({status})"),
            ));
        }
        if Instant::now() >= deadline {
            let _ = child.start_kill();
            return Err(SeleniumBaseError::browser_launch(
                binary_name,
                format!(
                    "no DevTools endpoint appeared within {:?}",
                    options.startup_timeout
                ),
            ));
        }
        tokio::time::sleep(STARTUP_POLL_INTERVAL).await;
    }
}

/// Merges `prefs` into the profile's `Default/Preferences`, which Chrome reads
/// when it starts.
///
/// A dotted top-level name is a path into the file; objects are merged key by
/// key, so what the profile already holds stays unless a preference replaces
/// it. A file that is not a JSON object is left alone and reported.
pub(crate) fn write_preferences(
    profile_dir: &Path,
    prefs: &Map<String, Value>,
) -> Result<(), SeleniumBaseError> {
    let dir = profile_dir.join("Default");
    std::fs::create_dir_all(&dir)?;
    let file = dir.join("Preferences");
    let mut current = match std::fs::read_to_string(&file) {
        Ok(text) => match serde_json::from_str::<Value>(&text) {
            Ok(Value::Object(map)) => map,
            _ => {
                return Err(SeleniumBaseError::invalid_config(format!(
                    "{} is not a JSON object, so the preferences were not written into it",
                    file.display()
                )))
            }
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Map::new(),
        Err(error) => return Err(error.into()),
    };
    for (name, value) in prefs {
        let path: Vec<&str> = name.split('.').collect();
        merge_at(&mut current, &path, value);
    }
    std::fs::write(&file, serde_json::to_vec(&Value::Object(current))?)?;
    Ok(())
}

/// Writes the preferences a browser starts with into `profile_dir`.
///
/// `own_profile` says the profile is a throwaway one this launch made. Such a
/// profile first gets the library's defaults, which keep Chrome's password
/// prompts from taking the keyboard from the page (see
/// [`password_prompts`](crate::browser::password_prompts)); then `prefs`, the
/// ones the caller set, which win. A profile the caller named is the caller's:
/// only `prefs` go into it, and nothing is written when there are none.
pub(crate) fn write_profile_preferences(
    profile_dir: &Path,
    own_profile: bool,
    prefs: &Map<String, Value>,
) -> Result<(), SeleniumBaseError> {
    if own_profile {
        write_preferences(
            profile_dir,
            &crate::browser::password_prompts::preferences(),
        )?;
    }
    if !prefs.is_empty() {
        write_preferences(profile_dir, prefs)?;
    }

    Ok(())
}

/// Sets the preference at `path` in `target` to `value`, making the objects on
/// the way where they are missing.
fn merge_at(target: &mut Map<String, Value>, path: &[&str], value: &Value) {
    match path {
        [] => {}
        [last] => merge_value(target, last, value),
        [first, rest @ ..] => {
            let entry = target
                .entry((*first).to_owned())
                .or_insert_with(|| Value::Object(Map::new()));
            if !entry.is_object() {
                *entry = Value::Object(Map::new());
            }
            if let Value::Object(next) = entry {
                merge_at(next, rest, value);
            }
        }
    }
}

/// Sets `key` in `target` to `value`, merging two objects key by key.
fn merge_value(target: &mut Map<String, Value>, key: &str, value: &Value) {
    match (target.get_mut(key), value) {
        (Some(Value::Object(existing)), Value::Object(incoming)) => {
            for (inner, inner_value) in incoming {
                merge_value(existing, inner, inner_value);
            }
        }
        _ => {
            target.insert(key.to_owned(), value.clone());
        }
    }
}

/// The browser endpoint Chrome advertises on a fixed `port`: the port and the
/// WebSocket path of its `/json/version`, once it answers there.
async fn advertised_endpoint(probe: &reqwest::Client, port: u16) -> Option<(u16, String)> {
    let version: Value = probe
        .get(format!("http://127.0.0.1:{port}/json/version"))
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;
    parse_version(port, &version)
}

/// The path of the `webSocketDebuggerUrl` in a `/json/version` document, when
/// that address is on `port`.
fn parse_version(port: u16, version: &Value) -> Option<(u16, String)> {
    let url = version["webSocketDebuggerUrl"].as_str()?;
    let (authority, path) = url.strip_prefix("ws://")?.split_once('/')?;
    let advertised: u16 = authority.rsplit_once(':')?.1.parse().ok()?;
    (advertised == port).then(|| (port, format!("/{path}")))
}

/// Parses `DevToolsActivePort`: the port on the first line, the browser
/// WebSocket path on the second.
fn parse_endpoint(content: &str) -> Option<(u16, String)> {
    let mut lines = content.lines();
    let port = lines.next()?.trim().parse().ok()?;
    let path = lines.next()?.trim();
    path.starts_with('/').then(|| (port, path.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn args_for(builder: LaunchOptionsBuilder) -> Vec<String> {
        builder
            .build()
            .unwrap()
            .browser_args(Path::new("/tmp/profile"))
    }

    #[cfg(feature = "plugins")]
    #[test]
    fn an_unknown_plugin_name_stops_the_build_and_names_what_is_known() {
        let error = LaunchOptions::builder()
            .plugins_named(["no-such-plugin"])
            .build()
            .unwrap_err();

        assert!(
            matches!(&error, SeleniumBaseError::Plugin { plugin, .. } if plugin == "no-such-plugin"),
            "{error:?}"
        );
        assert!(error.to_string().contains("known:"), "{error}");
    }

    #[cfg(feature = "plugin-shield")]
    #[test]
    fn plugins_named_and_a_selection_become_the_plugins_of_the_options() {
        use crate::config::plugins::PluginSelection;

        let named = LaunchOptions::builder()
            .plugins_named(["shield"])
            .build()
            .unwrap();
        assert_eq!(named.plugins.names(), ["shield"]);

        let selection = PluginSelection::new()
            .enable("shield")
            .with_options("shield", json!({ "level": "strict" }));
        let selected = LaunchOptions::builder()
            .plugin_selection(&selection)
            .build()
            .unwrap();
        assert_eq!(selected.plugins.names(), ["shield"]);

        let typo = PluginSelection::new()
            .enable("shield")
            .with_options("shield", json!({ "lvel": "strict" }));
        let error = LaunchOptions::builder()
            .plugin_selection(&typo)
            .build()
            .unwrap_err();
        assert!(
            matches!(&error, SeleniumBaseError::Plugin { plugin, .. } if plugin == "shield"),
            "{error:?}"
        );

        let twice = LaunchOptions::builder()
            .plugins_named(["shield"])
            .plugin_selection(&PluginSelection::new().enable("shield"))
            .build()
            .unwrap_err();
        assert!(twice.to_string().contains("already in the set"), "{twice}");
    }

    #[cfg(feature = "plugin-shield")]
    #[test]
    fn named_plugins_come_after_the_set_given_to_plugins() {
        use seleniumbase_rs_plugin::{async_trait, Plugin, PluginSet};

        struct Mine;

        #[async_trait]
        impl Plugin for Mine {
            fn name(&self) -> &'static str {
                "mine"
            }
        }

        let options = LaunchOptions::builder()
            .plugins(PluginSet::new().with(Mine).unwrap())
            .plugins_named(["shield"])
            .build()
            .unwrap();

        assert_eq!(options.plugins.names(), ["mine", "shield"]);
    }

    #[test]
    fn incompatible_modes_are_rejected_at_build_time() {
        assert!(LaunchOptions::builder()
            .incognito(true)
            .guest(true)
            .build()
            .is_err());
        assert!(LaunchOptions::builder()
            .use_chromium(true)
            .executable_path("/usr/bin/chromium")
            .build()
            .is_err());
    }

    #[test]
    fn the_endpoint_on_a_fixed_port_is_read_from_its_version_document() {
        let version = json!({
            "Browser": "Chrome/155.0.8059.40",
            "webSocketDebuggerUrl": "ws://127.0.0.1:9222/devtools/browser/abc-123"
        });
        assert_eq!(
            parse_version(9222, &version),
            Some((9222, "/devtools/browser/abc-123".to_owned()))
        );
        // Another port's browser, or no address at all, is not this endpoint.
        assert_eq!(parse_version(9333, &version), None);
        assert_eq!(parse_version(9222, &json!({})), None);
        assert_eq!(
            parse_version(
                9222,
                &json!({ "webSocketDebuggerUrl": "http://127.0.0.1:9222/x" })
            ),
            None
        );
    }

    #[tokio::test]
    async fn a_taken_fixed_port_is_refused_before_anything_starts() {
        let taken = std::net::TcpListener::bind("127.0.0.1:0").expect("a port");
        let port = taken.local_addr().expect("address").port();
        // A binary that exists, so only the port can stop the launch.
        let options = LaunchOptions::builder()
            .executable_path(std::env::current_exe().expect("this test binary"))
            .debugging_port(port)
            .build()
            .expect("options");
        let error = launch(&options).await.err().expect("the port is taken");
        assert!(
            error.to_string().contains(&format!("port {port}")),
            "{error}"
        );
    }

    #[test]
    fn chrome_picks_a_free_debugging_port_unless_one_is_given() {
        let picked = args_for(LaunchOptions::builder());
        assert_eq!(picked[0], "--remote-debugging-port=0");
        let fixed = args_for(LaunchOptions::builder().debugging_port(9222));
        assert_eq!(fixed[0], "--remote-debugging-port=9222");
        assert_eq!(
            fixed
                .iter()
                .filter(|arg| arg.starts_with("--remote-debugging-port="))
                .count(),
            1,
            "one port switch: {fixed:?}"
        );
    }

    #[test]
    fn an_unparseable_proxy_is_rejected_at_build_time() {
        assert!(LaunchOptions::builder().proxy("").build().is_err());
        assert!(LaunchOptions::builder()
            .proxy("user:pass@")
            .build()
            .is_err());
    }

    #[test]
    fn headless_defaults_to_linux_only_and_can_be_overridden() {
        assert_eq!(
            LaunchOptions::default().is_headless(),
            cfg!(target_os = "linux")
        );
        assert!(LaunchOptions::builder()
            .headless(true)
            .build()
            .unwrap()
            .is_headless());
        assert!(!LaunchOptions::builder()
            .headless(false)
            .build()
            .unwrap()
            .is_headless());
    }

    #[test]
    fn each_option_reaches_the_command_line() {
        let args = args_for(
            LaunchOptions::builder()
                .headless(true)
                .incognito(true)
                .no_sandbox(true)
                .lang("de-DE")
                .window_size(1280, 800)
                .arg("--mute-audio"),
        );
        for expected in [
            "--user-data-dir=/tmp/profile",
            "--headless=new",
            "--incognito",
            "--no-sandbox",
            "--lang=de-DE",
            "--window-size=1280,800",
            "--mute-audio",
        ] {
            assert!(args.iter().any(|a| a == expected), "missing {expected}");
        }
        assert!(
            !args.iter().any(|a| a == "--guest"),
            "guest mode was never requested"
        );
        assert_eq!(args.last().map(String::as_str), Some("about:blank"));
    }

    #[test]
    fn a_headed_launch_has_no_headless_flag() {
        let args = args_for(LaunchOptions::builder().headless(false));
        assert!(!args.iter().any(|a| a.starts_with("--headless")));
    }

    #[test]
    fn proxy_credentials_never_reach_the_command_line() {
        let args = args_for(LaunchOptions::builder().proxy("alice:s3cret@proxy.example.com:8080"));
        assert!(args
            .iter()
            .any(|a| a == "--proxy-server=proxy.example.com:8080"));
        assert!(!args
            .iter()
            .any(|a| a.contains("s3cret") || a.contains("alice")));
    }

    #[test]
    fn credentials_are_split_from_the_server_and_scheme_is_kept() {
        let plain = Proxy::parse("proxy.example.com:8080").unwrap();
        assert_eq!(plain.server(), "proxy.example.com:8080");
        assert!(!plain.has_credentials());

        let socks = Proxy::parse("socks5://u:p@10.0.0.1:1080").unwrap();
        assert_eq!(socks.server(), "socks5://10.0.0.1:1080");
        assert_eq!(socks.credentials(), Some(("u", "p")));
    }

    #[test]
    fn debug_and_display_never_reveal_credentials() {
        let proxy = Proxy::parse("alice:s3cret@proxy.example.com:8080").unwrap();
        let options = LaunchOptions::builder()
            .proxy("alice:s3cret@proxy.example.com:8080")
            .build()
            .unwrap();
        for rendered in [
            format!("{proxy:?}"),
            format!("{proxy}"),
            format!("{options:?}"),
        ] {
            assert!(
                !rendered.contains("s3cret"),
                "leaked the password: {rendered}"
            );
            assert!(
                !rendered.contains("alice"),
                "leaked the username: {rendered}"
            );
        }
    }

    #[test]
    fn endpoint_files_are_read_and_malformed_ones_rejected() {
        assert_eq!(
            parse_endpoint("43211\n/devtools/browser/abc-123\n"),
            Some((43211, "/devtools/browser/abc-123".to_owned()))
        );
        for bad in [
            "",
            "not-a-port\n/devtools/browser/x",
            "9222\n",
            "9222\nno-slash",
        ] {
            assert_eq!(parse_endpoint(bad), None, "accepted {bad:?}");
        }
    }

    #[test]
    fn an_extension_turns_on_loading_it_over_the_protocol() {
        let plain = args_for(LaunchOptions::builder());
        assert!(!plain
            .iter()
            .any(|a| a == "--enable-unsafe-extension-debugging"));

        let with = args_for(LaunchOptions::builder().extension("/opt/extensions/helper"));
        assert!(with
            .iter()
            .any(|a| a == "--enable-unsafe-extension-debugging"));
        assert!(
            !with.iter().any(|a| a.starts_with("--load-extension")),
            "branded Chrome ignores the switch, so the protocol loads it"
        );
    }

    fn read_preferences(profile: &Path) -> Value {
        let text = std::fs::read_to_string(profile.join("Default").join("Preferences")).unwrap();
        serde_json::from_str(&text).unwrap()
    }

    #[test]
    fn preferences_are_merged_into_the_profiles_own() {
        let profile = tempfile::tempdir().unwrap();
        let default = profile.path().join("Default");
        std::fs::create_dir_all(&default).unwrap();
        std::fs::write(
            default.join("Preferences"),
            r#"{"profile":{"name":"Work","exit_type":"Normal"},"keep":1}"#,
        )
        .unwrap();
        let options = LaunchOptions::builder()
            .preference("profile.exit_type", "Crashed")
            .preference("webkit", json!({ "webprefs": { "default_font_size": 22 } }))
            .preference(
                "profile",
                json!({ "content_settings": { "exceptions": { "https://a.test:443,*": 1 } } }),
            )
            .build()
            .unwrap();

        write_preferences(profile.path(), &options.prefs).unwrap();

        let written = read_preferences(profile.path());
        assert_eq!(written["keep"], 1, "what was there stays");
        assert_eq!(written["profile"]["name"], "Work");
        assert_eq!(
            written["profile"]["exit_type"], "Crashed",
            "a dotted name is a path"
        );
        assert_eq!(written["webkit"]["webprefs"]["default_font_size"], 22);
        assert_eq!(
            written["profile"]["content_settings"]["exceptions"]["https://a.test:443,*"], 1,
            "dots inside a nested key are part of the key"
        );
    }

    #[test]
    fn a_fresh_profile_gets_preferences_and_a_damaged_file_is_left_alone() {
        let mut prefs = Map::new();
        prefs.insert("intl.accept_languages".to_owned(), json!("de-DE"));

        let fresh = tempfile::tempdir().unwrap();
        write_preferences(fresh.path(), &prefs).unwrap();
        assert_eq!(
            read_preferences(fresh.path()),
            json!({ "intl": { "accept_languages": "de-DE" } })
        );

        let damaged = tempfile::tempdir().unwrap();
        let file = damaged.path().join("Default").join("Preferences");
        std::fs::create_dir_all(damaged.path().join("Default")).unwrap();
        std::fs::write(&file, "not json").unwrap();
        let error = write_preferences(damaged.path(), &prefs).unwrap_err();
        assert!(
            matches!(error, SeleniumBaseError::InvalidConfig(_)),
            "{error:?}"
        );
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "not json");
    }

    fn password_prompts_are_off(written: &Value) -> bool {
        written["profile"]["password_manager_leak_detection"] == false
            && written["profile"]["password_manager_enabled"] == false
            && written["credentials_enable_service"] == false
    }

    #[test]
    fn a_throwaway_profile_starts_with_the_password_prompts_off() {
        let profile = tempfile::tempdir().unwrap();
        write_profile_preferences(profile.path(), true, &Map::new()).unwrap();
        let written = read_preferences(profile.path());
        assert!(password_prompts_are_off(&written), "{written}");
    }

    #[test]
    fn the_callers_preferences_win_over_the_defaults_and_sit_beside_them() {
        let profile = tempfile::tempdir().unwrap();
        let options = LaunchOptions::builder()
            .preference("profile.password_manager_leak_detection", true)
            .preference("intl.accept_languages", "de-DE")
            .build()
            .unwrap();
        write_profile_preferences(profile.path(), true, &options.prefs).unwrap();
        let written = read_preferences(profile.path());
        assert_eq!(written["profile"]["password_manager_leak_detection"], true);
        assert_eq!(written["profile"]["password_manager_enabled"], false);
        assert_eq!(written["credentials_enable_service"], false);
        assert_eq!(written["intl"]["accept_languages"], "de-DE");
    }

    #[test]
    fn a_profile_the_caller_named_is_left_as_it_is() {
        let profile = tempfile::tempdir().unwrap();
        write_profile_preferences(profile.path(), false, &Map::new()).unwrap();
        assert!(
            !profile.path().join("Default").join("Preferences").exists(),
            "nothing is written into a profile the caller owns"
        );

        let mut prefs = Map::new();
        prefs.insert("intl.accept_languages".to_owned(), json!("de-DE"));
        write_profile_preferences(profile.path(), false, &prefs).unwrap();
        let written = read_preferences(profile.path());
        assert_eq!(written, json!({ "intl": { "accept_languages": "de-DE" } }));
    }

    #[test]
    fn a_missing_explicit_executable_is_a_launch_error() {
        let options = LaunchOptions::builder()
            .executable_path("/definitely/not/a/browser")
            .build()
            .unwrap();
        assert!(matches!(
            resolve_binary(&options),
            Err(SeleniumBaseError::BrowserLaunch { .. })
        ));
    }
}
