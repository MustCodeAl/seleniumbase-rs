//! When the browser says a page has finished loading.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::SeleniumBaseError;

/// The WebDriver `pageLoadStrategy`: how long a navigation waits before it
/// returns.
///
/// Python SeleniumBase calls this `--page-load-strategy` (`--pls`, and
/// `page_load_strategy=` or `pls=` in its Python API). Anything but
/// [`Normal`](Self::Normal) makes `open` return before the page is complete,
/// so the test has to wait for the elements it needs.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::browser::options::PageLoadStrategy;
///
/// let strategy: PageLoadStrategy = "eager".parse()?;
/// assert_eq!(strategy, PageLoadStrategy::Eager);
/// assert_eq!(strategy.as_str(), "eager");
/// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
/// ```
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PageLoadStrategy {
    /// Wait for the `load` event, after every sub-resource has arrived (the
    /// WebDriver default).
    #[default]
    Normal,
    /// Wait for `DOMContentLoaded`, once the HTML is parsed.
    Eager,
    /// Return as soon as the first response has been received.
    None,
}

impl PageLoadStrategy {
    /// The name WebDriver gives it in the `pageLoadStrategy` capability.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Eager => "eager",
            Self::None => "none",
        }
    }
}

impl fmt::Display for PageLoadStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for PageLoadStrategy {
    type Err = SeleniumBaseError;

    /// Reads `normal`, `eager` or `none`, in any letter case.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text.trim().to_ascii_lowercase().as_str() {
            "normal" => Ok(Self::Normal),
            "eager" => Ok(Self::Eager),
            "none" => Ok(Self::None),
            _ => Err(SeleniumBaseError::parse(
                text,
                "a page load strategy: normal, eager or none",
            )),
        }
    }
}

impl From<PageLoadStrategy>
    for thirtyfour::common::capabilities::desiredcapabilities::PageLoadStrategy
{
    fn from(strategy: PageLoadStrategy) -> Self {
        match strategy {
            PageLoadStrategy::Normal => Self::Normal,
            PageLoadStrategy::Eager => Self::Eager,
            PageLoadStrategy::None => Self::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_three_names_python_accepts_are_read_in_any_case() {
        assert_eq!(
            "normal".parse::<PageLoadStrategy>().unwrap(),
            PageLoadStrategy::Normal
        );
        assert_eq!(
            "EAGER".parse::<PageLoadStrategy>().unwrap(),
            PageLoadStrategy::Eager
        );
        assert_eq!(
            " None ".parse::<PageLoadStrategy>().unwrap(),
            PageLoadStrategy::None
        );
        assert!("fast".parse::<PageLoadStrategy>().is_err());
        assert!("".parse::<PageLoadStrategy>().is_err());
    }

    #[test]
    fn the_capability_value_is_the_lowercase_name() {
        for strategy in [
            PageLoadStrategy::Normal,
            PageLoadStrategy::Eager,
            PageLoadStrategy::None,
        ] {
            let driver: thirtyfour::common::capabilities::desiredcapabilities::PageLoadStrategy =
                strategy.into();
            assert_eq!(
                serde_json::to_value(driver).unwrap(),
                serde_json::json!(strategy.as_str())
            );
            assert_eq!(strategy.to_string(), strategy.as_str());
        }
    }
}
