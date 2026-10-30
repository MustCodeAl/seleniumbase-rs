//! Host lists for blocking advertising and tracking requests.
//!
//! Python SeleniumBase blocks ads with a bundled browser extension. A browser
//! driven over the DevTools Protocol can do it without one, by telling Chrome
//! which URLs never to load (`Network.setBlockedURLs`); this module is the
//! list those URLs come from.
//!
//! An [`AdBlockList`] is a set of host names. A host in the list blocks itself
//! and every subdomain of it, so `doubleclick.net` also blocks
//! `googleads.g.doubleclick.net`. [`AdBlockList::builtin`] is the crate's own
//! starter list, [`AdBlockList::parse`] reads the host-based rules of the
//! common list formats, and [`AdBlockList::url_patterns`] turns a list into
//! the patterns Chrome wants.
//!
//! A [`BrowserConfig`] with `ad_block` set gives the built-in list to a
//! Chromium WebDriver session, which sends it to the browser with
//! `Network.setBlockedURLs`; a list of your own, set with
//! [`BrowserConfig::with_ad_block_list`], replaces the built-in one. The
//! browser is sent [`AdBlockList::bounded_url_patterns`], which refuses a list
//! too long to match every request against.
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::config::ad_block_list::AdBlockList;
//!
//! let mut list = AdBlockList::builtin();
//! list.add_host("ads.example.com")?;
//!
//! assert!(list.is_blocked("https://googleads.g.doubleclick.net/pagead/id"));
//! assert!(list.is_blocked("https://ads.example.com/banner.png"));
//! assert!(!list.is_blocked("https://example.com/"));
//! # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
//! ```

use std::collections::BTreeSet;
use std::path::Path;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use url::{Host, Url};

use super::strip_comment;
use crate::browser::config::BrowserConfig;
use crate::error::SeleniumBaseError;

/// The built-in hosts: well-known advertising, analytics and fingerprinting
/// endpoints, sorted.
///
/// Deliberately short and uncontroversial. This is the union of the lists the
/// Pure CDP ad blocker and `Fingerprint::tracker_hosts` used to keep
/// separately, so a host is added in one place.
const BUILTIN_HOSTS: [&str; 22] = [
    "adnxs.com",
    "adservice.google.com",
    "adsrvr.org",
    "advertising.com",
    "amazon-adsystem.com",
    "amplitude.com",
    "cdn.fingerprint.com",
    "criteo.com",
    "doubleclick.net",
    "facebook.net",
    "fingerprintjs.com",
    "fpjs.io",
    "google-analytics.com",
    "googleadservices.com",
    "googlesyndication.com",
    "googletagmanager.com",
    "hotjar.com",
    "mixpanel.com",
    "outbrain.com",
    "scorecardresearch.com",
    "segment.io",
    "taboola.com",
];

/// The most patterns one `Network.setBlockedURLs` call carries, which is
/// 1,000 hosts at two patterns each.
///
/// Chrome compares every request with every pattern in turn, so a long list
/// slows each request down. A list that needs more is refused rather than cut
/// short, because a block list applied in part would look like one applied in
/// full.
pub const MAX_URL_PATTERNS: usize = 2_000;

/// Names that hosts files map to the loopback address; they are not ad hosts.
const LOOPBACK_NAMES: [&str; 6] = [
    "localhost",
    "localhost.localdomain",
    "local",
    "broadcasthost",
    "ip6-localhost",
    "ip6-loopback",
];

/// A set of host names whose requests are blocked, with their subdomains.
///
/// Hosts are stored in lower case without a scheme, port, path, leading dot or
/// wildcard, and are kept sorted.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AdBlockList {
    hosts: BTreeSet<String>,
}

/// An [`AdBlockList`] read from text, and how much of the text was unusable.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ParsedList {
    /// The hosts that were found.
    pub list: AdBlockList,
    /// Rules that were understood as rules but cannot be expressed as a host
    /// block, such as cosmetic filters, exceptions, or rules with a path or
    /// options. Comments and blank lines are not counted.
    pub skipped: usize,
}

impl AdBlockList {
    /// An empty list, which blocks nothing.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The crate's starter list of advertising and tracking hosts.
    #[must_use]
    pub fn builtin() -> Self {
        Self {
            hosts: BUILTIN_HOSTS
                .iter()
                .map(|host| (*host).to_owned())
                .collect(),
        }
    }

    /// The list `config` asks to block, otherwise `None`: the list set with
    /// [`BrowserConfig::with_ad_block_list`] if there is one, else the built-in
    /// list, when `config.ad_block` is on.
    ///
    /// A list that is set but not asked for (`ad_block` is `false`) blocks
    /// nothing. A list of one's own that is empty blocks nothing either; it is
    /// not replaced by the built-in one.
    #[must_use]
    pub fn for_config(config: &BrowserConfig) -> Option<Self> {
        config.ad_block.then(|| {
            config
                .launch
                .ad_block_list
                .clone()
                .unwrap_or_else(Self::builtin)
        })
    }

    /// A list of the given hosts.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] for the first entry that
    /// is not a valid host name; see [`add_host`](Self::add_host).
    pub fn from_hosts<I, S>(hosts: I) -> Result<Self, SeleniumBaseError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut list = Self::new();
        for host in hosts {
            list.add_host(host.as_ref())?;
        }
        Ok(list)
    }

    /// Reads host rules from the text of a block list.
    ///
    /// Three kinds of line are understood: a bare host (`ads.example.com`), a
    /// hosts-file line (`0.0.0.0 ads.example.com tracker.example.net`), and an
    /// Adblock Plus domain rule (`||ads.example.com^`). Comment lines (`#`,
    /// `!`), list headers (`[...]`) and blank lines are ignored. Anything else
    /// is counted in [`ParsedList::skipped`], so a big list that is mostly
    /// rules this cannot express is noticed rather than silently half-applied.
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::config::ad_block_list::AdBlockList;
    ///
    /// let parsed = AdBlockList::parse(
    ///     "! A list\n||ads.example.com^\n0.0.0.0 tracker.example.net\n@@||ok.example.com^\n",
    /// );
    /// assert_eq!(parsed.list.len(), 2);
    /// assert_eq!(parsed.skipped, 1, "the exception rule");
    /// ```
    #[must_use]
    pub fn parse(text: &str) -> ParsedList {
        let mut parsed = ParsedList::default();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty()
                || line.starts_with('#')
                || line.starts_with('!')
                || (line.starts_with('[') && line.ends_with(']'))
            {
                continue;
            }
            match Self::hosts_in_line(line) {
                Some(hosts) => {
                    for host in hosts {
                        parsed.list.hosts.insert(host);
                    }
                }
                None => parsed.skipped += 1,
            }
        }
        parsed
    }

    /// Reads a block list from a file; see [`parse`](Self::parse).
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Io`] if the file cannot be read.
    pub fn from_file(path: impl AsRef<Path>) -> Result<ParsedList, SeleniumBaseError> {
        Ok(Self::parse(&std::fs::read_to_string(path)?))
    }

    /// The hosts of one list line, or `None` if the line is not a host rule.
    fn hosts_in_line(line: &str) -> Option<Vec<String>> {
        // Adblock Plus domain rule: ||host^ (and nothing else).
        if let Some(rule) = line.strip_prefix("||") {
            let host = rule.strip_suffix('^').unwrap_or(rule);
            return normalize_host(host).map(|host| vec![host]);
        }
        // Hosts file: an address, then one or more names, then maybe a comment.
        let mut words = strip_comment(line).split_whitespace();
        let first = words.next()?;
        let rest: Vec<&str> = words.collect();
        if is_loopback_address(first) {
            let hosts: Vec<String> = rest
                .iter()
                .filter(|name| !LOOPBACK_NAMES.contains(name))
                .filter_map(|name| normalize_host(name))
                .collect();
            // A hosts file maps localhost to 127.0.0.1; that line is no rule
            // and no loss, but an unreadable name is.
            return (!hosts.is_empty() || rest.iter().all(|n| LOOPBACK_NAMES.contains(n)))
                .then_some(hosts);
        }
        // A bare host on its own.
        if rest.is_empty() {
            return normalize_host(first).map(|host| vec![host]);
        }
        None
    }

    /// Adds a host to the list.
    ///
    /// Accepts a host name optionally written as `*.example.com` or
    /// `.example.com`, in any case, with a trailing dot. Returns `true` if the
    /// host was not already in the list.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] if `host` is not a plain
    /// host name with at least two labels: a URL, a path, a port, an address,
    /// or an empty string is refused rather than quietly never matching.
    pub fn add_host(&mut self, host: &str) -> Result<bool, SeleniumBaseError> {
        let host = normalize_host(host).ok_or_else(|| {
            SeleniumBaseError::invalid_config(format!(
                "not a host name for an ad-block list: {host:?}"
            ))
        })?;
        Ok(self.hosts.insert(host))
    }

    /// Whether `host` is in the list, exactly. See
    /// [`is_host_blocked`](Self::is_host_blocked) for subdomains.
    #[must_use]
    pub fn contains(&self, host: &str) -> bool {
        normalize_host(host).is_some_and(|host| self.hosts.contains(&host))
    }

    /// Whether requests to `host` are blocked: it, or a parent domain of it,
    /// is in the list.
    #[must_use]
    pub fn is_host_blocked(&self, host: &str) -> bool {
        let host = host.trim_end_matches('.').to_ascii_lowercase();
        let mut candidate = host.as_str();
        loop {
            if self.hosts.contains(candidate) {
                return true;
            }
            match candidate.split_once('.') {
                Some((_, parent)) => candidate = parent,
                None => return false,
            }
        }
    }

    /// Whether a request to `url` is blocked.
    ///
    /// A string that is not an absolute URL with a host name (a relative path,
    /// a `data:` URL, an IP address) is never blocked. A URL written without a
    /// scheme but with `//` in front is read as `https`.
    #[must_use]
    pub fn is_blocked(&self, url: &str) -> bool {
        let parsed = Url::parse(url).or_else(|_| {
            url.strip_prefix("//").map_or_else(
                || Url::parse(url),
                |rest| Url::parse(&format!("https://{rest}")),
            )
        });
        match parsed.as_ref().map(Url::host) {
            Ok(Some(Host::Domain(domain))) => self.is_host_blocked(domain),
            _ => false,
        }
    }

    /// The hosts in the list, sorted.
    pub fn hosts(&self) -> impl Iterator<Item = &str> {
        self.hosts.iter().map(String::as_str)
    }

    /// The URL patterns that make Chrome refuse these hosts, for
    /// `Network.setBlockedURLs`.
    ///
    /// Each host gives two patterns, `*://host/*` and `*://*.host/*`: the
    /// second alone would let the bare domain through.
    ///
    /// A pattern ends the host with a `/`, so it does not match a URL that
    /// names a port (`https://host:8443/`); ad and tracker hosts are served on
    /// the default ports. [`is_blocked`](Self::is_blocked) ignores the port.
    #[must_use]
    pub fn url_patterns(&self) -> Vec<String> {
        self.hosts
            .iter()
            .flat_map(|host| [format!("*://{host}/*"), format!("*://*.{host}/*")])
            .collect()
    }

    /// The patterns to give `Network.setBlockedURLs`: [`url_patterns`] without
    /// the hosts that a parent domain in the list already covers, and never
    /// more than [`MAX_URL_PATTERNS`].
    ///
    /// `ads.example.com` is left out when `example.com` is in the list, because
    /// `*://*.example.com/*` matches it.
    ///
    /// [`url_patterns`]: Self::url_patterns
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] if more than
    /// [`MAX_URL_PATTERNS`] patterns are left, naming the count. The list is
    /// not cut short: trim it, or block fewer hosts.
    pub fn bounded_url_patterns(&self) -> Result<Vec<String>, SeleniumBaseError> {
        let patterns: Vec<String> = self
            .hosts
            .iter()
            .filter(|host| !self.has_listed_parent(host))
            .flat_map(|host| [format!("*://{host}/*"), format!("*://*.{host}/*")])
            .collect();
        if patterns.len() > MAX_URL_PATTERNS {
            return Err(SeleniumBaseError::invalid_config(format!(
                "the ad-block list needs {} URL patterns, more than the {MAX_URL_PATTERNS} \
                 the browser is given; remove hosts, or list a parent domain in place \
                 of its subdomains",
                patterns.len()
            )));
        }
        Ok(patterns)
    }

    /// Whether a parent domain of `host` is in the list, so that the parent's
    /// wildcard already covers `host`.
    fn has_listed_parent(&self, host: &str) -> bool {
        let mut rest = host;
        while let Some((_, parent)) = rest.split_once('.') {
            if self.hosts.contains(parent) {
                return true;
            }
            rest = parent;
        }
        false
    }

    /// How many hosts are in the list.
    #[must_use]
    pub fn len(&self) -> usize {
        self.hosts.len()
    }

    /// Whether the list blocks nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.hosts.is_empty()
    }
}

/// Written as the sorted array of its hosts, which is how a configuration file
/// holds one: `["ads.example.com", "tracker.example.net"]`.
impl Serialize for AdBlockList {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(&self.hosts)
    }
}

/// Read from an array of hosts; an entry that is not a host name is an error,
/// as with [`AdBlockList::from_hosts`].
impl<'de> Deserialize<'de> for AdBlockList {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let hosts = Vec::<String>::deserialize(deserializer)?;
        Self::from_hosts(hosts).map_err(serde::de::Error::custom)
    }
}

/// Whether a hosts-file line starts with a loopback or unspecified address.
fn is_loopback_address(word: &str) -> bool {
    matches!(
        word,
        "0.0.0.0" | "127.0.0.1" | "::" | "::1" | "0:0:0:0:0:0:0:0"
    )
}

/// Lower-cases a host and strips a leading `*.` or `.` and a trailing `.`.
///
/// Returns `None` unless the result is a plain name of at least two
/// non-empty labels made of letters, digits, hyphens and underscores.
fn normalize_host(raw: &str) -> Option<String> {
    let host = raw.trim();
    let host = host.strip_prefix("*.").unwrap_or(host);
    let host = host.strip_prefix('.').unwrap_or(host);
    let host = host.strip_suffix('.').unwrap_or(host).to_ascii_lowercase();
    let labels_ok = host.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    });
    let is_address = host.bytes().all(|b| b.is_ascii_digit() || b == b'.');
    (host.len() <= 253 && host.contains('.') && labels_ok && !is_address).then_some(host)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evasion::fingerprint::default_tracker_hosts;

    #[test]
    fn a_host_blocks_itself_and_its_subdomains() {
        let list = AdBlockList::from_hosts(["doubleclick.net"]).unwrap();
        assert!(list.is_blocked("https://doubleclick.net/"));
        assert!(list.is_blocked("https://googleads.g.doubleclick.net/track"));
        assert!(!list.is_blocked("https://notdoubleclick.net/"));
        assert!(!list.is_blocked("https://doubleclick.net.evil.test/"));
    }

    #[test]
    fn matching_ignores_case_port_credentials_and_a_trailing_dot() {
        let list = AdBlockList::from_hosts(["ads.example.com"]).unwrap();
        assert!(list.is_blocked("HTTPS://ADS.Example.COM:8443/x?y=1#z"));
        assert!(list.is_blocked("http://user:pw@ads.example.com/"));
        assert!(list.is_blocked("https://ads.example.com./"));
        assert!(list.is_blocked("wss://ads.example.com/socket"));
    }

    #[test]
    fn a_hosts_name_inside_the_path_or_query_does_not_block() {
        let list = AdBlockList::from_hosts(["ads.example.com"]).unwrap();
        assert!(!list.is_blocked("https://example.com/redirect?to=ads.example.com"));
        assert!(!list.is_blocked("https://example.com/ads.example.com/x"));
        assert!(!list.is_blocked("https://user:ads.example.com@example.com/"));
    }

    #[test]
    fn what_is_not_a_url_with_a_host_name_is_not_blocked() {
        let list = AdBlockList::from_hosts(["example.com"]).unwrap();
        assert!(!list.is_blocked("/relative/example.com"));
        assert!(!list.is_blocked("data:text/html,example.com"));
        assert!(!list.is_blocked("http://93.184.216.34/"));
        assert!(!list.is_blocked(""));
    }

    #[test]
    fn a_url_without_a_scheme_but_with_slashes_is_read_as_https() {
        let list = AdBlockList::from_hosts(["ads.example.com"]).unwrap();
        assert!(list.is_blocked("//ads.example.com/tag.js"));
    }

    #[test]
    fn an_empty_list_blocks_nothing() {
        let list = AdBlockList::new();
        assert!(list.is_empty());
        assert!(!list.is_blocked("https://any-site.com/"));
    }

    #[test]
    fn hosts_are_normalised_and_deduplicated() {
        let mut list = AdBlockList::new();
        assert!(list.add_host("Tracker.IO").unwrap());
        assert!(!list.add_host("*.tracker.io").unwrap());
        assert!(!list.add_host(".tracker.io.").unwrap());
        assert_eq!(list.len(), 1);
        assert!(list.contains("TRACKER.io"));
        assert!(list.is_blocked("http://tracker.io/pixel.gif"));
    }

    #[test]
    fn things_that_are_not_host_names_are_refused() {
        let mut list = AdBlockList::new();
        for bad in [
            "",
            "https://ads.example.com",
            "ads.example.com/banner",
            "ads.example.com:8080",
            "localhost",
            "127.0.0.1",
            "-.example.com",
            "a..example.com",
            "exa mple.com",
            "*.*.example.com",
        ] {
            assert!(list.add_host(bad).is_err(), "{bad:?} must be refused");
        }
        assert!(list.is_empty());
    }

    #[test]
    fn from_hosts_stops_at_the_first_bad_entry() {
        let result = AdBlockList::from_hosts(["good.example.com", "not a host"]);
        assert!(result.is_err());
    }

    #[test]
    fn url_patterns_cover_the_bare_domain_and_the_subdomains() {
        let list = AdBlockList::from_hosts(["b.example.com", "a.example.com"]).unwrap();
        assert_eq!(
            list.url_patterns(),
            [
                "*://a.example.com/*",
                "*://*.a.example.com/*",
                "*://b.example.com/*",
                "*://*.b.example.com/*",
            ]
        );
    }

    #[test]
    fn a_bare_host_hosts_file_and_abp_rules_are_all_read() {
        let parsed = AdBlockList::parse(
            "[Adblock Plus 2.0]\n\
             ! Title: test\n\
             # a comment\n\
             \n\
             bare.example.com\n\
             0.0.0.0 hosts-one.example.com hosts-two.example.com # trailing comment\n\
             127.0.0.1 localhost\n\
             ::1 ip6-localhost\n\
             ||abp.example.com^\n\
             ||abp-no-caret.example.com\n",
        );
        let hosts: Vec<_> = parsed.list.hosts().collect();
        assert_eq!(
            hosts,
            [
                "abp-no-caret.example.com",
                "abp.example.com",
                "bare.example.com",
                "hosts-one.example.com",
                "hosts-two.example.com",
            ]
        );
        assert_eq!(parsed.skipped, 0);
    }

    #[test]
    fn rules_that_are_not_host_blocks_are_counted_not_applied() {
        let parsed = AdBlockList::parse(
            "@@||allowed.example.com^\n\
             ||ads.example.com^$third-party\n\
             ||example.org/banners/\n\
             example.net##.advert\n\
             /ads/*.gif\n\
             ||kept.example.com^\n",
        );
        assert_eq!(
            parsed.list.hosts().collect::<Vec<_>>(),
            ["kept.example.com"]
        );
        assert_eq!(parsed.skipped, 5);
    }

    #[test]
    fn a_list_can_be_read_from_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("list.txt");
        std::fs::write(&path, "||ads.example.com^\n").unwrap();

        let parsed = AdBlockList::from_file(&path).unwrap();

        assert!(parsed.list.is_blocked("https://ads.example.com/"));
        assert!(AdBlockList::from_file(dir.path().join("missing.txt")).is_err());
    }

    #[test]
    fn the_builtin_list_is_sorted_and_valid() {
        let list = AdBlockList::builtin();
        assert_eq!(list.len(), BUILTIN_HOSTS.len(), "no duplicates");
        let mut sorted = BUILTIN_HOSTS;
        sorted.sort_unstable();
        assert_eq!(sorted, BUILTIN_HOSTS);
        for host in BUILTIN_HOSTS {
            assert_eq!(normalize_host(host).as_deref(), Some(host));
        }
    }

    #[test]
    fn the_builtin_list_covers_the_hosts_the_other_blockers_use() {
        let list = AdBlockList::builtin();
        // `Fingerprint::tracker_hosts` defaults.
        for host in default_tracker_hosts() {
            assert!(list.is_host_blocked(&host), "{host} is a tracker host");
        }
        // The Pure CDP ad blocker's patterns (`AD_BLOCK_PATTERNS` in
        // `sb_cdp/browser.rs`) as hosts.
        for host in [
            "doubleclick.net",
            "googlesyndication.com",
            "googleadservices.com",
            "google-analytics.com",
            "googletagmanager.com",
            "adnxs.com",
            "adsrvr.org",
            "advertising.com",
            "amazon-adsystem.com",
            "criteo.com",
            "outbrain.com",
            "taboola.com",
            "scorecardresearch.com",
            "facebook.net",
        ] {
            assert!(list.is_host_blocked(host), "{host} is an ad host");
        }
    }

    #[test]
    fn a_config_gets_the_builtin_list_only_when_it_asks_for_ad_blocking() {
        let off = BrowserConfig::default();
        assert!(AdBlockList::for_config(&off).is_none());

        let on = BrowserConfig {
            ad_block: true,
            ..BrowserConfig::default()
        };
        assert_eq!(AdBlockList::for_config(&on), Some(AdBlockList::builtin()));
    }

    #[test]
    fn a_list_of_the_callers_own_replaces_the_builtin_one() {
        let own = AdBlockList::from_hosts(["ads.example.com"]).unwrap();
        let on = BrowserConfig::default()
            .with_ad_block(true)
            .with_ad_block_list(own.clone());
        assert_eq!(AdBlockList::for_config(&on), Some(own.clone()));

        // A list that is set but not asked for blocks nothing.
        let mut off = on.clone();
        off.ad_block = false;
        assert!(AdBlockList::for_config(&off).is_none());

        // An empty list of one's own is not the built-in list in disguise.
        let empty = BrowserConfig::default().with_ad_block_list(AdBlockList::new());
        assert_eq!(AdBlockList::for_config(&empty), Some(AdBlockList::new()));
    }

    #[test]
    fn setting_a_list_of_ones_own_turns_ad_blocking_on() {
        let config = BrowserConfig::default()
            .with_ad_block_list(AdBlockList::from_hosts(["ads.example.com"]).unwrap());
        assert!(config.ad_block);
    }

    #[test]
    fn bounded_patterns_are_the_url_patterns_when_no_host_is_covered() {
        let list = AdBlockList::from_hosts(["b.example.com", "a.example.net"]).unwrap();
        assert_eq!(list.bounded_url_patterns().unwrap(), list.url_patterns());
        assert_eq!(
            AdBlockList::builtin().bounded_url_patterns().unwrap().len(),
            2 * BUILTIN_HOSTS.len()
        );
        assert!(AdBlockList::new()
            .bounded_url_patterns()
            .unwrap()
            .is_empty());
    }

    #[test]
    fn bounded_patterns_leave_out_a_host_that_a_parent_in_the_list_covers() {
        let list = AdBlockList::from_hosts([
            "example.com",
            "ads.example.com",
            "x.ads.example.com",
            "example.org",
            "badexample.com",
        ])
        .unwrap();
        assert_eq!(
            list.bounded_url_patterns().unwrap(),
            [
                "*://badexample.com/*",
                "*://*.badexample.com/*",
                "*://example.com/*",
                "*://*.example.com/*",
                "*://example.org/*",
                "*://*.example.org/*",
            ]
        );
        // What was left out is still blocked, by the parent's wildcard.
        assert!(list.is_blocked("https://x.ads.example.com/a.js"));
    }

    #[test]
    fn a_list_over_the_limit_is_refused_and_one_at_it_is_not() {
        let hosts = |count: usize| {
            AdBlockList::from_hosts((0..count).map(|i| format!("host{i}.com"))).unwrap()
        };
        let at_limit = hosts(MAX_URL_PATTERNS / 2);
        assert_eq!(
            at_limit.bounded_url_patterns().unwrap().len(),
            MAX_URL_PATTERNS
        );

        let over = hosts(MAX_URL_PATTERNS / 2 + 1);
        let Err(SeleniumBaseError::InvalidConfig(message)) = over.bounded_url_patterns() else {
            panic!("a list over the limit must be refused");
        };
        assert!(
            message.contains(&(MAX_URL_PATTERNS + 2).to_string()),
            "names the pattern count: {message}"
        );
        assert!(
            message.contains(&MAX_URL_PATTERNS.to_string()),
            "names the limit: {message}"
        );
    }

    #[test]
    fn a_list_is_written_and_read_as_an_array_of_hosts() {
        let list = AdBlockList::from_hosts(["b.example.com", "a.example.com"]).unwrap();
        let text = serde_json::to_string(&list).unwrap();
        assert_eq!(text, r#"["a.example.com","b.example.com"]"#);
        assert_eq!(serde_json::from_str::<AdBlockList>(&text).unwrap(), list);
        // Entries are read like `add_host` reads them.
        let loose: AdBlockList =
            serde_json::from_str(r#"["*.Tracker.IO", ".tracker.io."]"#).unwrap();
        assert_eq!(loose.hosts().collect::<Vec<_>>(), ["tracker.io"]);
        assert!(serde_json::from_str::<AdBlockList>(r#"["not a host"]"#).is_err());
        assert!(serde_json::from_str::<AdBlockList>(r#"{"hosts":[]}"#).is_err());
    }
}
