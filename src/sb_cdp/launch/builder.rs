//! The setters of [`LaunchOptionsBuilder`], and its `build`.
//!
//! A setter writes one setting into the builder's own layer (see [`layers`](super::layers)); it
//! never reads another setting, so the order of the calls changes nothing. What the settings mean
//! together is worked out by [`build`](LaunchOptionsBuilder::build).

use std::path::PathBuf;

use std::time::Duration;

use serde_json::Value;

use super::layers::{Layer, Layers, ProxySpec, SharedFingerprint};
use super::{LaunchOptions, LaunchOptionsBuilder, Redacted};
use crate::browser::config::BrowserConfig;
use crate::browser::options::{DeviceMetrics, LaunchSwitch, ProxyBypassList, VirtualDisplay};
use crate::error::SeleniumBaseError;
use crate::evasion::fingerprint::{Fingerprint, WebRtcPolicy};

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
        self.given.set.cdp_url = Some(Redacted(url.into()));
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
        self.given.set.cdp_target = Some(target.into());
        self
    }

    /// Sets Chrome's WebRTC IP-handling policy, the same flag a `Fingerprint`
    /// applies. Chrome's own behaviour is kept unless this is called. A policy
    /// set here wins over the one a [`fingerprint`](Self::fingerprint) names.
    ///
    /// The flag limits which addresses are offered but does not stop a page
    /// seeing `.local` host candidates; for that, use
    /// [`shield_webrtc`](Self::shield_webrtc).
    #[must_use]
    pub fn webrtc_policy(mut self, policy: WebRtcPolicy) -> Self {
        self.given.set.webrtc = Some(policy);
        self
    }

    /// Makes every tab look like `fingerprint` describes.
    ///
    /// The user agent, language, window size, WebRTC policy and proxy it names
    /// become launch settings, and its script and protocol overrides are applied
    /// to every tab as it opens, before the first navigation. A setting made on
    /// the builder (a user agent, say), or read from a config with
    /// [`browser_config`](Self::browser_config), wins over the fingerprint's,
    /// whether it is made before this call or after it. A fingerprint set here
    /// wins over the one of a config.
    ///
    /// Chrome flags listed in the fingerprint's `cmd_params` are not passed on:
    /// a flag such as `--renderer-cmd-prefix` runs a command, and a fingerprint
    /// often comes from a profile file. Add flags you trust with
    /// [`arg`](Self::arg).
    ///
    /// See [`Page::apply_fingerprint`](crate::sb_cdp::Page::apply_fingerprint) for
    /// what is applied and what is deliberately left out.
    #[must_use]
    pub fn fingerprint(mut self, fingerprint: &Fingerprint) -> Self {
        self.given.set.fingerprint = Some(SharedFingerprint::new(fingerprint));
        self
    }

    /// Makes WebRTC relay-only in every tab, so none gathers a candidate or
    /// contacts a STUN server. Pages that need a WebRTC call to connect will
    /// not. See [`Page::shield_webrtc`](crate::sb_cdp::Page::shield_webrtc).
    #[must_use]
    pub fn shield_webrtc(mut self, shield: bool) -> Self {
        self.given.set.shield_webrtc = Some(shield);
        self
    }

    /// Navigates to `url` as soon as the browser is up.
    #[must_use]
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.given.set.url = Some(url.into());
        self
    }

    /// Forces headless (`true`) or headed (`false`) mode.
    ///
    /// Unset, the browser is headless on Linux, where there is usually no
    /// display, and headed elsewhere; a [`virtual_display`](Self::virtual_display)
    /// makes it headed.
    #[must_use]
    pub fn headless(mut self, headless: bool) -> Self {
        self.given.set.headless = Some(headless);
        self
    }

    /// Prefers Chromium over Google Chrome when locating the browser.
    /// `use_chromium(false)` keeps Chrome even when a
    /// [`browser_config`](Self::browser_config) names Chromium.
    #[must_use]
    pub fn use_chromium(mut self, use_chromium: bool) -> Self {
        self.given.set.use_chromium = Some(use_chromium);
        self
    }

    /// Uses the browser at `path` instead of searching for one.
    #[must_use]
    pub fn executable_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.given.set.executable_path = Some(path.into());
        self
    }

    /// Launches in incognito mode.
    #[must_use]
    pub fn incognito(self, incognito: bool) -> Self {
        self.switch(LaunchSwitch::Incognito, incognito)
    }

    /// Launches in guest mode.
    #[must_use]
    pub fn guest(self, guest: bool) -> Self {
        self.switch(LaunchSwitch::Guest, guest)
    }

    /// Blocks requests to common advertising and tracking hosts.
    ///
    /// A basic block list applied through the protocol, not a full content
    /// blocker.
    #[must_use]
    pub fn ad_block(mut self, ad_block: bool) -> Self {
        self.given.set.ad_block = Some(ad_block);
        self
    }

    /// Routes traffic through a proxy: `SERVER:PORT` or
    /// `USER:PASS@SERVER:PORT`.
    #[must_use]
    pub fn proxy(mut self, proxy: impl Into<String>) -> Self {
        self.given.set.proxy = Some(ProxySpec {
            text: Redacted(proxy.into()),
            checked: false,
        });
        self
    }

    /// Keeps the browser profile in `dir` instead of a throwaway directory.
    #[must_use]
    pub fn user_data_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.given.set.user_data_dir = Some(dir.into());
        self
    }

    /// Overrides the `User-Agent` header.
    #[must_use]
    pub fn user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.given.set.user_agent = Some(user_agent.into());
        self
    }

    /// Sets the browser UI and `Accept-Language` locale, such as `en-US`.
    #[must_use]
    pub fn lang(mut self, lang: impl Into<String>) -> Self {
        self.given.set.lang = Some(lang.into());
        self
    }

    /// Sets the initial window size in pixels.
    #[must_use]
    pub fn window_size(mut self, width: u32, height: u32) -> Self {
        self.given.set.window_size = Some((width, height));
        self
    }

    /// Sets the initial window position in pixels.
    #[must_use]
    pub fn window_position(mut self, x: i32, y: i32) -> Self {
        self.given.set.window_position = Some((x, y));
        self
    }

    /// Passes `--no-sandbox`, which Chrome needs when run as root in a
    /// container. Also enabled by setting the `SB_NO_SANDBOX` variable, which
    /// this setting overrides either way.
    #[must_use]
    pub fn no_sandbox(self, no_sandbox: bool) -> Self {
        self.switch(LaunchSwitch::NoSandbox, no_sandbox)
    }

    /// Appends a raw Chrome command-line argument. The arguments of a
    /// [`browser_config`](Self::browser_config) come first, so Chrome, which keeps
    /// the last of a switch, takes the ones given here.
    #[must_use]
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.given.set.extra_args.push(arg.into());
        self
    }

    /// Loads the unpacked extension in `dir` once the browser runs.
    ///
    /// Branded Chrome ignores `--load-extension` since version 137, so the
    /// extension is loaded over the protocol (`Extensions.loadUnpacked`), which
    /// needs `--enable-unsafe-extension-debugging`; the launch adds that switch.
    /// A browser attached with [`Browser::connect`](crate::sb_cdp::Browser::connect)
    /// loads none.
    #[must_use]
    pub fn extension(mut self, dir: impl Into<PathBuf>) -> Self {
        self.given.set.extensions.push(dir.into());
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
        self.given.set.prefs.insert(name.into(), value.into());
        self
    }

    /// The plugins whose hooks run when the browser starts, for every tab it
    /// opens and when it closes (feature `plugins`, ADR 004).
    ///
    /// [`Browser::launch`](crate::sb_cdp::Browser::launch) checks the set, lets each
    /// plugin change the start-up settings (arguments, preferences, extensions,
    /// proxy, headless), starts the browser and calls `on_browser`; every tab
    /// the library attaches to gets the plugins' packages and `on_page_created`;
    /// [`Browser::close`](crate::sb_cdp::Browser::close) calls `on_close`. An error from
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
        self.given.set.plugins = Some(plugins);
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
            self.given.set.selection = std::mem::take(&mut self.given.set.selection).enable(name);
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
        self.given.set.selection.extend(selection);
        self
    }

    /// Sets how long to wait for the browser to become reachable.
    #[must_use]
    pub fn startup_timeout(mut self, timeout: Duration) -> Self {
        self.given.set.startup_timeout = Some(timeout);
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
        self.given.set.debugging_port = Some(port);
        self
    }

    /// Makes the browser a phone: every tab gets Chrome's mobile emulation
    /// (a 390 by 715 screen at a pixel ratio of 3, touch input, and the
    /// `Android` user agent with matching client hints), as the WebDriver
    /// engine does with `mobile`.
    ///
    /// A [`user_agent`](Self::user_agent) set by the caller is reported
    /// instead of the phone's. `mobile(false)` turns the phone off whatever
    /// else asks for one, [`device_metrics`](Self::device_metrics) and a
    /// config's `mobile` included, in whatever order the calls are made. A
    /// [`fingerprint`](Self::fingerprint) already sets the screen and the user
    /// agent, so [`build`](Self::build) refuses the two together.
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::sb_cdp::LaunchOptions;
    ///
    /// let options = LaunchOptions::builder().mobile(true).build()?;
    /// assert!(options.user_agent().is_some_and(|agent| agent.contains("Android")));
    /// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
    /// ```
    #[must_use]
    pub fn mobile(mut self, on: bool) -> Self {
        self.given.set.mobile = Some(on);
        self
    }

    /// Sets the phone screen that [`mobile`](Self::mobile) emulates, and turns
    /// mobile emulation on (Python's `--metrics`, `d_width`, `d_height` and
    /// `d_p_r`) unless `mobile(false)` was given.
    #[must_use]
    pub fn device_metrics(mut self, metrics: DeviceMetrics) -> Self {
        self.given.set.device_metrics = Some(metrics);
        self
    }

    /// Forces Chrome's dark theme on web pages (`--enable-features=WebContentsForceDark`).
    ///
    /// To change only what `prefers-color-scheme` reports, use
    /// [`Emulation::color_scheme`](crate::sb_cdp::Emulation::color_scheme).
    #[must_use]
    pub fn dark_mode(self, on: bool) -> Self {
        self.switch(LaunchSwitch::DarkMode, on)
    }

    /// Opens the developer tools with each tab (`--auto-open-devtools-for-tabs`).
    /// A headless browser has no window to show them in, and gets nothing.
    #[must_use]
    pub fn devtools(self, on: bool) -> Self {
        self.switch(LaunchSwitch::Devtools, on)
    }

    /// Switches web security off (`--disable-web-security`, and certificate
    /// errors are ignored): cross-origin requests and bad certificates are
    /// allowed.
    #[must_use]
    pub fn disable_web_security(self, on: bool) -> Self {
        self.switch(LaunchSwitch::DisableWebSecurity, on)
    }

    /// Renders with `SwiftShader`, a software WebGL renderer, for a machine with
    /// no GPU.
    #[must_use]
    pub fn swiftshader(self, on: bool) -> Self {
        self.switch(LaunchSwitch::Swiftshader, on)
    }

    /// Opens the window maximized (`--start-maximized`) and sends no
    /// [`window_size`](Self::window_size). A headless browser has no window,
    /// and gets neither.
    #[must_use]
    pub fn maximize(self, on: bool) -> Self {
        self.switch(LaunchSwitch::Maximize, on)
    }

    /// Chromium features to switch off, joined with the library's own and the
    /// plugins' into the one `--disable-features` switch Chrome reads.
    #[must_use]
    pub fn disable_features<I, S>(mut self, features: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.given
            .set
            .disable_features
            .extend(features.into_iter().map(Into::into));
        self
    }

    /// Chromium features to switch on, joined into the one `--enable-features`
    /// switch Chrome reads.
    #[must_use]
    pub fn enable_features<I, S>(mut self, features: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.given
            .set
            .enable_features
            .extend(features.into_iter().map(Into::into));
        self
    }

    /// Adds a rule that maps host names to addresses, such as
    /// `MAP example.com 127.0.0.1`. The rules of every call are joined into
    /// the one `--host-resolver-rules` switch Chrome reads.
    #[must_use]
    pub fn host_resolver_rules(mut self, rules: impl Into<String>) -> Self {
        self.given.set.host_resolver_rules.push(rules.into());
        self
    }

    /// The hosts that skip the [`proxy`](Self::proxy) (`--proxy-bypass-list`).
    #[must_use]
    pub fn proxy_bypass_list(mut self, list: ProxyBypassList) -> Self {
        self.given.set.proxy_bypass_list = Some(list);
        self
    }

    /// The proxy auto-config (PAC) file to use (`--proxy-pac-url`), written
    /// `[user:password@]URL`; the URL must end in `.pac`.
    ///
    /// Chrome is never given the login. A PAC file that asks for one is
    /// answered over the protocol, as a proxy is. [`build`](Self::build)
    /// refuses a URL that is not a PAC file address.
    #[must_use]
    pub fn proxy_pac_url(mut self, url: impl Into<String>) -> Self {
        self.given.set.proxy_pac_url = Some(Redacted(url.into()));
        self
    }

    /// Runs a headed browser on a virtual screen (`Xvfb`), for a Linux
    /// machine with no display. Other systems, and a headless browser, start
    /// none.
    ///
    /// Unless [`headless`](Self::headless) is set, here or by a
    /// [`browser_config`](Self::browser_config), the browser is headed.
    /// Screens smaller than 1024 by 768 are raised to that.
    #[must_use]
    pub fn virtual_display(mut self, display: VirtualDisplay) -> Self {
        self.given.set.virtual_display = Some(display);
        self
    }

    /// Turns one of the library's on/off launch settings on or off, as the
    /// WebDriver engine reads it from a `BrowserConfig`. A switch turned on or
    /// off here is decided by this call, whatever a
    /// [`browser_config`](Self::browser_config) says of it.
    ///
    /// The engine uses `BlockImages`, `DisableJavaScript`, `DisableCookies`,
    /// `DoNotTrack` and `ExternalPdf` (profile preferences), `DisableCsp`
    /// (`Page.setBypassCSP` in every tab), `DarkMode`, `Incognito`, `Guest`,
    /// `Devtools`, `DisableWebSecurity`, `Swiftshader`, `Maximize`,
    /// `DisableGpu` and `NoSandbox`. `Enable3dApis` changes nothing, since this
    /// engine never switches the GPU off by itself. The others belong to the
    /// WebDriver engine and are logged at debug level and left out.
    #[must_use]
    pub fn switch(mut self, switch: LaunchSwitch, on: bool) -> Self {
        self.given.set.set_switch(switch, on);
        self
    }

    /// Everything of `config` that the pure CDP engine can use, as Python's
    /// `sb_cdp.Chrome(**kwargs)` reads the same options.
    ///
    /// Headless or headed, the browser (a binary path, or Chromium), the user
    /// agent, the locale (`--lang` and `intl.accept_languages`), the proxy and
    /// its bypass list, a PAC file, the profile folder, unpacked extension
    /// folders, the start page, extra arguments, mobile emulation and its
    /// screen, the window size and position, the virtual display, the
    /// remote debugging port, `ad_block`, a fingerprint, the on/off switches
    /// (see [`switch`](Self::switch)), the disabled features and the host
    /// resolver rules.
    ///
    /// What only the WebDriver engine can use (the page load strategy,
    /// capabilities, the driver version, Firefox options, the DevTools event
    /// log, extension archives, a custom ad block list, a WebDriver URL) is
    /// listed at debug level and left out. A proxy or PAC address that is not
    /// well formed makes [`build`](Self::build) fail, as it makes the WebDriver
    /// engine fail.
    ///
    /// The config is one layer under the settings of the builder: a setting
    /// made on the builder wins over the config's, then the config's wins
    /// over the fingerprint's, whatever the order of the calls. The lists
    /// (arguments, extensions, preferences, features) are added together, the
    /// config's first. A second call replaces the config of the first.
    ///
    /// With a `cdp_url` the browser is attached to, not launched, and only
    /// what can reach a running app is taken: a proxy, a PAC file, and a user
    /// agent or locale without a fingerprint make [`build`](Self::build) fail
    /// with [`InvalidConfig`](SeleniumBaseError::InvalidConfig), instead of
    /// being dropped without a word.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::Path;
    /// use seleniumbase_rs::sb_cdp::LaunchOptions;
    /// use seleniumbase_rs::BrowserConfig;
    ///
    /// let config = BrowserConfig::default().with_headless(true).with_mobile(true);
    /// let options = LaunchOptions::builder().browser_config(&config).build()?;
    /// assert!(options.is_headless());
    /// assert!(options
    ///     .browser_args(Path::new("/tmp/profile"))
    ///     .iter()
    ///     .any(|arg| arg.starts_with("--user-agent=") && arg.contains("Android")));
    ///
    /// // A setting of the builder wins over the config's, before or after it.
    /// let headed = LaunchOptions::builder()
    ///     .headless(false)
    ///     .browser_config(&config)
    ///     .build()?;
    /// assert!(!headed.is_headless());
    /// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
    /// ```
    #[must_use]
    pub fn browser_config(mut self, config: &BrowserConfig) -> Self {
        self.given.config = Layer::from_config(config);
        self
    }

    /// Checks the combination and produces the options.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] when `incognito` and
    /// `guest` are both set, when `use_chromium` is combined with an explicit
    /// executable path, when a fingerprint is combined with mobile emulation,
    /// when the proxy or the PAC address cannot be parsed, or when an
    /// attached browser ([`cdp_url`](Self::cdp_url)) is given what only a
    /// launch can use, and [`SeleniumBaseError::Plugin`] when a plugin named
    /// with `plugins_named` or `plugin_selection` (feature `plugins`) is
    /// unknown, refuses its options or is named twice.
    pub fn build(self) -> Result<LaunchOptions, SeleniumBaseError> {
        Layers::resolve(*self.given)
    }
}
