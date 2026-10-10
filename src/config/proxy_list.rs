//! A list of proxy servers to pick from.
//!
//! Python SeleniumBase keeps a "phone book" of proxies, `PROXY_LIST`, so a run
//! can say `--proxy=proxy1` instead of spelling out an address. A
//! [`ProxyList`] is that phone book, and also a pool: it can hand out its
//! proxies in turn or at random, which is how a set of parallel tests spreads
//! across several proxies.
//!
//! Every entry is checked when it is added, so a typo is an error with the
//! line number instead of a browser that cannot connect.
//!
//! # The file format
//!
//! One proxy per line, optionally given a name. Blank lines and `#` comments
//! are ignored.
//!
//! ```text
//! # office proxies
//! proxy1 = 10.0.0.5:3128
//! proxy2 = alice:s3cret@proxy.example.com:8080
//! socks = socks5://10.0.0.9:1080
//! 192.0.2.7:8080
//! ```
//!
//! A line is `name = proxy` when the part before the first `=` is a plain name
//! (letters, digits, `_` and `-`); otherwise the whole line is the proxy. A
//! proxy is `host:port`, with an optional `user:password@` in front and an
//! optional `http://`, `https://`, `socks4://` or `socks5://`.
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::config::proxy_list::ProxyList;
//! use seleniumbase_rs::BrowserConfig;
//!
//! let list = ProxyList::parse("proxy1 = 10.0.0.5:3128\nproxy2 = 10.0.0.6:3128\n")?;
//!
//! // `--proxy=proxy1`: a name is looked up, anything else must be a proxy.
//! assert_eq!(list.resolve("proxy1")?, "10.0.0.5:3128");
//! assert_eq!(list.resolve("192.0.2.1:8080")?, "192.0.2.1:8080");
//!
//! // Hand them out in turn.
//! let first = list.next_round_robin().expect("a proxy");
//! let config = BrowserConfig::default().with_proxy(first.spec());
//! assert_eq!(config.proxy.as_deref(), Some("10.0.0.5:3128"));
//! # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
//! ```

use std::fmt;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

use rand::RngExt;
use url::Url;

use super::strip_comment;
use crate::error::SeleniumBaseError;
use crate::sb_cdp::Proxy;

/// Proxy schemes Chrome accepts in `--proxy-server`.
const SCHEMES: [&str; 4] = ["http", "https", "socks4", "socks5"];

/// One proxy in a [`ProxyList`], with the name it can be asked for by.
///
/// The password, if there is one, is part of [`spec`](Self::spec) and nowhere
/// else: [`Debug`](fmt::Debug) and [`Display`](fmt::Display) show only the
/// server.
#[derive(Clone)]
pub struct ProxyEntry {
    name: Option<String>,
    spec: String,
    proxy: Proxy,
}

impl ProxyEntry {
    /// The name this entry is looked up by, if it has one.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// The proxy as written, ready for `BrowserConfig::proxy`. **It contains
    /// the password**, so do not log it.
    #[must_use]
    pub fn spec(&self) -> &str {
        &self.spec
    }

    /// The parsed proxy.
    #[must_use]
    pub fn proxy(&self) -> &Proxy {
        &self.proxy
    }

    /// The `host:port` (with scheme, if one was given) of the proxy; safe to
    /// show.
    #[must_use]
    pub fn server(&self) -> &str {
        self.proxy.server()
    }
}

impl fmt::Debug for ProxyEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProxyEntry")
            .field("name", &self.name)
            .field("proxy", &self.proxy)
            .finish()
    }
}

impl fmt::Display for ProxyEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.name {
            Some(name) => write!(f, "{name} ({})", self.server()),
            None => f.write_str(self.server()),
        }
    }
}

/// An ordered list of proxies, some of them named.
///
/// It is safe to share between threads: taking the next proxy needs only `&self`.
#[derive(Debug, Default)]
pub struct ProxyList {
    entries: Vec<ProxyEntry>,
    cursor: AtomicUsize,
}

impl Clone for ProxyList {
    fn clone(&self) -> Self {
        Self {
            entries: self.entries.clone(),
            cursor: AtomicUsize::new(self.cursor.load(Ordering::Relaxed)),
        }
    }
}

impl ProxyList {
    /// An empty list.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Reads a list in the file format described in the [module
    /// documentation](self).
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] naming the first bad line
    /// (never its password), including a name that is used twice.
    pub fn parse(text: &str) -> Result<Self, SeleniumBaseError> {
        let mut list = Self::new();
        for (index, line) in text.lines().enumerate() {
            let line = strip_comment(line).trim();
            if line.is_empty() {
                continue;
            }
            let (name, spec) = split_name(line);
            list.insert(name, spec).map_err(|error| {
                SeleniumBaseError::invalid_config(format!("proxy list line {}: {error}", index + 1))
            })?;
        }
        Ok(list)
    }

    /// Reads a list from a file.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Io`] if the file cannot be read, or the
    /// error of [`parse`](Self::parse).
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, SeleniumBaseError> {
        Self::parse(&std::fs::read_to_string(path)?)
    }

    /// Adds a proxy to the end of the list.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] if `spec` is not a valid
    /// proxy, or if `name` is given and already in use or not a plain name.
    pub fn insert(&mut self, name: Option<&str>, spec: &str) -> Result<(), SeleniumBaseError> {
        if let Some(name) = name {
            if !is_name(name) {
                return Err(SeleniumBaseError::invalid_config(format!(
                    "{name:?} is not a proxy name (use letters, digits, '_' and '-')"
                )));
            }
            if self.get(name).is_some() {
                return Err(SeleniumBaseError::invalid_config(format!(
                    "the proxy name {name:?} is used twice"
                )));
            }
        }
        let spec = spec.trim();
        let proxy = Proxy::parse(spec)?;
        check_server(proxy.server())?;
        self.entries.push(ProxyEntry {
            name: name.map(str::to_owned),
            spec: spec.to_owned(),
            proxy,
        });
        Ok(())
    }

    /// Adds an unnamed proxy to the end of the list.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] if `spec` is not a valid
    /// proxy.
    pub fn add(&mut self, spec: &str) -> Result<(), SeleniumBaseError> {
        self.insert(None, spec)
    }

    /// The entry with this name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&ProxyEntry> {
        self.entries
            .iter()
            .find(|entry| entry.name.as_deref() == Some(name))
    }

    /// The proxy to use for `--proxy=VALUE`: the entry named `value`, or else
    /// `value` itself if it is a valid proxy. **The result contains any
    /// password**; do not log it.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] if `value` is neither a
    /// name in the list nor a valid proxy.
    pub fn resolve(&self, value: &str) -> Result<String, SeleniumBaseError> {
        let value = value.trim();
        if let Some(entry) = self.get(value) {
            return Ok(entry.spec.clone());
        }
        // A bare word can only be meant as a name: no proxy address is one
        // (it needs a port). Only a bare word is echoed back, because
        // anything else may carry a password.
        if is_name(value) {
            let names: Vec<&str> = self.entries.iter().filter_map(ProxyEntry::name).collect();
            let known = if names.is_empty() {
                "the list has no named proxies".to_owned()
            } else {
                format!("known names: {}", names.join(", "))
            };
            return Err(SeleniumBaseError::invalid_config(format!(
                "no proxy named {value:?} in the proxy list ({known})"
            )));
        }
        let proxy = Proxy::parse(value)?;
        check_server(proxy.server())?;
        Ok(value.to_owned())
    }

    /// The next proxy in turn, starting from the first and wrapping around.
    /// `None` if the list is empty.
    #[must_use]
    pub fn next_round_robin(&self) -> Option<&ProxyEntry> {
        if self.entries.is_empty() {
            return None;
        }
        let turn = self.cursor.fetch_add(1, Ordering::Relaxed);
        self.entries.get(turn % self.entries.len())
    }

    /// A proxy chosen at random. `None` if the list is empty.
    #[must_use]
    pub fn random(&self) -> Option<&ProxyEntry> {
        if self.entries.is_empty() {
            return None;
        }
        let index = rand::rng().random_range(0..self.entries.len());
        self.entries.get(index)
    }

    /// The entries, in order.
    pub fn iter(&self) -> std::slice::Iter<'_, ProxyEntry> {
        self.entries.iter()
    }

    /// How many proxies are in the list.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the list has no proxies.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl<'a> IntoIterator for &'a ProxyList {
    type Item = &'a ProxyEntry;
    type IntoIter = std::slice::Iter<'a, ProxyEntry>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Splits `name = proxy`, or returns the whole line as the proxy.
fn split_name(line: &str) -> (Option<&str>, &str) {
    match line.split_once('=') {
        Some((name, spec)) if is_name(name.trim()) && !spec.trim().is_empty() => {
            (Some(name.trim()), spec.trim())
        }
        _ => (None, line),
    }
}

/// A plain name: letters, digits, `_` and `-`, starting with a letter or `_`.
fn is_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Checks the credential-free part of a proxy: a known scheme, a host name or
/// address, and a port. Chrome silently ignores a proxy it cannot read, so
/// this is where a typo is caught.
fn check_server(server: &str) -> Result<(), SeleniumBaseError> {
    let bad = |reason: &str| {
        SeleniumBaseError::invalid_config(format!("proxy {server:?} is not valid: {reason}"))
    };
    let (scheme, authority) = server.split_once("://").unwrap_or(("http", server));
    if !SCHEMES.contains(&scheme.to_ascii_lowercase().as_str()) {
        return Err(bad("the scheme must be http, https, socks4 or socks5"));
    }
    let (host, port) = authority
        .rsplit_once(':')
        .ok_or_else(|| bad("a port is required, as in host:8080"))?;
    if !matches!(port.parse::<u16>(), Ok(1..)) {
        return Err(bad("the port must be a number from 1 to 65535"));
    }
    let valid_host = !host.is_empty()
        && Url::parse(&format!("http://{host}/"))
            .is_ok_and(|url| url.host().is_some() && url.username().is_empty());
    if !valid_host {
        return Err(bad("the host is not a valid name or address"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(specs: &[&str]) -> ProxyList {
        let mut list = ProxyList::new();
        for spec in specs {
            list.add(spec).expect("a valid proxy");
        }
        list
    }

    fn servers(list: &ProxyList) -> Vec<&str> {
        list.iter().map(ProxyEntry::server).collect()
    }

    #[test]
    fn proxies_are_handed_out_in_turn_and_wrap_around() {
        let list = list(&["a.test:8080", "b.test:8080"]);
        let order: Vec<_> = (0..5)
            .map(|_| list.next_round_robin().unwrap().server())
            .collect();
        assert_eq!(
            order,
            [
                "a.test:8080",
                "b.test:8080",
                "a.test:8080",
                "b.test:8080",
                "a.test:8080"
            ]
        );
    }

    #[test]
    fn an_empty_list_has_no_next_and_no_random_proxy() {
        let list = ProxyList::new();
        assert!(list.is_empty());
        assert!(list.next_round_robin().is_none());
        assert!(list.random().is_none());
    }

    #[test]
    fn a_random_proxy_is_a_member_and_every_member_can_come_up() {
        let list = list(&["a.test:1", "b.test:2", "c.test:3"]);
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..300 {
            let pick = list.random().unwrap();
            assert!(servers(&list).contains(&pick.server()));
            seen.insert(pick.server().to_owned());
        }
        assert_eq!(seen.len(), 3, "300 draws from 3 should reach each");
    }

    #[test]
    fn the_turn_is_shared_between_threads_without_a_repeat() {
        let list = list(&["a.test:1", "b.test:2", "c.test:3", "d.test:4"]);
        let counts = std::sync::Mutex::new(std::collections::BTreeMap::<String, usize>::new());
        std::thread::scope(|scope| {
            for _ in 0..4 {
                scope.spawn(|| {
                    for _ in 0..100 {
                        let server = list.next_round_robin().unwrap().server().to_owned();
                        *counts.lock().unwrap().entry(server).or_default() += 1;
                    }
                });
            }
        });
        let counts = counts.into_inner().unwrap();
        assert_eq!(
            counts.values().copied().collect::<Vec<_>>(),
            [100, 100, 100, 100]
        );
    }

    #[test]
    fn the_file_format_has_names_comments_and_credentials() {
        let list = ProxyList::parse(
            "# office proxies\n\
             \n\
             proxy1 = 10.0.0.5:3128\n\
             proxy-2=alice:s3cret@proxy.example.com:8080  # with a login\n\
             socks = socks5://10.0.0.9:1080\n\
             192.0.2.7:8080\n",
        )
        .unwrap();

        assert_eq!(list.len(), 4);
        assert_eq!(list.get("proxy1").unwrap().server(), "10.0.0.5:3128");
        let with_login = list.get("proxy-2").unwrap();
        assert_eq!(with_login.server(), "proxy.example.com:8080");
        assert_eq!(with_login.spec(), "alice:s3cret@proxy.example.com:8080");
        assert!(with_login.proxy().has_credentials());
        assert_eq!(
            list.get("socks").unwrap().server(),
            "socks5://10.0.0.9:1080"
        );
        assert_eq!(list.iter().last().unwrap().name(), None);
    }

    #[test]
    fn a_password_may_contain_an_equals_sign() {
        let list = ProxyList::parse("alice:pa=ss@proxy.test:8080\n").unwrap();
        let entry = list.iter().next().unwrap();
        assert_eq!(entry.name(), None);
        assert_eq!(entry.spec(), "alice:pa=ss@proxy.test:8080");
    }

    #[test]
    fn a_bad_line_is_reported_by_number_without_its_password() {
        let error = ProxyList::parse("ok.test:80\n\nbob:topsecret@no-port.test\n")
            .unwrap_err()
            .to_string();
        assert!(error.contains("line 3"), "{error}");
        assert!(!error.contains("topsecret"), "{error}");
    }

    #[test]
    fn invalid_proxies_are_refused() {
        for bad in [
            "",
            "no-port.test",
            "host.test:0",
            "host.test:65536",
            "host.test:http",
            "ftp://host.test:21",
            "host with space.test:80",
            ":8080",
            "user@:8080",
        ] {
            assert!(list_add(bad).is_err(), "{bad:?} must be refused");
        }
    }

    fn list_add(spec: &str) -> Result<(), SeleniumBaseError> {
        ProxyList::new().add(spec)
    }

    #[test]
    fn ipv6_and_all_the_chrome_schemes_are_accepted() {
        for good in [
            "[::1]:8080",
            "http://h.test:80",
            "https://h.test:443",
            "socks4://h.test:1080",
            "SOCKS5://h.test:1080",
            "u:p@h.test:1",
        ] {
            assert!(list_add(good).is_ok(), "{good:?} must be accepted");
        }
    }

    #[test]
    fn a_name_is_used_once_and_must_be_plain() {
        let mut list = ProxyList::new();
        list.insert(Some("p1"), "a.test:1").unwrap();
        assert!(list.insert(Some("p1"), "b.test:2").is_err());
        assert!(list.insert(Some("two words"), "b.test:2").is_err());
        assert!(list.insert(Some("1st"), "b.test:2").is_err());
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn resolving_prefers_a_name_and_otherwise_validates() {
        let list = ProxyList::parse("proxy1 = u:p@a.test:1\n").unwrap();

        assert_eq!(list.resolve("proxy1").unwrap(), "u:p@a.test:1");
        assert_eq!(list.resolve("  b.test:2 ").unwrap(), "b.test:2");
        let unknown = list.resolve("proxy2").unwrap_err().to_string();
        assert!(unknown.contains("no proxy named \"proxy2\""), "{unknown}");
        assert!(unknown.contains("known names: proxy1"), "{unknown}");
    }

    #[test]
    fn resolving_a_bad_proxy_does_not_echo_it() {
        let list = ProxyList::new();
        let error = list
            .resolve("bob:topsecret@no-port.test")
            .unwrap_err()
            .to_string();
        assert!(!error.contains("topsecret"), "{error}");
    }

    #[test]
    fn debug_and_display_never_show_a_password() {
        let list = ProxyList::parse("p1 = alice:hunter2-secret@proxy.test:8080\n").unwrap();
        let entry = list.get("p1").unwrap();

        for shown in [
            format!("{entry:?}"),
            format!("{entry}"),
            format!("{list:?}"),
        ] {
            assert!(!shown.contains("hunter2"), "{shown}");
            assert!(!shown.contains("alice"), "{shown}");
            assert!(shown.contains("proxy.test:8080"), "{shown}");
        }
    }

    #[test]
    fn a_clone_continues_from_the_same_turn() {
        let list = list(&["a.test:1", "b.test:2"]);
        assert_eq!(list.next_round_robin().unwrap().server(), "a.test:1");
        let copy = list.clone();
        assert_eq!(copy.next_round_robin().unwrap().server(), "b.test:2");
        assert_eq!(list.next_round_robin().unwrap().server(), "b.test:2");
    }

    #[test]
    fn a_list_can_be_read_from_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("proxies.txt");
        std::fs::write(&path, "p1 = a.test:1\n").unwrap();

        assert_eq!(ProxyList::from_file(&path).unwrap().len(), 1);
        assert!(ProxyList::from_file(dir.path().join("missing.txt")).is_err());
    }

    #[test]
    fn iterating_a_list_by_reference_visits_every_entry() {
        let list = list(&["a.test:1", "b.test:2"]);
        let mut count = 0;
        for entry in &list {
            assert!(entry.server().ends_with(['1', '2']));
            count += 1;
        }
        assert_eq!(count, 2);
    }
}
