//! The phone, the on/off switches and the lists of values of a
//! [`LaunchOptionsBuilder`](super::LaunchOptionsBuilder), and the Chrome
//! arguments the switches become.
//!
//! The arguments come from the same function the WebDriver engine uses
//! (`browser::options::switch_args`), so a switch means the same on both
//! engines. Two differences are on purpose: this engine never switches the GPU
//! off by itself in a headless browser (so `enable_3d_apis` needs nothing), and
//! an extension is loaded over the protocol, which Chrome allows in an
//! incognito window.

use crate::browser::options::{
    switch_args, DeviceMetrics, LaunchSettings, LaunchSwitch, ProxyBypassList, VirtualDisplay,
};

use super::{LaunchOptions, LaunchOptionsBuilder, Redacted};

impl LaunchOptions {
    /// The Chrome arguments the on/off switches ask for, in a fixed order.
    ///
    /// Developer tools and a maximized window need a window, so a headless
    /// browser gets neither. Dark mode, incognito, guest and the sandbox have
    /// arguments of their own elsewhere in [`browser_args`](Self::browser_args).
    pub(super) fn switch_arguments(&self, headless: bool) -> Vec<String> {
        let mut launch = LaunchSettings::default();
        for switch in &self.switches {
            let used = match switch {
                LaunchSwitch::DisableWebSecurity
                | LaunchSwitch::Swiftshader
                | LaunchSwitch::DisableGpu => true,
                LaunchSwitch::Devtools | LaunchSwitch::Maximize => !headless,
                _ => false,
            };
            if used {
                launch.set(*switch, true);
            }
        }
        // `headless` is `false` on purpose: it would switch the GPU off, which
        // this engine does not do by itself.
        switch_args(&launch, false, false)
    }
}

impl LaunchOptionsBuilder {
    /// Makes the browser a phone: every tab gets Chrome's mobile emulation
    /// (a 390 by 715 screen at a pixel ratio of 3, touch input, and the
    /// `Android` user agent with matching client hints), as the WebDriver
    /// engine does with `mobile`.
    ///
    /// A [`user_agent`](Self::user_agent) set by the caller is reported
    /// instead of the phone's. Turning it off also forgets
    /// [`device_metrics`](Self::device_metrics). A [`fingerprint`](Self::fingerprint)
    /// already sets the screen and the user agent, so [`build`](Self::build)
    /// refuses the two together.
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
        self.mobile = on;
        if !on {
            self.device_metrics = None;
        }
        self
    }

    /// Sets the phone screen that [`mobile`](Self::mobile) emulates, and turns
    /// mobile emulation on (Python's `--metrics`, `d_width`, `d_height` and
    /// `d_p_r`).
    #[must_use]
    pub fn device_metrics(mut self, metrics: DeviceMetrics) -> Self {
        self.device_metrics = Some(metrics);
        self.mobile = true;
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

    /// Renders with SwiftShader, a software WebGL renderer, for a machine with
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
        self.disable_features
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
        self.enable_features
            .extend(features.into_iter().map(Into::into));
        self
    }

    /// Adds a rule that maps host names to addresses, such as
    /// `MAP example.com 127.0.0.1`. The rules of every call are joined into
    /// the one `--host-resolver-rules` switch Chrome reads.
    #[must_use]
    pub fn host_resolver_rules(mut self, rules: impl Into<String>) -> Self {
        self.host_resolver_rules.push(rules.into());
        self
    }

    /// The hosts that skip the [`proxy`](Self::proxy) (`--proxy-bypass-list`).
    #[must_use]
    pub fn proxy_bypass_list(mut self, list: ProxyBypassList) -> Self {
        self.proxy_bypass_list = list;
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
        self.proxy_pac_url = Some(Redacted(url.into()));
        self
    }

    /// Runs a headed browser on a virtual screen (`Xvfb`), for a Linux
    /// machine with no display. Other systems, and a headless browser, start
    /// none.
    ///
    /// Unless [`headless`](Self::headless) is set, the browser is headed.
    /// Screens smaller than 1024 by 768 are raised to that.
    #[must_use]
    pub fn virtual_display(mut self, display: VirtualDisplay) -> Self {
        self.virtual_display = Some(display);
        if self.headless.is_none() {
            self.headless = Some(false);
        }
        self
    }

    /// Turns one of the library's on/off launch settings on or off, as the
    /// WebDriver engine reads it from a `BrowserConfig`.
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
        match switch {
            LaunchSwitch::Incognito => self.incognito = on,
            LaunchSwitch::Guest => self.guest = on,
            LaunchSwitch::NoSandbox => self.no_sandbox = on,
            LaunchSwitch::BlockImages
            | LaunchSwitch::DisableJavaScript
            | LaunchSwitch::DisableCookies
            | LaunchSwitch::DoNotTrack
            | LaunchSwitch::ExternalPdf
            | LaunchSwitch::DisableCsp
            | LaunchSwitch::DarkMode
            | LaunchSwitch::Devtools
            | LaunchSwitch::DisableWebSecurity
            | LaunchSwitch::Swiftshader
            | LaunchSwitch::Maximize
            | LaunchSwitch::DisableGpu => {
                if on {
                    self.switches.insert(switch);
                } else {
                    self.switches.remove(&switch);
                }
            }
            LaunchSwitch::Enable3dApis => {}
            other => {
                tracing::debug!("{other:?} is not used by the pure CDP engine");
            }
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use serde_json::json;

    use super::*;

    fn args(builder: LaunchOptionsBuilder) -> Vec<String> {
        builder
            .no_sandbox(false)
            .build()
            .unwrap()
            .browser_args(Path::new("/tmp/profile"))
    }

    #[test]
    fn the_switch_preferences_are_the_ones_the_webdriver_engine_writes() {
        let options = LaunchOptions::builder()
            .switch(LaunchSwitch::BlockImages, true)
            .switch(LaunchSwitch::DoNotTrack, true)
            .preference("enable_do_not_track", false)
            .build()
            .unwrap();
        assert_eq!(
            options.prefs["profile.managed_default_content_settings.images"],
            json!(2)
        );
        // A preference the caller sets wins over the one a switch implies.
        assert_eq!(options.prefs["enable_do_not_track"], json!(false));
    }

    #[test]
    fn a_switch_turned_off_leaves_no_trace_and_foreign_ones_are_ignored() {
        let options = LaunchOptions::builder()
            .switch(LaunchSwitch::BlockImages, true)
            .switch(LaunchSwitch::BlockImages, false)
            .switch(LaunchSwitch::EscEnd, true)
            .switch(LaunchSwitch::Enable3dApis, true)
            .build()
            .unwrap();
        assert!(options.prefs.is_empty());
        assert!(options.switches.is_empty());
    }

    #[test]
    fn dark_mode_is_one_enabled_feature_that_is_not_repeated() {
        let options = LaunchOptions::builder()
            .enable_features(["WebContentsForceDark", "Other"])
            .dark_mode(true)
            .build()
            .unwrap();
        assert_eq!(
            options.enable_features,
            ["WebContentsForceDark", "Other"].map(String::from)
        );
        assert!(args(LaunchOptions::builder().dark_mode(true))
            .contains(&"--enable-features=WebContentsForceDark".to_owned()));
    }

    #[test]
    fn developer_tools_and_a_maximized_window_need_a_window() {
        let builder = || LaunchOptions::builder().devtools(true).maximize(true);
        let headed = args(builder().headless(false).window_size(800, 600));
        assert!(headed.contains(&"--auto-open-devtools-for-tabs".to_owned()));
        assert!(headed.contains(&"--start-maximized".to_owned()));
        assert!(
            !headed.iter().any(|arg| arg.starts_with("--window-size")),
            "a maximized window is sent no size: {headed:?}"
        );
        let headless = args(builder().headless(true).window_size(800, 600));
        assert!(!headless.contains(&"--auto-open-devtools-for-tabs".to_owned()));
        assert!(!headless.contains(&"--start-maximized".to_owned()));
        assert!(headless.contains(&"--window-size=800,600".to_owned()));
    }

    #[test]
    fn the_gpu_is_kept_unless_asked_off_even_in_a_headless_browser() {
        let plain = args(LaunchOptions::builder().headless(true));
        assert!(!plain.contains(&"--disable-gpu".to_owned()));
        let off = args(
            LaunchOptions::builder()
                .headless(true)
                .switch(LaunchSwitch::DisableGpu, true),
        );
        assert!(off.contains(&"--disable-gpu".to_owned()));
    }

    #[test]
    fn turning_mobile_off_forgets_the_screen() {
        let options = LaunchOptions::builder()
            .device_metrics(DeviceMetrics::new(360, 640, 2.0))
            .mobile(false)
            .build()
            .unwrap();
        assert!(options.device.is_none());
        assert_eq!(options.user_agent(), None);
    }

    #[test]
    fn metrics_turn_mobile_on_and_a_caller_agent_replaces_the_phone_agent() {
        let metrics = DeviceMetrics::new(360, 640, 2.0);
        let options = LaunchOptions::builder()
            .device_metrics(metrics)
            .user_agent("Agent/1")
            .build()
            .unwrap();
        assert_eq!(options.device, Some((metrics, Some("Agent/1".to_owned()))));
        assert_eq!(options.user_agent(), Some("Agent/1"));
    }

    #[test]
    fn a_virtual_display_makes_the_browser_headed_unless_told_otherwise() {
        let display = VirtualDisplay::default();
        assert!(!LaunchOptions::builder()
            .virtual_display(display)
            .build()
            .unwrap()
            .is_headless());
        assert!(LaunchOptions::builder()
            .headless(true)
            .virtual_display(display)
            .build()
            .unwrap()
            .is_headless());
    }

    #[test]
    fn a_pac_url_that_is_not_a_pac_file_is_refused_at_build() {
        let error = LaunchOptions::builder()
            .proxy_pac_url("http://pac.example/proxy.js")
            .build()
            .unwrap_err();
        assert!(error.to_string().contains(".pac"), "{error}");
    }
}
