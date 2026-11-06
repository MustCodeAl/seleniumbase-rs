//! The Chrome arguments the on/off switches of a
//! [`LaunchOptionsBuilder`](super::LaunchOptionsBuilder) ask for.
//! They become launch arguments when the options are built.
//!
//! The arguments come from the same function the WebDriver engine uses
//! (`browser::options::switch_args`), so a switch means the same on both
//! engines. Two differences are on purpose: this engine never switches the GPU
//! off by itself in a headless browser (so `enable_3d_apis` needs nothing), and
//! an extension is loaded over the protocol, which Chrome allows in an
//! incognito window.

use crate::browser::options::{switch_args, LaunchSettings, LaunchSwitch};

use super::LaunchOptions;

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

#[cfg(test)]
mod tests {
    use std::path::Path;

    use serde_json::json;

    use super::*;
    use crate::browser::options::{DeviceMetrics, VirtualDisplay};
    use crate::sb_cdp::LaunchOptionsBuilder;

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
