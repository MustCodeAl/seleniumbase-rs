//! The virtual screen a headed browser draws on when the machine has none.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::SeleniumBaseError;

/// A virtual X display (Xvfb) for a browser to open its window on.
///
/// Python SeleniumBase starts one on Linux with `--xvfb`, sized by
/// `--xvfb-metrics=WIDTH,HEIGHT` (1366 by 768 when not given). It lets a test
/// run headed, with a real window, on a server with no screen. Only Linux has
/// Xvfb: on any other system a session ignores this setting, as Python does.
/// A headless session needs no screen and ignores it too.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::browser::options::VirtualDisplay;
///
/// assert_eq!(VirtualDisplay::default().to_string(), "1366,768");
/// let big: VirtualDisplay = "1920,1080".parse()?;
/// assert_eq!(big.xvfb_args(99), [":99", "-screen", "0", "1920x1080x24", "-nolisten", "tcp"]);
/// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
/// ```
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct VirtualDisplay {
    /// Width of the screen in pixels.
    pub width: u32,
    /// Height of the screen in pixels.
    pub height: u32,
}

impl VirtualDisplay {
    /// A screen `width` by `height` pixels.
    #[must_use]
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    /// The arguments to start `Xvfb` with for display number `number`: a
    /// 24-bit screen of this size that listens on its local socket only.
    #[must_use]
    pub fn xvfb_args(&self, number: u32) -> Vec<String> {
        vec![
            format!(":{number}"),
            "-screen".to_owned(),
            "0".to_owned(),
            format!("{}x{}x24", self.width, self.height),
            "-nolisten".to_owned(),
            "tcp".to_owned(),
        ]
    }

    /// The smallest screen a virtual display is given: 1024 by 768, the
    /// minimum Python SeleniumBase raises `--xvfb-metrics` to, because a
    /// browser window does not fit a smaller one.
    pub const MINIMUM: Self = Self::new(1024, 768);

    /// This screen with each side raised to [`MINIMUM`](Self::MINIMUM) where it
    /// is smaller.
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::browser::options::VirtualDisplay;
    ///
    /// assert_eq!(VirtualDisplay::new(800, 600).clamped(), VirtualDisplay::new(1024, 768));
    /// assert_eq!(VirtualDisplay::new(1920, 500).clamped(), VirtualDisplay::new(1920, 768));
    /// assert_eq!(VirtualDisplay::default().clamped(), VirtualDisplay::default());
    /// ```
    #[must_use]
    pub const fn clamped(self) -> Self {
        let width = if self.width < Self::MINIMUM.width {
            Self::MINIMUM.width
        } else {
            self.width
        };
        let height = if self.height < Self::MINIMUM.height {
            Self::MINIMUM.height
        } else {
            self.height
        };
        Self::new(width, height)
    }
}

impl Default for VirtualDisplay {
    /// 1366 by 768, the size Python SeleniumBase uses.
    fn default() -> Self {
        Self::new(1366, 768)
    }
}

impl fmt::Display for VirtualDisplay {
    /// Formats as `WIDTH,HEIGHT`, the form `--xvfb-metrics` takes.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{},{}", self.width, self.height)
    }
}

impl FromStr for VirtualDisplay {
    type Err = SeleniumBaseError;

    /// Reads `WIDTH,HEIGHT`; both must be above zero.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let invalid = || SeleniumBaseError::parse(text, "WIDTH,HEIGHT in pixels, above zero");
        let (width, height) = text.split_once(',').ok_or_else(invalid)?;
        let width: u32 = width.trim().parse().map_err(|_| invalid())?;
        let height: u32 = height.trim().parse().map_err(|_| invalid())?;
        if width == 0 || height == 0 {
            return Err(invalid());
        }
        Ok(Self { width, height })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_is_the_size_python_uses() {
        assert_eq!(VirtualDisplay::default(), VirtualDisplay::new(1366, 768));
    }

    #[test]
    fn metrics_are_read_like_the_python_option() {
        assert_eq!(
            "1920,1080".parse::<VirtualDisplay>().unwrap(),
            VirtualDisplay::new(1920, 1080)
        );
        assert_eq!(
            " 800 , 600 ".parse::<VirtualDisplay>().unwrap(),
            VirtualDisplay::new(800, 600)
        );
        for bad in [
            "", "1920", "1920,", "0,600", "800,0", "a,b", "1,2,3", "-1,2",
        ] {
            assert!(bad.parse::<VirtualDisplay>().is_err(), "{bad:?}");
        }
    }

    #[test]
    fn xvfb_is_started_on_the_numbered_display_with_a_24_bit_screen() {
        assert_eq!(
            VirtualDisplay::new(1366, 768).xvfb_args(7),
            [":7", "-screen", "0", "1366x768x24", "-nolisten", "tcp"]
        );
    }
}
