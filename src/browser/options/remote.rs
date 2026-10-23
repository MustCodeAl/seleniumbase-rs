//! The address of a remote WebDriver server, such as a Selenium Grid.

use std::fmt;
use std::str::FromStr;

use crate::error::SeleniumBaseError;

/// The protocol a Selenium Grid is reached over (`--protocol`).
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::browser::options::GridProtocol;
///
/// assert_eq!("https".parse::<GridProtocol>()?, GridProtocol::Https);
/// assert_eq!(GridProtocol::default().to_string(), "http");
/// assert!("ftp".parse::<GridProtocol>().is_err());
/// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
/// ```
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum GridProtocol {
    /// Plain HTTP, the default.
    #[default]
    Http,
    /// HTTP over TLS.
    Https,
}

impl GridProtocol {
    /// The port the protocol uses when a URL does not name one.
    #[must_use]
    pub const fn default_port(self) -> u16 {
        match self {
            Self::Http => 80,
            Self::Https => 443,
        }
    }
}

impl fmt::Display for GridProtocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Http => "http",
            Self::Https => "https",
        })
    }
}

impl FromStr for GridProtocol {
    type Err = SeleniumBaseError;

    /// Reads `http` or `https`, in any letter case.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text.trim().to_ascii_lowercase().as_str() {
            "http" => Ok(Self::Http),
            "https" => Ok(Self::Https),
            _ => Err(SeleniumBaseError::parse(text, "a protocol: http or https")),
        }
    }
}

/// The WebDriver URL of a Selenium Grid, the way Python SeleniumBase builds it
/// from `--protocol`, `--server` and `--port`.
///
/// - A server written `https://...` makes the protocol `https`; any other scheme
///   in front of the name is dropped.
/// - The server may carry a login, `USER:KEY@hub.browserstack.com`, which stays
///   in the URL (a WebDriver client sends it as Basic authentication).
/// - `port` is added unless the server names one already, or it is the
///   protocol's own port (80 for `http`, 443 for `https`), which a URL leaves
///   out.
/// - `/wd/hub` is added unless the server's path ends in it.
///
/// # Errors
///
/// Returns an error when `servername` is empty.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::browser::options::{grid_url, GridProtocol};
///
/// assert_eq!(
///     grid_url(GridProtocol::Http, "USER:KEY@hub.browserstack.com", 80)?,
///     "http://USER:KEY@hub.browserstack.com/wd/hub"
/// );
/// assert_eq!(
///     grid_url(GridProtocol::Http, "127.0.0.1", 4444)?,
///     "http://127.0.0.1:4444/wd/hub"
/// );
/// assert_eq!(
///     grid_url(GridProtocol::Http, "https://hub.example.com:8443/wd/hub", 4444)?,
///     "https://hub.example.com:8443/wd/hub"
/// );
/// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
/// ```
pub fn grid_url(
    protocol: GridProtocol,
    servername: &str,
    port: u16,
) -> Result<String, SeleniumBaseError> {
    let text = servername.trim();
    if text.is_empty() {
        return Err(SeleniumBaseError::invalid_config(
            "a Selenium Grid server name is needed, such as hub.example.com or \
             USER:KEY@hub.browserstack.com",
        ));
    }
    let (protocol, rest) = match text.split_once("://") {
        Some((scheme, rest)) if scheme.eq_ignore_ascii_case("https") => (GridProtocol::Https, rest),
        Some((_, rest)) => (protocol, rest),
        None => (protocol, text),
    };
    let (authority, path) = rest.split_once('/').map_or((rest, ""), |(a, p)| (a, p));
    let (login, host_port) = authority
        .rsplit_once('@')
        .map_or(("", authority), |(login, host)| (login, host));
    let names_a_port = host_port
        .rsplit_once(':')
        .is_some_and(|(_, port)| !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()));
    let mut url = format!("{protocol}://");
    if !login.is_empty() {
        url.push_str(login);
        url.push('@');
    }
    url.push_str(host_port);
    if !names_a_port && port != protocol.default_port() {
        url.push_str(&format!(":{port}"));
    }
    let path = path.trim_matches('/');
    if path.is_empty() {
        url.push_str("/wd/hub");
    } else if path == "wd/hub" || path.ends_with("/wd/hub") {
        url.push('/');
        url.push_str(path);
    } else {
        url.push('/');
        url.push_str(path);
        url.push_str("/wd/hub");
    }
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_grids_python_users_name_become_webdriver_urls() {
        let cases = [
            (
                GridProtocol::Http,
                "USER:KEY@hub.browserstack.com",
                80,
                "http://USER:KEY@hub.browserstack.com/wd/hub",
            ),
            (
                GridProtocol::Https,
                "USER:KEY@ondemand.us-west-1.saucelabs.com",
                443,
                "https://USER:KEY@ondemand.us-west-1.saucelabs.com/wd/hub",
            ),
            (
                GridProtocol::Http,
                "https://ondemand.saucelabs.com",
                443,
                "https://ondemand.saucelabs.com/wd/hub",
            ),
            (
                GridProtocol::Http,
                "127.0.0.1",
                4444,
                "http://127.0.0.1:4444/wd/hub",
            ),
            (
                GridProtocol::Http,
                "hub.example.com:5555",
                4444,
                "http://hub.example.com:5555/wd/hub",
            ),
            (
                GridProtocol::Http,
                "USER:KEY@hub.example.com",
                4444,
                "http://USER:KEY@hub.example.com:4444/wd/hub",
            ),
            (
                GridProtocol::Http,
                "http://hub.example.com:4444/wd/hub",
                1,
                "http://hub.example.com:4444/wd/hub",
            ),
            (
                GridProtocol::Http,
                "hub.example.com:4444/wd/hub/",
                1,
                "http://hub.example.com:4444/wd/hub",
            ),
            (
                GridProtocol::Http,
                "hub.example.com/prefix",
                4444,
                "http://hub.example.com:4444/prefix/wd/hub",
            ),
            (
                GridProtocol::Http,
                "ftp://hub.example.com",
                4444,
                "http://hub.example.com:4444/wd/hub",
            ),
            (
                GridProtocol::Http,
                "  hub.example.com  ",
                4444,
                "http://hub.example.com:4444/wd/hub",
            ),
        ];
        for (protocol, server, port, expected) in cases {
            assert_eq!(
                grid_url(protocol, server, port).unwrap(),
                expected,
                "{server} {port}"
            );
        }
    }

    #[test]
    fn a_grid_without_a_name_is_refused() {
        for server in ["", "   "] {
            let error = grid_url(GridProtocol::Http, server, 4444).unwrap_err();
            assert!(error.to_string().contains("server name"), "{error}");
        }
    }

    #[test]
    fn the_protocol_reads_back_what_it_writes() {
        for protocol in [GridProtocol::Http, GridProtocol::Https] {
            assert_eq!(
                protocol.to_string().parse::<GridProtocol>().unwrap(),
                protocol
            );
        }
        assert_eq!(
            " HTTPS ".parse::<GridProtocol>().unwrap(),
            GridProtocol::Https
        );
        assert_eq!(GridProtocol::Https.default_port(), 443);
        assert_eq!(GridProtocol::Http.default_port(), 80);
    }
}
