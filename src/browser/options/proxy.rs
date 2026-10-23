//! Proxies: the address of a proxy server or of a PAC file, checked the way
//! Python SeleniumBase checks it, and the hosts that skip the proxy.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

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

/// How a browser talks to its proxy server.
///
/// The text before `://` in a proxy address; an address without one is an HTTP
/// proxy, as in Python SeleniumBase.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum ProxyScheme {
    /// A plain HTTP proxy, the default.
    #[default]
    Http,
    /// An HTTP proxy that the browser reaches over TLS.
    Https,
    /// A SOCKS version 4 proxy.
    Socks4,
    /// A SOCKS version 5 proxy.
    Socks5,
    /// A SOCKS version 5 proxy that resolves host names itself.
    Socks5h,
}

impl ProxyScheme {
    /// The scheme as it is written in an address.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Https => "https",
            Self::Socks4 => "socks4",
            Self::Socks5 => "socks5",
            Self::Socks5h => "socks5h",
        }
    }

    /// Whether it is one of the SOCKS schemes.
    #[must_use]
    pub const fn is_socks(self) -> bool {
        matches!(self, Self::Socks4 | Self::Socks5 | Self::Socks5h)
    }

    fn from_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "http" => Some(Self::Http),
            "https" => Some(Self::Https),
            "socks4" => Some(Self::Socks4),
            "socks5" => Some(Self::Socks5),
            "socks5h" => Some(Self::Socks5h),
            _ => None,
        }
    }
}

/// Splits the `user:password@` login off `text`, which is a proxy address or the
/// URL of a PAC file (`has_path`), and returns it with `text` as it is without
/// it.
///
/// The login may come first (`user:pass@http://host/x.pac`, `user:pass@host:1`)
/// or sit in the authority of a URL (`http://user:pass@host/x.pac`).
fn split_login(text: &str, has_path: bool) -> (Option<&str>, String) {
    let scheme_end = text.find("://");
    if let (Some(at), Some(scheme)) = (text.find('@'), scheme_end) {
        if at < scheme {
            return (Some(&text[..at]), text[at + 1..].to_owned());
        }
    }
    let start = scheme_end.map_or(0, |scheme| scheme + 3);
    let end = if has_path {
        text[start..]
            .find('/')
            .map_or(text.len(), |slash| start + slash)
    } else {
        text.len()
    };
    match text[start..end].rfind('@') {
        Some(at) => {
            let at = start + at;
            (
                Some(&text[start..at]),
                format!("{}{}", &text[..start], &text[at + 1..]),
            )
        }
        None => (None, text.to_owned()),
    }
}

/// `text` with the password of its login hidden (`user:***@`), for an error
/// message or a log.
fn redacted(text: &str, has_path: bool) -> String {
    let (login, rest) = split_login(text, has_path);
    let Some(login) = login else {
        return rest;
    };
    let shown = format!("{}:***@", login.split(':').next().unwrap_or_default());
    if text.starts_with(login) && text[login.len()..].starts_with('@') {
        return format!("{shown}{rest}");
    }
    match rest.split_once("://") {
        Some((scheme, host)) => format!("{scheme}://{shown}{host}"),
        None => format!("{shown}{rest}"),
    }
}

/// `user:password` as a pair, or `None` when `login` is not one.
fn credentials_of(login: &str) -> Option<(String, String)> {
    let (user, password) = login.split_once(':')?;
    (!user.is_empty()).then(|| (user.to_owned(), password.to_owned()))
}

/// A proxy server written `[scheme://][user:password@]host:port`.
///
/// It is checked the way Python SeleniumBase checks `--proxy`: the schemes are
/// `http` (the default), `https`, `socks4`, `socks5` and `socks5h`, the port is
/// a number, and a SOCKS proxy cannot take a login in Chrome or Firefox. An
/// error shows the address with the password hidden (`user:***@`), and so do
/// [`Debug`](fmt::Debug) and [`Display`](fmt::Display).
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::browser::options::{ProxyAddress, ProxyScheme};
///
/// let proxy: ProxyAddress = "user:secret@proxy.example:8080".parse()?;
/// assert_eq!(proxy.scheme(), ProxyScheme::Http);
/// assert_eq!(proxy.credentials(), Some(("user", "secret")));
/// // Chrome is never given the login.
/// assert_eq!(proxy.chrome_server(), "proxy.example:8080");
/// assert_eq!(proxy.to_string(), "http://user:***@proxy.example:8080");
///
/// let socks: ProxyAddress = "socks5://10.0.0.1:1080".parse()?;
/// assert_eq!(socks.chrome_server(), "socks5://10.0.0.1:1080");
/// assert!("ftp://proxy.example:21".parse::<ProxyAddress>().is_err());
/// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
/// ```
#[derive(Clone, Eq, PartialEq)]
pub struct ProxyAddress {
    scheme: ProxyScheme,
    host: String,
    port: u16,
    credentials: Option<(String, String)>,
}

impl ProxyAddress {
    /// How the browser talks to the proxy.
    #[must_use]
    pub const fn scheme(&self) -> ProxyScheme {
        self.scheme
    }

    /// The proxy's host name or address (an IPv6 address keeps its brackets).
    #[must_use]
    pub fn host(&self) -> &str {
        &self.host
    }

    /// The proxy's port.
    #[must_use]
    pub const fn port(&self) -> u16 {
        self.port
    }

    /// The user name and password, when the address has them.
    #[must_use]
    pub fn credentials(&self) -> Option<(&str, &str)> {
        self.credentials
            .as_ref()
            .map(|(user, password)| (user.as_str(), password.as_str()))
    }

    /// The value of Chrome's `--proxy-server`: `host:port` for an HTTP proxy,
    /// `scheme://host:port` for the others, and never a login, which Chrome
    /// does not read there and a process list would show. Chrome's `socks5`
    /// already resolves names at the proxy, so `socks5h` is written as that.
    #[must_use]
    pub fn chrome_server(&self) -> String {
        let Self { host, port, .. } = self;
        match self.scheme {
            ProxyScheme::Http => format!("{host}:{port}"),
            ProxyScheme::Https => format!("https://{host}:{port}"),
            ProxyScheme::Socks4 => format!("socks4://{host}:{port}"),
            ProxyScheme::Socks5 | ProxyScheme::Socks5h => format!("socks5://{host}:{port}"),
        }
    }

    /// The `network.proxy.*` preferences that make Firefox use this proxy, and
    /// skip it for the hosts of `bypass`.
    ///
    /// An HTTP or HTTPS proxy is set for both HTTP and TLS traffic; a SOCKS
    /// proxy gets its version, and `socks5h` also asks for the names to be
    /// resolved at the proxy.
    #[must_use]
    pub fn firefox_preferences(&self, bypass: &ProxyBypassList) -> Map<String, Value> {
        let mut prefs = Map::new();
        prefs.insert("network.proxy.type".into(), json!(1));
        if self.scheme.is_socks() {
            prefs.insert("network.proxy.socks".into(), json!(self.host));
            prefs.insert("network.proxy.socks_port".into(), json!(self.port));
            let version = if self.scheme == ProxyScheme::Socks4 {
                4
            } else {
                5
            };
            prefs.insert("network.proxy.socks_version".into(), json!(version));
            if self.scheme == ProxyScheme::Socks5h {
                prefs.insert("network.proxy.socks_remote_dns".into(), json!(true));
            }
        } else {
            for protocol in ["http", "ssl"] {
                prefs.insert(format!("network.proxy.{protocol}"), json!(self.host));
                prefs.insert(format!("network.proxy.{protocol}_port"), json!(self.port));
            }
        }
        if !bypass.is_empty() {
            // Firefox writes a whole domain as `.foo.com`, not `*.foo.com`.
            let hosts: Vec<&str> = bypass
                .hosts()
                .iter()
                .map(|host| host.strip_prefix('*').unwrap_or(host))
                .collect();
            prefs.insert(
                "network.proxy.no_proxies_on".into(),
                json!(hosts.join(", ")),
            );
        }
        prefs
    }
}

impl FromStr for ProxyAddress {
    type Err = SeleniumBaseError;

    /// Reads `[scheme://][user:password@]host:port`.
    ///
    /// # Errors
    ///
    /// Returns an error for another scheme, a host that is not a name or an
    /// address, a port that is not 1 to 65535, a login without a user name and
    /// a password, or a login on a SOCKS proxy. The password is never part of
    /// the message.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let text = text.trim();
        let invalid = || {
            SeleniumBaseError::invalid_config(format!(
                "Proxy string \"{}\" is NOT in the expected \"ip_address:port\" or \
                 \"server:port\" format (schemes: http, https, socks4, socks5, socks5h)",
                redacted(text, false)
            ))
        };
        let (login, rest) = split_login(text, false);
        let credentials = match login {
            Some(login) => Some(credentials_of(login).ok_or_else(invalid)?),
            None => None,
        };
        let (scheme, address) = match rest.split_once("://") {
            Some((name, address)) => (ProxyScheme::from_name(name).ok_or_else(invalid)?, address),
            None => (ProxyScheme::Http, rest.as_str()),
        };
        let address = address.strip_suffix('/').unwrap_or(address);
        let (host, port) = address.rsplit_once(':').ok_or_else(invalid)?;
        let plain_host = !host.is_empty()
            && host
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'));
        let ipv6_host = host.len() > 2
            && host.starts_with('[')
            && host.ends_with(']')
            && host[1..host.len() - 1]
                .bytes()
                .all(|b| b.is_ascii_hexdigit() || matches!(b, b':' | b'.'));
        if !plain_host && !ipv6_host {
            return Err(invalid());
        }
        let port = port
            .bytes()
            .all(|b| b.is_ascii_digit())
            .then(|| port.parse::<u16>().ok())
            .flatten()
            .filter(|port| *port != 0)
            .ok_or_else(invalid)?;
        if scheme.is_socks() && credentials.is_some() {
            return Err(SeleniumBaseError::invalid_config(format!(
                "a SOCKS proxy cannot take a username and password in Chrome or Firefox: {}",
                redacted(text, false)
            )));
        }
        Ok(Self {
            scheme,
            host: host.to_owned(),
            port,
            credentials,
        })
    }
}

impl fmt::Display for ProxyAddress {
    /// Writes `scheme://[user:***@]host:port`: the password is hidden.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}://", self.scheme.name())?;
        if let Some((user, _)) = &self.credentials {
            write!(f, "{user}:***@")?;
        }
        write!(f, "{}:{}", self.host, self.port)
    }
}

impl fmt::Debug for ProxyAddress {
    // By hand: the password must not reach a log.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ProxyAddress")
            .field(&format_args!("{self}"))
            .finish()
    }
}

/// The URL of a proxy auto-config (PAC) file, written `[user:password@]URL`.
///
/// Python SeleniumBase takes it as `--proxy-pac-url`. The URL must end in
/// `.pac`; a missing scheme means `http://`. A login may come first
/// (`user:pass@http://host/proxy.pac`) or sit in the URL
/// (`http://user:pass@host/proxy.pac`); either way [`url`](Self::url) has none,
/// since a browser is never given it, and [`Debug`](fmt::Debug) and
/// [`Display`](fmt::Display) hide the password.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::browser::options::PacUrl;
///
/// let pac: PacUrl = "user:secret@http://pac.example/proxy.pac".parse()?;
/// assert_eq!(pac.url(), "http://pac.example/proxy.pac");
/// assert_eq!(pac.credentials(), Some(("user", "secret")));
/// assert!("http://pac.example/proxy.js".parse::<PacUrl>().is_err());
/// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
/// ```
#[derive(Clone, Eq, PartialEq)]
pub struct PacUrl {
    url: String,
    credentials: Option<(String, String)>,
}

impl PacUrl {
    /// The URL of the PAC file, without a login.
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    /// The user name and password, when the address has them.
    #[must_use]
    pub fn credentials(&self) -> Option<(&str, &str)> {
        self.credentials
            .as_ref()
            .map(|(user, password)| (user.as_str(), password.as_str()))
    }

    /// The `network.proxy.*` preferences that make Firefox use the file.
    #[must_use]
    pub fn firefox_preferences(&self) -> Map<String, Value> {
        let mut prefs = Map::new();
        prefs.insert("network.proxy.type".into(), json!(2));
        prefs.insert("network.proxy.autoconfig_url".into(), json!(self.url));
        prefs
    }
}

impl FromStr for PacUrl {
    type Err = SeleniumBaseError;

    /// Reads `[user:password@]URL`.
    ///
    /// # Errors
    ///
    /// Returns an error when the URL does not end in `.pac` (any letter case),
    /// has a scheme other than `http`, `https` or `file`, or the login is not
    /// `user:password`. The password is never part of the message.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let text = text.trim();
        let invalid = || {
            SeleniumBaseError::invalid_config(format!(
                "The proxy PAC URL must end with \".pac\": {}",
                redacted(text, true)
            ))
        };
        let (login, rest) = split_login(text, true);
        let credentials = match login {
            Some(login) => Some(credentials_of(login).ok_or_else(invalid)?),
            None => None,
        };
        let url = if rest.contains("://") {
            rest
        } else {
            format!("http://{rest}")
        };
        let scheme_ok = url.split_once("://").is_some_and(|(scheme, _)| {
            ["http", "https", "file"]
                .iter()
                .any(|known| scheme.eq_ignore_ascii_case(known))
        });
        if !scheme_ok || !url.to_ascii_lowercase().ends_with(".pac") {
            return Err(invalid());
        }
        Ok(Self { url, credentials })
    }
}

impl fmt::Display for PacUrl {
    /// Writes the URL with `user:***@` in front when there is a login.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some((user, _)) = &self.credentials {
            write!(f, "{user}:***@")?;
        }
        f.write_str(&self.url)
    }
}

impl fmt::Debug for PacUrl {
    // By hand: the password must not reach a log.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("PacUrl")
            .field(&format_args!("{self}"))
            .finish()
    }
}

#[cfg(test)]
mod address_tests {
    use super::*;

    fn parse(text: &str) -> ProxyAddress {
        text.parse()
            .unwrap_or_else(|error| panic!("{text:?} was refused: {error}"))
    }

    #[test]
    fn the_proxy_strings_python_takes_are_read() {
        let cases = [
            ("1.2.3.4:80", ProxyScheme::Http, "1.2.3.4", 80, None),
            (
                "proxy.example:3128",
                ProxyScheme::Http,
                "proxy.example",
                3128,
                None,
            ),
            ("localhost:8080", ProxyScheme::Http, "localhost", 8080, None),
            ("http://h:8080/", ProxyScheme::Http, "h", 8080, None),
            ("HTTPS://h:443", ProxyScheme::Https, "h", 443, None),
            ("socks4://h:1080", ProxyScheme::Socks4, "h", 1080, None),
            (
                "socks5://10.0.0.1:1080",
                ProxyScheme::Socks5,
                "10.0.0.1",
                1080,
                None,
            ),
            ("socks5h://h:1080", ProxyScheme::Socks5h, "h", 1080, None),
            ("[::1]:8080", ProxyScheme::Http, "[::1]", 8080, None),
            ("  h:1  ", ProxyScheme::Http, "h", 1, None),
            (
                "u:secret@h:1",
                ProxyScheme::Http,
                "h",
                1,
                Some(("u", "secret")),
            ),
            (
                "https://u:p@h:2",
                ProxyScheme::Https,
                "h",
                2,
                Some(("u", "p")),
            ),
            ("u:p:q@h:3", ProxyScheme::Http, "h", 3, Some(("u", "p:q"))),
            ("u:p@w@h:3", ProxyScheme::Http, "h", 3, Some(("u", "p@w"))),
            ("u:@h:4", ProxyScheme::Http, "h", 4, Some(("u", ""))),
        ];
        for (text, scheme, host, port, credentials) in cases {
            let proxy = parse(text);
            assert_eq!(proxy.scheme(), scheme, "{text}");
            assert_eq!(proxy.host(), host, "{text}");
            assert_eq!(proxy.port(), port, "{text}");
            assert_eq!(proxy.credentials(), credentials, "{text}");
        }
    }

    #[test]
    fn anything_else_is_refused_and_the_password_stays_out_of_the_message() {
        for text in [
            "",
            "host",
            "host:",
            ":80",
            "host:notaport",
            "host:0",
            "host:70000",
            "host:-1",
            "ftp://host:21",
            "http://",
            "a b:1",
            "host:1:2",
            "user@host:1",
            ":pass@host:1",
            "[zz]:80",
            "ftp://user:hunter2@host:21",
            "user:hunter2@host:notaport",
        ] {
            let error = text.parse::<ProxyAddress>().unwrap_err().to_string();
            assert!(
                error.contains("is NOT in the expected"),
                "{text:?}: {error}"
            );
            assert!(!error.contains("hunter2"), "{error}");
        }
        let error = "ftp://user:hunter2@host:21"
            .parse::<ProxyAddress>()
            .unwrap_err()
            .to_string();
        assert!(error.contains("ftp://user:***@host:21"), "{error}");
    }

    #[test]
    fn a_socks_proxy_cannot_take_a_login() {
        for scheme in ["socks4", "socks5", "socks5h"] {
            let error = format!("{scheme}://user:hunter2@host:1080")
                .parse::<ProxyAddress>()
                .unwrap_err()
                .to_string();
            assert!(
                error.contains("a SOCKS proxy cannot take a username and password"),
                "{error}"
            );
            assert!(error.contains("user:***@host:1080"), "{error}");
            assert!(!error.contains("hunter2"), "{error}");
        }
        assert!("socks5://host:1080".parse::<ProxyAddress>().is_ok());
    }

    #[test]
    fn chrome_is_never_given_the_login() {
        for (text, server) in [
            ("u:secret@h:1", "h:1"),
            ("http://u:secret@h:1", "h:1"),
            ("https://u:secret@h:2", "https://h:2"),
            ("socks4://h:3", "socks4://h:3"),
            ("socks5://h:4", "socks5://h:4"),
            ("socks5h://h:5", "socks5://h:5"),
            ("[::1]:6", "[::1]:6"),
        ] {
            assert_eq!(parse(text).chrome_server(), server, "{text}");
        }
    }

    #[test]
    fn the_password_is_hidden_when_a_proxy_is_shown() {
        let proxy = parse("socks5://h:1080");
        assert_eq!(proxy.to_string(), "socks5://h:1080");
        let proxy = parse("user:hunter2@h:1");
        assert_eq!(proxy.to_string(), "http://user:***@h:1");
        assert!(!format!("{proxy:?}").contains("hunter2"));
        assert!(format!("{proxy:?}").contains("user:***@h:1"));
    }

    #[test]
    fn firefox_gets_the_preferences_of_the_scheme() {
        let none = ProxyBypassList::default();
        let http = parse("h:8080").firefox_preferences(&none);
        assert_eq!(
            Value::Object(http),
            json!({
                "network.proxy.type": 1,
                "network.proxy.http": "h",
                "network.proxy.http_port": 8080,
                "network.proxy.ssl": "h",
                "network.proxy.ssl_port": 8080,
            })
        );
        let socks4 = parse("socks4://h:1080").firefox_preferences(&none);
        assert_eq!(
            Value::Object(socks4),
            json!({
                "network.proxy.type": 1,
                "network.proxy.socks": "h",
                "network.proxy.socks_port": 1080,
                "network.proxy.socks_version": 4,
            })
        );
        let socks5 = parse("socks5://h:1080").firefox_preferences(&none);
        assert_eq!(socks5["network.proxy.socks_version"], 5);
        assert!(!socks5.contains_key("network.proxy.socks_remote_dns"));
        let remote = parse("socks5h://h:1080").firefox_preferences(&none);
        assert_eq!(remote["network.proxy.socks_remote_dns"], true);
        assert!(!remote.contains_key("network.proxy.http"));
    }

    #[test]
    fn the_bypass_list_becomes_firefoxs_no_proxies_on() {
        let bypass: ProxyBypassList = "*.example.com;localhost;10.0.0.0/8".parse().unwrap();
        let prefs = parse("h:1").firefox_preferences(&bypass);
        assert_eq!(
            prefs["network.proxy.no_proxies_on"],
            ".example.com, localhost, 10.0.0.0/8"
        );
        let prefs = parse("h:1").firefox_preferences(&ProxyBypassList::default());
        assert!(!prefs.contains_key("network.proxy.no_proxies_on"));
    }

    #[test]
    fn a_pac_url_is_read_with_or_without_a_login() {
        let cases = [
            (
                "http://pac.example/proxy.pac",
                "http://pac.example/proxy.pac",
                None,
            ),
            (
                "pac.example/proxy.pac",
                "http://pac.example/proxy.pac",
                None,
            ),
            (
                "https://pac.example/a/B.PAC",
                "https://pac.example/a/B.PAC",
                None,
            ),
            ("file:///tmp/proxy.pac", "file:///tmp/proxy.pac", None),
            (
                "user:secret@http://pac.example/proxy.pac",
                "http://pac.example/proxy.pac",
                Some(("user", "secret")),
            ),
            (
                "http://user:secret@pac.example/proxy.pac",
                "http://pac.example/proxy.pac",
                Some(("user", "secret")),
            ),
            (
                "user:secret@pac.example/proxy.pac",
                "http://pac.example/proxy.pac",
                Some(("user", "secret")),
            ),
        ];
        for (text, url, credentials) in cases {
            let pac: PacUrl = text.parse().unwrap();
            assert_eq!(pac.url(), url, "{text}");
            assert_eq!(pac.credentials(), credentials, "{text}");
        }
    }

    #[test]
    fn a_pac_url_must_end_in_pac_and_the_message_hides_the_password() {
        for text in [
            "http://pac.example/proxy.js",
            "http://pac.example/",
            "ftp://pac.example/proxy.pac",
            "user:hunter2@http://pac.example/proxy.js",
            "http://user:hunter2@pac.example/proxy.js",
            "justauser@http://pac.example/proxy.pac",
        ] {
            let error = text.parse::<PacUrl>().unwrap_err().to_string();
            assert!(
                error.contains("The proxy PAC URL must end with \".pac\": "),
                "{text}: {error}"
            );
            assert!(!error.contains("hunter2"), "{error}");
        }
        let error = "user:hunter2@http://pac.example/proxy.js"
            .parse::<PacUrl>()
            .unwrap_err()
            .to_string();
        assert!(
            error.ends_with("user:***@http://pac.example/proxy.js"),
            "{error}"
        );
        let pac: PacUrl = "user:hunter2@pac.example/proxy.pac".parse().unwrap();
        assert!(!format!("{pac:?} {pac}").contains("hunter2"));
    }

    #[test]
    fn firefox_reads_a_pac_file_through_its_autoconfig_url() {
        let pac: PacUrl = "http://pac.example/proxy.pac".parse().unwrap();
        assert_eq!(
            Value::Object(pac.firefox_preferences()),
            json!({
                "network.proxy.type": 2,
                "network.proxy.autoconfig_url": "http://pac.example/proxy.pac",
            })
        );
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
