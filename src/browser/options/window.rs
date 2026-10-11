//! The size and position of the browser window.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::SeleniumBaseError;

/// Splits `text` at a comma (or an `x`, when `allow_x` is set) into exactly two
/// numbers.
fn two_numbers<T: FromStr>(
    text: &str,
    expected: &str,
    allow_x: bool,
) -> Result<(T, T), SeleniumBaseError> {
    let invalid = || SeleniumBaseError::parse(text, expected);
    let mut parts = text.split(|c: char| c == ',' || (allow_x && c.eq_ignore_ascii_case(&'x')));
    let (Some(first), Some(second), None) = (parts.next(), parts.next(), parts.next()) else {
        return Err(invalid());
    };
    let first = first.trim().parse().map_err(|_| invalid())?;
    let second = second.trim().parse().map_err(|_| invalid())?;
    Ok((first, second))
}

/// The size of the browser window in pixels.
///
/// Python SeleniumBase takes it as `--window-size=WIDTH,HEIGHT` (and `size=` in
/// its Python API); [`FromStr`] reads the same text, and also `WIDTHxHEIGHT`.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::browser::options::WindowSize;
///
/// let size: WindowSize = "1920,1080".parse()?;
/// assert_eq!(size, WindowSize::new(1920, 1080));
/// assert_eq!(size.to_string(), "1920,1080");
/// assert!("0,600".parse::<WindowSize>().is_err());
/// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
/// ```
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct WindowSize {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

impl WindowSize {
    /// A window `width` pixels wide and `height` pixels high.
    #[must_use]
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

impl fmt::Display for WindowSize {
    /// Formats as `WIDTH,HEIGHT`, the value of Chrome's `--window-size`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{},{}", self.width, self.height)
    }
}

impl FromStr for WindowSize {
    type Err = SeleniumBaseError;

    /// Reads `WIDTH,HEIGHT` or `WIDTHxHEIGHT`. A side of zero is refused.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (width, height): (u32, u32) = two_numbers(text, "WIDTH,HEIGHT in pixels", true)?;
        if width == 0 || height == 0 {
            return Err(SeleniumBaseError::parse(
                text,
                "a window size whose sides are at least one pixel",
            ));
        }
        Ok(Self { width, height })
    }
}

/// The position of the browser window's top-left corner on the screen.
///
/// Python SeleniumBase takes it as `--window-position=X,Y` (and `position=`).
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::browser::options::WindowPosition;
///
/// let position: WindowPosition = "10,-20".parse()?;
/// assert_eq!(position, WindowPosition::new(10, -20));
/// assert_eq!(position.to_string(), "10,-20");
/// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
/// ```
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct WindowPosition {
    /// Distance from the left edge of the screen; negative on a second screen
    /// placed to the left.
    pub x: i32,
    /// Distance from the top edge of the screen.
    pub y: i32,
}

impl WindowPosition {
    /// A window whose top-left corner is at `(x, y)`.
    #[must_use]
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

impl fmt::Display for WindowPosition {
    /// Formats as `X,Y`, the value of Chrome's `--window-position`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{},{}", self.x, self.y)
    }
}

impl FromStr for WindowPosition {
    type Err = SeleniumBaseError;

    /// Reads `X,Y`.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (x, y) = two_numbers(text, "X,Y in pixels", false)?;
        Ok(Self { x, y })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_size_is_read_from_the_text_python_takes() {
        assert_eq!(
            "1280,720".parse::<WindowSize>().unwrap(),
            WindowSize::new(1280, 720)
        );
        assert_eq!(
            " 800 , 600 ".parse::<WindowSize>().unwrap(),
            WindowSize::new(800, 600)
        );
        assert_eq!(
            "1024x768".parse::<WindowSize>().unwrap(),
            WindowSize::new(1024, 768)
        );
        assert_eq!(
            "1024X768".parse::<WindowSize>().unwrap(),
            WindowSize::new(1024, 768)
        );
    }

    #[test]
    fn a_size_that_is_not_two_positive_numbers_is_refused() {
        for text in [
            "",
            "800",
            "800,",
            "800,600,400",
            "a,b",
            "-1,600",
            "0,600",
            "800,0",
        ] {
            assert!(text.parse::<WindowSize>().is_err(), "{text:?} was accepted");
        }
    }

    #[test]
    fn a_position_may_be_negative_and_is_not_split_on_x() {
        assert_eq!(
            "0,0".parse::<WindowPosition>().unwrap(),
            WindowPosition::new(0, 0)
        );
        assert_eq!(
            "-1920,40".parse::<WindowPosition>().unwrap(),
            WindowPosition::new(-1920, 40)
        );
        assert!("10x20".parse::<WindowPosition>().is_err());
        assert!("10".parse::<WindowPosition>().is_err());
    }

    #[test]
    fn display_gives_the_chrome_flag_value() {
        assert_eq!(WindowSize::new(1, 2).to_string(), "1,2");
        assert_eq!(WindowPosition::new(-3, 4).to_string(), "-3,4");
    }
}
