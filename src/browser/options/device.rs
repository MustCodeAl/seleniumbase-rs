//! Chrome's mobile device emulation.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::error::SeleniumBaseError;

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
}

impl Default for DeviceMetrics {
    /// 411 by 731 at a ratio of 3: the screen Python SeleniumBase emulates when
    /// it is asked for a mobile device and given no metrics.
    fn default() -> Self {
        Self::new(411, 731, 3.0)
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
        assert_eq!(DeviceMetrics::default(), DeviceMetrics::new(411, 731, 3.0));
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
}
