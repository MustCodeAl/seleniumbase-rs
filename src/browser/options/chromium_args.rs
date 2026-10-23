//! The Chromium command-line arguments that the launch settings ask for.
//!
//! [`parse_chromium_args`] reads Python SeleniumBase's `--chromium-arg` text;
//! [`switch_args`] turns the on/off switches and the argument-valued settings
//! of a [`LaunchSettings`] into the arguments Chromium takes, in one place and
//! a fixed order.

use super::{LaunchSettings, LaunchSwitch};

/// Reads the text of `--chromium-arg` (Python's `chromium_arg`): items
/// separated by commas, each written as a command-line switch.
///
/// An item is trimmed and made a `--` switch: `--x` stays, `-x` becomes `--x`,
/// and `x` becomes `--x`. Items shorter than three characters are dropped, as
/// Python drops them. An item `disable-features=A` is not an argument of its
/// own: `A` goes to the second list, because Chrome reads only the last
/// `--disable-features` it is given, and the session joins every feature into
/// one switch.
///
/// Returns the arguments and the features to disable.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::browser::options::parse_chromium_args;
///
/// let (args, features) = parse_chromium_args("--mute-audio, -ignore-gpu-blocklist,lang=fr,disable-features=Translate,x");
/// assert_eq!(args, ["--mute-audio", "--ignore-gpu-blocklist", "--lang=fr"]);
/// assert_eq!(features, ["Translate"]);
/// ```
#[must_use]
pub fn parse_chromium_args(text: &str) -> (Vec<String>, Vec<String>) {
    let mut args = Vec::new();
    let mut features = Vec::new();
    for item in text.split(',').map(str::trim) {
        if item.len() < 3 {
            continue;
        }
        let switch = item.trim_start_matches('-');
        if let Some(feature) = switch.strip_prefix("disable-features=") {
            let feature = feature.trim();
            if !feature.is_empty() && !features.iter().any(|known| known == feature) {
                features.push(feature.to_owned());
            }
        } else if !switch.is_empty() {
            args.push(format!("--{switch}"));
        }
    }
    (args, features)
}

/// The Chromium arguments that `launch` asks for, in a fixed order.
///
/// `headless` says whether the window is hidden, which decides whether the
/// GPU is switched off, and whether developer tools and maximizing mean
/// anything. `loads_extensions` says whether the session loads an extension,
/// which Chrome does not allow together with `--incognito` or `--guest`: they
/// are left out, with a warning, as Python leaves them out.
///
/// The GPU is switched off (`--disable-gpu`) when asked for, and in a
/// headless session unless it is given a software renderer (`swiftshader`) or
/// told that the page needs WebGL (`enable_3d_apis`).
#[must_use]
pub(crate) fn switch_args(
    launch: &LaunchSettings,
    headless: bool,
    loads_extensions: bool,
) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();

    if launch.has(LaunchSwitch::DarkMode) {
        args.push("--enable-features=WebContentsForceDark".to_owned());
    }
    for (switch, argument) in [
        (LaunchSwitch::Incognito, "--incognito"),
        (LaunchSwitch::Guest, "--guest"),
    ] {
        if !launch.has(switch) {
            continue;
        }
        if loads_extensions {
            tracing::warn!(
                "{argument} is left out: Chrome does not load extensions in that mode, and this \
                 session loads one"
            );
        } else {
            args.push(argument.to_owned());
        }
    }
    if launch.has(LaunchSwitch::Devtools) && !headless {
        args.push("--auto-open-devtools-for-tabs".to_owned());
    }
    if launch.has(LaunchSwitch::DisableWebSecurity) {
        args.extend(
            [
                "--disable-web-security",
                "--ignore-certificate-errors",
                "--ignore-ssl-errors=yes",
            ]
            .map(str::to_owned),
        );
    }
    let swiftshader = launch.has(LaunchSwitch::Swiftshader);
    if swiftshader {
        args.extend(["--use-gl=angle", "--use-angle=swiftshader-webgl"].map(str::to_owned));
    }
    let gpu_off = launch.has(LaunchSwitch::DisableGpu)
        || (headless && !swiftshader && !launch.has(LaunchSwitch::Enable3dApis));
    if gpu_off {
        args.push("--disable-gpu".to_owned());
    }
    if launch.has(LaunchSwitch::Maximize) && !headless {
        args.push("--start-maximized".to_owned());
    }
    // On Linux the session turns the sandbox off itself, where it commonly
    // cannot start; asking again would send the switch twice.
    if launch.has(LaunchSwitch::NoSandbox) && !cfg!(target_os = "linux") {
        args.push("--no-sandbox".to_owned());
    }
    if let Some(rules) = launch.host_resolver_rules.as_deref() {
        args.push(format!("--host-resolver-rules={rules}"));
    }
    if !launch.disable_features.is_empty() {
        args.push(format!(
            "--disable-features={}",
            launch.disable_features.join(",")
        ));
    }
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(switches: &[LaunchSwitch]) -> LaunchSettings {
        let mut launch = LaunchSettings::default();
        for switch in switches {
            launch.set(*switch, true);
        }
        launch
    }

    #[test]
    fn chromium_arg_text_becomes_double_dash_switches() {
        let (args, features) = parse_chromium_args(
            " --mute-audio , -ignore-gpu-blocklist,lang=fr, ab ,--disable-features=Translate,\
             disable-features=Foo,disable-features=Foo",
        );
        assert_eq!(
            args,
            ["--mute-audio", "--ignore-gpu-blocklist", "--lang=fr"]
        );
        assert_eq!(features, ["Translate", "Foo"]);
    }

    #[test]
    fn an_empty_or_short_chromium_arg_text_gives_nothing() {
        for text in ["", ",", "x", "-x", " , ab ,"] {
            assert_eq!(
                parse_chromium_args(text),
                (Vec::new(), Vec::new()),
                "{text:?}"
            );
        }
    }

    #[test]
    fn no_switch_asks_for_no_argument_and_a_headless_session_loses_its_gpu() {
        let none = LaunchSettings::default();
        assert!(switch_args(&none, false, false).is_empty());
        assert_eq!(switch_args(&none, true, false), ["--disable-gpu"]);
    }

    #[test]
    fn each_switch_gives_the_arguments_python_gives() {
        let cases: [(LaunchSwitch, &[&str]); 5] = [
            (
                LaunchSwitch::DarkMode,
                &["--enable-features=WebContentsForceDark"],
            ),
            (LaunchSwitch::Incognito, &["--incognito"]),
            (LaunchSwitch::Guest, &["--guest"]),
            (LaunchSwitch::Devtools, &["--auto-open-devtools-for-tabs"]),
            (
                LaunchSwitch::DisableWebSecurity,
                &[
                    "--disable-web-security",
                    "--ignore-certificate-errors",
                    "--ignore-ssl-errors=yes",
                ],
            ),
        ];
        for (switch, expected) in cases {
            assert_eq!(
                switch_args(&with(&[switch]), false, false),
                expected,
                "{switch:?}"
            );
        }
        assert_eq!(
            switch_args(&with(&[LaunchSwitch::DisableGpu]), false, false),
            ["--disable-gpu"]
        );
        assert_eq!(
            switch_args(&with(&[LaunchSwitch::Maximize]), false, false),
            ["--start-maximized"]
        );
    }

    #[test]
    fn developer_tools_and_maximizing_need_a_window() {
        let launch = with(&[LaunchSwitch::Devtools, LaunchSwitch::Maximize]);
        assert_eq!(switch_args(&launch, true, false), ["--disable-gpu"]);
    }

    #[test]
    fn incognito_and_guest_are_dropped_when_an_extension_is_loaded() {
        let launch = with(&[LaunchSwitch::Incognito, LaunchSwitch::Guest]);
        assert_eq!(
            switch_args(&launch, false, false),
            ["--incognito", "--guest"]
        );
        assert!(switch_args(&launch, false, true).is_empty());
    }

    #[test]
    fn swiftshader_gives_a_software_renderer_and_keeps_the_gpu_switch_off_the_list() {
        let launch = with(&[LaunchSwitch::Swiftshader]);
        for headless in [false, true] {
            assert_eq!(
                switch_args(&launch, headless, false),
                ["--use-gl=angle", "--use-angle=swiftshader-webgl"],
                "headless: {headless}"
            );
        }
    }

    #[test]
    fn enable_3d_apis_keeps_the_gpu_in_a_headless_session() {
        let launch = with(&[LaunchSwitch::Enable3dApis]);
        assert!(switch_args(&launch, true, false).is_empty());
        // Asking for the GPU off wins, headed or not.
        let both = with(&[LaunchSwitch::Enable3dApis, LaunchSwitch::DisableGpu]);
        assert_eq!(switch_args(&both, true, false), ["--disable-gpu"]);
    }

    #[test]
    fn no_sandbox_is_sent_once() {
        let args = switch_args(&with(&[LaunchSwitch::NoSandbox]), false, false);
        // On Linux `StealthOptions` already adds it.
        assert_eq!(args.is_empty(), cfg!(target_os = "linux"));
    }

    #[test]
    fn the_resolver_rules_and_the_disabled_features_are_one_switch_each() {
        let launch = LaunchSettings {
            host_resolver_rules: Some("MAP example.com 127.0.0.1".to_owned()),
            disable_features: vec!["Translate".to_owned(), "Foo".to_owned()],
            ..LaunchSettings::default()
        };
        assert_eq!(
            switch_args(&launch, false, false),
            [
                "--host-resolver-rules=MAP example.com 127.0.0.1",
                "--disable-features=Translate,Foo"
            ]
        );
    }

    #[test]
    fn the_order_is_fixed() {
        let launch = with(&[
            LaunchSwitch::Maximize,
            LaunchSwitch::Devtools,
            LaunchSwitch::DarkMode,
            LaunchSwitch::DisableGpu,
        ]);
        assert_eq!(
            switch_args(&launch, false, false),
            [
                "--enable-features=WebContentsForceDark",
                "--auto-open-devtools-for-tabs",
                "--disable-gpu",
                "--start-maximized"
            ]
        );
    }
}
