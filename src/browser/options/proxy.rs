//! Hosts that skip the proxy.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::SeleniumBaseError;

/// Hosts that the browser reaches directly instead of through the proxy.
///
/// Python SeleniumBase takes it as `--proxy-bypass-list="*.foo.com;localhost"`:
/// host patterns separated by semicolons. Chrome understands the same patterns
/// (`*.foo.com`, `.foo.com`, `10.0.0.0/8`, `<local>` for names without a dot).
/// It only applies to a proxy named with
/// [`with_proxy`](crate::BrowserConfig::with_proxy); a PAC file decides for
/// itself which hosts skip the proxy.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::browser::options::ProxyBypassList;
///
/// let list: ProxyBypassList = "*.example.com; localhost".parse()?;
/// assert_eq!(list.hosts(), ["*.example.com", "localhost"]);
/// assert_eq!(list.to_chrome_value().as_deref(), Some("*.example.com;localhost"));
/// assert!(ProxyBypassList::default().to_chrome_value().is_none());
/// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
/// ```
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ProxyBypassList(Vec<String>);

impl ProxyBypassList {
    /// A list of `hosts`. Each is trimmed and empty ones are dropped.
    ///
    /// # Errors
    ///
    /// Returns an error if a host contains a semicolon, a comma or whitespace,
    /// which Chrome would read as two entries.
    pub fn new(
        hosts: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<Self, SeleniumBaseError> {
        let mut list = Vec::new();
        for host in hosts {
            let host = host.as_ref().trim();
            if host.is_empty() {
                continue;
            }
            if host.contains([';', ',']) || host.contains(char::is_whitespace) {
                return Err(SeleniumBaseError::parse(
                    host,
                    "one host pattern without semicolons, commas or spaces",
                ));
            }
            list.push(host.to_owned());
        }
        Ok(Self(list))
    }

    /// The host patterns.
    #[must_use]
    pub fn hosts(&self) -> &[String] {
        &self.0
    }

    /// Whether no host skips the proxy.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The value of Chrome's `--proxy-bypass-list`, or `None` for an empty list.
    #[must_use]
    pub fn to_chrome_value(&self) -> Option<String> {
        (!self.0.is_empty()).then(|| self.0.join(";"))
    }
}

impl fmt::Display for ProxyBypassList {
    /// Formats as the semicolon-separated text Python takes.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0.join(";"))
    }
}

impl FromStr for ProxyBypassList {
    type Err = SeleniumBaseError;

    /// Reads patterns separated by semicolons or commas.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::new(text.split([';', ',']))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns_split_on_semicolons_and_commas_and_are_trimmed() {
        let list: ProxyBypassList = "*.foo.com; localhost,10.0.0.0/8;;".parse().unwrap();
        assert_eq!(list.hosts(), ["*.foo.com", "localhost", "10.0.0.0/8"]);
        assert_eq!(list.to_string(), "*.foo.com;localhost;10.0.0.0/8");
    }

    #[test]
    fn an_empty_list_passes_no_flag() {
        assert!(ProxyBypassList::default().to_chrome_value().is_none());
        assert!("".parse::<ProxyBypassList>().unwrap().is_empty());
        assert!(" ; ".parse::<ProxyBypassList>().unwrap().is_empty());
    }

    #[test]
    fn a_pattern_chrome_would_split_in_two_is_refused() {
        assert!(ProxyBypassList::new(["a;b"]).is_err());
        assert!(ProxyBypassList::new(["a,b"]).is_err());
        assert!(ProxyBypassList::new(["a b"]).is_err());
        assert!(ProxyBypassList::new(["a", "<local>"]).is_ok());
    }

    #[test]
    fn it_is_written_as_a_plain_list() {
        let list = ProxyBypassList::new(["a.com", "b.com"]).unwrap();
        assert_eq!(
            serde_json::to_string(&list).unwrap(),
            r#"["a.com","b.com"]"#
        );
        assert_eq!(
            serde_json::from_str::<ProxyBypassList>(r#"["a.com","b.com"]"#).unwrap(),
            list
        );
    }
}
