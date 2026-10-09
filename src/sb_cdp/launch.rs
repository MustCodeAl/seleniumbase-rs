//! Describing and starting a Chrome process with a DevTools endpoint.

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use tempfile::TempDir;
use tokio::process::{Child, Command};

use crate::error::SeleniumBaseError;
use crate::stealth::fingerprint::WebRtcPolicy;
use crate::stealth::patcher::find_system_chrome;

/// How long a freshly launched browser may take to publish its endpoint.
///
/// Cold starts on a busy CI machine routinely take several seconds; twenty
/// leaves ample margin without hiding a browser that has genuinely hung.
const DEFAULT_STARTUP_TIMEOUT: Duration = Duration::from_secs(20);

/// How often to look for the endpoint file while the browser starts.
const STARTUP_POLL_INTERVAL: Duration = Duration::from_millis(50);

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

    pub(crate) fn credentials(&self) -> Option<(&str, &str)> {
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
pub struct LaunchOptions {
    pub(crate) url: Option<String>,
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
    pub(crate) extra_args: Vec<String>,
    pub(crate) startup_timeout: Duration,
}

impl Default for LaunchOptions {
    fn default() -> Self {
        LaunchOptionsBuilder::default().assemble(None)
    }
}

impl LaunchOptions {
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

    /// The command-line arguments Chrome will be started with.
    ///
    /// `profile_dir` is the profile directory in use, which may be a
    /// throwaway temporary one.
    pub(crate) fn browser_args(&self, profile_dir: &Path) -> Vec<String> {
        let mut args = vec![
            "--remote-debugging-port=0".to_owned(),
            format!("--user-data-dir={}", profile_dir.display()),
            "--no-first-run".to_owned(),
            "--no-default-browser-check".to_owned(),
            "--disable-search-engine-choice-screen".to_owned(),
            "--disable-features=Translate".to_owned(),
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
        if let Some(proxy) = &self.proxy {
            // Chrome ignores credentials here; they are supplied over the
            // protocol when the proxy asks for them.
            args.push(format!("--proxy-server={}", proxy.server()));
        }
        if let Some(user_agent) = &self.user_agent {
            args.push(format!("--user-agent={user_agent}"));
        }
        if let Some(lang) = &self.lang {
            args.push(format!("--lang={lang}"));
        }
        if let Some((width, height)) = self.window_size {
            args.push(format!("--window-size={width},{height}"));
        }
        if let Some((x, y)) = self.window_position {
            args.push(format!("--window-position={x},{y}"));
        }
        args.extend(self.extra_args.iter().cloned());
        // The first page the browser opens.
        args.push("about:blank".to_owned());
        args
    }
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
    headless: Option<bool>,
    use_chromium: bool,
    executable_path: Option<PathBuf>,
    incognito: bool,
    guest: bool,
    ad_block: bool,
    proxy: Option<String>,
    user_data_dir: Option<PathBuf>,
    user_agent: Option<String>,
    lang: Option<String>,
    window_size: Option<(u32, u32)>,
    window_position: Option<(i32, i32)>,
    no_sandbox: bool,
    webrtc: Option<WebRtcPolicy>,
    shield_webrtc: bool,
    extra_args: Vec<String>,
    startup_timeout: Duration,
}

impl Default for LaunchOptionsBuilder {
    fn default() -> Self {
        Self {
            url: None,
            headless: None,
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
            extra_args: Vec::new(),
            startup_timeout: DEFAULT_STARTUP_TIMEOUT,
        }
    }
}

impl LaunchOptionsBuilder {
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
        self.proxy = Some(proxy.into());
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

    /// Sets how long to wait for the browser to become reachable.
    #[must_use]
    pub fn startup_timeout(mut self, timeout: Duration) -> Self {
        self.startup_timeout = timeout;
        self
    }

    /// Checks the combination and produces the options.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] when `incognito` and
    /// `guest` are both set, when `use_chromium` is combined with an explicit
    /// executable path, or when the proxy cannot be parsed.
    pub fn build(self) -> Result<LaunchOptions, SeleniumBaseError> {
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
        let proxy = self.proxy.as_deref().map(Proxy::parse).transpose()?;
        Ok(self.assemble(proxy))
    }

    /// Converts the builder to options without re-checking the combination.
    fn assemble(self, proxy: Option<Proxy>) -> LaunchOptions {
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
            headless: self.headless,
            source,
            mode,
            ad_block: self.ad_block,
            proxy,
            user_data_dir: self.user_data_dir,
            user_agent: self.user_agent,
            lang: self.lang,
            window_size: self.window_size,
            window_position: self.window_position,
            no_sandbox: self.no_sandbox,
            webrtc: self.webrtc,
            shield_webrtc: self.shield_webrtc,
            extra_args: self.extra_args,
            startup_timeout: self.startup_timeout,
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
    let endpoint_file = profile_dir.join("DevToolsActivePort");
    // A stale file from an earlier run would be read as the new endpoint.
    let _ = std::fs::remove_file(&endpoint_file);

    let mut child = Command::new(&binary)
        .args(options.browser_args(&profile_dir))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| SeleniumBaseError::browser_launch(binary_name.clone(), e.to_string()))?;

    let deadline = Instant::now() + options.startup_timeout;
    loop {
        if let Some((port, path)) = std::fs::read_to_string(&endpoint_file)
            .ok()
            .and_then(|content| parse_endpoint(&content))
        {
            return Ok(LaunchedBrowser {
                child,
                port,
                ws_url: format!("ws://127.0.0.1:{port}{path}"),
                profile,
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

    fn args_for(builder: LaunchOptionsBuilder) -> Vec<String> {
        builder
            .build()
            .unwrap()
            .browser_args(Path::new("/tmp/profile"))
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
