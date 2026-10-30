//! Chrome's mobile device emulation.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::error::SeleniumBaseError;

/// The user agent a phone-emulating session reports when the caller names none:
/// Chrome on Android in its frozen format (release 10, model `K`), claiming
/// [`PRESET_CHROMIUM_MAJOR`](crate::evasion::randomize::PRESET_CHROMIUM_MAJOR),
/// which a test holds it to.
///
/// Python SeleniumBase names a Pixel 7 XL here; this crate keeps its own
/// Android agent on both engines, so a mobile session looks the same whichever
/// engine runs it.
pub const MOBILE_USER_AGENT: &str = "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/156.0.0.0 Mobile Safari/537.36";

/// The screen of the phone that Chrome emulates.
///
/// Python SeleniumBase takes it as `--metrics="CSS_Width,CSS_Height,Pixel_Ratio"`
/// (also `--device-metrics`), and as `d_width`, `d_height` and `d_p_r` in its
/// Python API. Giving it to a [`BrowserConfig`](crate::BrowserConfig) turns on
/// Chrome's mobile emulation: the page sees this viewport, a touch screen, and
/// this device pixel ratio.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::browser::options::DeviceMetrics;
///
/// let metrics: DeviceMetrics = "412,732,3".parse()?;
/// assert_eq!((metrics.width, metrics.height), (412, 732));
/// assert_eq!(metrics.pixel_ratio, 3.0);
/// assert_eq!(metrics.to_string(), "412,732,3");
/// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
/// ```
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct DeviceMetrics {
    /// Viewport width in CSS pixels (`d_width`).
    pub width: u32,
    /// Viewport height in CSS pixels (`d_height`).
    pub height: u32,
    /// Device pixels per CSS pixel (`d_p_r`).
    pub pixel_ratio: f64,
}

impl DeviceMetrics {
    /// A screen `width` by `height` CSS pixels at `pixel_ratio` device pixels
    /// per CSS pixel.
    #[must_use]
    pub const fn new(width: u32, height: u32, pixel_ratio: f64) -> Self {
        Self {
            width,
            height,
            pixel_ratio,
        }
    }

    /// The `mobileEmulation` experimental option of `goog:chromeOptions` for
    /// this screen, with `user_agent` as the browser's user agent when one is
    /// given.
    #[must_use]
    pub fn mobile_emulation(&self, user_agent: Option<&str>) -> Value {
        let mut emulation = json!({
            "deviceMetrics": {
                "width": self.width,
                "height": self.height,
                "pixelRatio": self.pixel_ratio,
            }
        });
        if let Some(user_agent) = user_agent {
            emulation["userAgent"] = json!(user_agent);
        }
        emulation
    }

    /// The DevTools commands that emulate this phone in one tab, for a session
    /// that cannot be given the `mobileEmulation` capability (the undetected
    /// mode, where the capability would show).
    ///
    /// They are `Emulation.setDeviceMetricsOverride` with `mobile: true`,
    /// `Emulation.setTouchEmulationEnabled` with five touch points and, when a
    /// `user_agent` is given, `Emulation.setUserAgentOverride` with client-hint
    /// metadata that says the same: an Android phone. The overrides belong to
    /// a tab, so they are sent again for each window a session switches to.
    #[must_use]
    pub fn emulation_commands(&self, user_agent: Option<&str>) -> Vec<(&'static str, Value)> {
        let mut commands = vec![
            (
                "Emulation.setDeviceMetricsOverride",
                json!({
                    "width": self.width,
                    "height": self.height,
                    "deviceScaleFactor": self.pixel_ratio,
                    "mobile": true,
                }),
            ),
            (
                "Emulation.setTouchEmulationEnabled",
                json!({ "enabled": true, "maxTouchPoints": 5 }),
            ),
        ];
        if let Some(user_agent) = user_agent {
            commands.push((
                "Emulation.setUserAgentOverride",
                json!({
                    "userAgent": user_agent,
                    "userAgentMetadata": {
                        "platform": "Android",
                        "platformVersion": "10",
                        "architecture": "",
                        "model": "K",
                        "mobile": true,
                    },
                }),
            ));
        }
        commands
    }
}

/// The phone a session emulates, and the user agent it reports, from the
/// settings that ask for one.
///
/// `metrics` name the screen; `mobile` alone gets [`DeviceMetrics::default`].
/// The agent is `user_agent` when the caller chose one, else
/// [`MOBILE_USER_AGENT`] when `mobile` is on, else none (the metrics were
/// given alone, and the browser keeps its own agent). `None` when neither
/// asks for a phone.
pub(crate) fn emulated_phone(
    mobile: bool,
    metrics: Option<DeviceMetrics>,
    user_agent: Option<&str>,
) -> Option<(DeviceMetrics, Option<&str>)> {
    let screen = metrics.or_else(|| mobile.then(DeviceMetrics::default))?;
    Some((screen, user_agent.or(mobile.then_some(MOBILE_USER_AGENT))))
}

impl Default for DeviceMetrics {
    /// 390 by 715 at a ratio of 3 (Python's `constants.Mobile`), on both engines, used when
    /// it is asked for a mobile device and given no metrics.
    fn default() -> Self {
        Self::new(390, 715, 3.0)
    }
}

impl fmt::Display for DeviceMetrics {
    /// Formats as `WIDTH,HEIGHT,RATIO`, the form `--metrics` takes.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{},{},{}", self.width, self.height, self.pixel_ratio)
    }
}

impl FromStr for DeviceMetrics {
    type Err = SeleniumBaseError;

    /// Reads `WIDTH,HEIGHT,RATIO`; the ratio may be fractional (`2.625`).
    /// Every part must be above zero.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let invalid = || SeleniumBaseError::parse(text, "WIDTH,HEIGHT,PIXEL_RATIO, all above zero");
        let parts: Vec<&str> = text.split(',').map(str::trim).collect();
        let [width, height, ratio] = parts.as_slice() else {
            return Err(invalid());
        };
        let width: u32 = width.parse().map_err(|_| invalid())?;
        let height: u32 = height.parse().map_err(|_| invalid())?;
        let pixel_ratio: f64 = ratio.parse().map_err(|_| invalid())?;
        if width == 0 || height == 0 || !pixel_ratio.is_finite() || pixel_ratio <= 0.0 {
            return Err(invalid());
        }
        Ok(Self::new(width, height, pixel_ratio))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_metrics_python_documents_are_read() {
        assert_eq!(
            "412,732,3".parse::<DeviceMetrics>().unwrap(),
            DeviceMetrics::new(412, 732, 3.0)
        );
        assert_eq!(
            " 360 , 640 , 2.625 ".parse::<DeviceMetrics>().unwrap(),
            DeviceMetrics::new(360, 640, 2.625)
        );
    }

    #[test]
    fn metrics_that_are_not_three_positive_numbers_are_refused() {
        for text in [
            "",
            "412",
            "412,732",
            "412,732,3,1",
            "a,b,c",
            "0,732,3",
            "412,0,3",
            "412,732,0",
            "412,732,-1",
            "412,732,nan",
            "412,732,inf",
            "-5,732,3",
        ] {
            assert!(
                text.parse::<DeviceMetrics>().is_err(),
                "{text:?} was accepted"
            );
        }
    }

    #[test]
    fn the_default_is_the_screen_python_emulates() {
        assert_eq!(DeviceMetrics::default(), DeviceMetrics::new(390, 715, 3.0));
    }

    #[test]
    fn display_round_trips() {
        for metrics in [DeviceMetrics::default(), DeviceMetrics::new(360, 640, 2.5)] {
            assert_eq!(
                metrics.to_string().parse::<DeviceMetrics>().unwrap(),
                metrics
            );
        }
    }

    #[test]
    fn the_emulation_option_has_the_shape_chromedriver_expects() {
        let metrics = DeviceMetrics::new(412, 732, 3.0);
        assert_eq!(
            metrics.mobile_emulation(None),
            json!({ "deviceMetrics": { "width": 412, "height": 732, "pixelRatio": 3.0 } })
        );
        assert_eq!(
            metrics.mobile_emulation(Some("Agent/1")),
            json!({
                "deviceMetrics": { "width": 412, "height": 732, "pixelRatio": 3.0 },
                "userAgent": "Agent/1"
            })
        );
    }

    #[test]
    fn the_emulation_commands_make_a_touch_phone_with_the_given_agent() {
        let metrics = DeviceMetrics::new(360, 640, 2.0);
        let commands = metrics.emulation_commands(None);
        assert_eq!(
            commands,
            vec![
                (
                    "Emulation.setDeviceMetricsOverride",
                    json!({
                        "width": 360, "height": 640, "deviceScaleFactor": 2.0, "mobile": true
                    })
                ),
                (
                    "Emulation.setTouchEmulationEnabled",
                    json!({ "enabled": true, "maxTouchPoints": 5 })
                ),
            ]
        );

        let with_agent = metrics.emulation_commands(Some("Agent/1"));
        assert_eq!(with_agent.len(), 3);
        assert_eq!(with_agent[2].0, "Emulation.setUserAgentOverride");
        assert_eq!(with_agent[2].1["userAgent"], "Agent/1");
        assert_eq!(with_agent[2].1["userAgentMetadata"]["mobile"], true);
        assert_eq!(with_agent[2].1["userAgentMetadata"]["platform"], "Android");
    }

    #[test]
    fn the_mobile_user_agent_is_an_android_phone() {
        assert!(MOBILE_USER_AGENT.contains("Android"));
        assert!(MOBILE_USER_AGENT.contains("Mobile Safari"));
    }
}
