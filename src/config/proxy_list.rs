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
//! assert_eq!(config.browser.proxy.server.as_deref(), Some("10.0.0.5:3128"));
//! # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
//! ```

use std::collections::HashMap;
use std::fmt;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, SystemTime};

use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};
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

/// The longest a proxy rests, so that a rest of any length can be added to a time.
const MAX_REST: Duration = Duration::from_secs(100 * 365 * 24 * 60 * 60);

/// How many sites [`RotationStrategy::StickyPerSite`] remembers; past this the
/// memory starts again, so a long run that visits many sites stays bounded.
pub const MAX_STICKY_SITES: usize = 4096;

/// How [`ProxyList::pick`] chooses the next proxy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum RotationStrategy {
    /// In turn, wrapping around, as [`ProxyList::next_round_robin`] does.
    #[default]
    RoundRobin,
    /// At random. With a seed the order is the same for the same seed, so a run
    /// can be repeated; without one it is [`ProxyList::random`]'s pick.
    Random {
        /// The seed of the order, or `None` for a fresh one each time.
        seed: Option<u64>,
    },
    /// The proxy with the fewest failures [reported](ProxyList::report) since
    /// its last success; ties go in turn.
    LeastFailures,
    /// The same proxy for the same site, chosen in turn the first time the site
    /// is asked for.
    StickyPerSite,
}

/// What happened to a request through a proxy, as [`ProxyList::report`]
/// records it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProxyOutcome {
    /// The request went through.
    Success,
    /// The proxy could not be reached or refused the connection.
    ConnectFailed,
    /// The site answered with a challenge.
    Challenged,
    /// The site refused the request.
    Blocked,
}

/// What the rotation strategies remember between picks.
#[derive(Debug, Default, Clone)]
struct Rotation {
    health: Vec<Health>,
    sticky: HashMap<String, usize>,
    draws: u64,
}

impl Rotation {
    /// Keeps one health record per entry, as entries are added.
    fn fit(&mut self, len: usize) {
        if self.health.len() < len {
            self.health.resize(len, Health::default());
        }
    }
}

/// One proxy's failures since its last success, and when its rest ends.
#[derive(Debug, Default, Clone, Copy)]
struct Health {
    failures: u32,
    resting_until: Option<SystemTime>,
}

impl Health {
    fn resting(&self, now: SystemTime) -> bool {
        self.resting_until.is_some_and(|until| now < until)
    }
}

/// The lock, also after a thread panicked while holding it: the state is a
/// set of counters that stay usable.
fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// An ordered list of proxies, some of them named.
///
/// It is safe to share between threads: taking the next proxy needs only `&self`.
#[derive(Debug, Default)]
pub struct ProxyList {
    entries: Vec<ProxyEntry>,
    cursor: AtomicUsize,
    rotation: RotationStrategy,
    quarantine: Option<(u32, Duration)>,
    state: Mutex<Rotation>,
}

impl Clone for ProxyList {
    fn clone(&self) -> Self {
        Self {
            entries: self.entries.clone(),
            cursor: AtomicUsize::new(self.cursor.load(Ordering::Relaxed)),
            rotation: self.rotation,
            quarantine: self.quarantine,
            state: Mutex::new(locked(&self.state).clone()),
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

    /// Sets how [`pick`](Self::pick) chooses: in turn (the default), at random,
    /// the proxy with the fewest failures, or the same proxy for the same site.
    #[must_use]
    pub fn with_rotation(mut self, strategy: RotationStrategy) -> Self {
        self.rotation = strategy;
        locked(&self.state).draws = 0;
        self
    }

    /// Rests a proxy once it has failed `after_failures` times in a row (zero
    /// counts as one), for `rest`. A resting proxy is not picked, and its count
    /// starts again when the rest begins. A rest of more than a hundred years counts as a
    /// hundred years, which no run outlasts.
    #[must_use]
    pub fn with_quarantine(mut self, after_failures: u32, rest: Duration) -> Self {
        self.quarantine = Some((after_failures.max(1), rest.min(MAX_REST)));
        self
    }

    /// The proxy for a request to `site`, chosen by the
    /// [rotation strategy](Self::with_rotation) among the proxies that are not
    /// resting. `None` when the list is empty or every proxy is resting. The
    /// site matters only to [`RotationStrategy::StickyPerSite`].
    ///
    /// Whether a proxy is resting is read from the clock; [`pick_at`](Self::pick_at) takes the
    /// time instead.
    #[must_use]
    pub fn pick(&self, site: Option<&str>) -> Option<&ProxyEntry> {
        self.pick_at(site, SystemTime::now())
    }

    /// Like [`pick`](Self::pick), but at the time `now` instead of the time of the clock: a
    /// proxy is resting when the rest that [`report_at`](Self::report_at) (or
    /// [`report`](Self::report)) began has not ended by `now`. It is for a caller that keeps its
    /// own clock, such as a proxy pool that holds Unix seconds (`UNIX_EPOCH` plus a
    /// [`Duration`] makes the time), and for a test that moves time by hand.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::{Duration, UNIX_EPOCH};
    /// use seleniumbase_rs::config::proxy_list::{ProxyEntry, ProxyList, ProxyOutcome};
    ///
    /// let list = ProxyList::parse("a.test:1\nb.test:1\n")?
    ///     .with_quarantine(1, Duration::from_secs(600));
    /// let at = |secs| UNIX_EPOCH + Duration::from_secs(secs);
    ///
    /// // `a` fails at second 100, so it rests until second 700.
    /// let a = list.iter().next().expect("a proxy");
    /// list.report_at(a, ProxyOutcome::ConnectFailed, at(100));
    /// assert_eq!(list.pick_at(None, at(699)).map(ProxyEntry::server), Some("b.test:1"));
    /// assert_eq!(list.pick_at(None, at(700)).map(ProxyEntry::server), Some("a.test:1"));
    /// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
    /// ```
    #[must_use]
    pub fn pick_at(&self, site: Option<&str>, now: SystemTime) -> Option<&ProxyEntry> {
        self.choose(&|_| true, site, None, now)
    }

    /// Like [`pick`](Self::pick), among the proxies for which `accept` is true as well as not
    /// resting: a pick of one country, say, when the entries are named for their country.
    /// `None` when no proxy passes `accept` and is not resting. `accept` is called before the
    /// list takes its lock, so it may use the list. With
    /// [`RotationStrategy::StickyPerSite`] a site that stuck to a proxy `accept` refuses moves
    /// to one it allows, and sticks to that.
    #[must_use]
    pub fn pick_where(
        &self,
        accept: impl Fn(&ProxyEntry) -> bool,
        site: Option<&str>,
    ) -> Option<&ProxyEntry> {
        self.pick_where_at(accept, site, SystemTime::now())
    }

    /// Like [`pick_where`](Self::pick_where), at the time `now` (see
    /// [`pick_at`](Self::pick_at)).
    #[must_use]
    pub fn pick_where_at(
        &self,
        accept: impl Fn(&ProxyEntry) -> bool,
        site: Option<&str>,
        now: SystemTime,
    ) -> Option<&ProxyEntry> {
        self.choose(&accept, site, None, now)
    }

    /// Like [`pick`](Self::pick), but never `current`, so that changing proxy
    /// really moves. With [`RotationStrategy::StickyPerSite`] the site then
    /// sticks to the new proxy.
    #[must_use]
    pub fn pick_other(&self, current: &ProxyEntry, site: Option<&str>) -> Option<&ProxyEntry> {
        self.pick_other_at(current, site, SystemTime::now())
    }

    /// Like [`pick_other`](Self::pick_other), at the time `now` (see
    /// [`pick_at`](Self::pick_at)).
    #[must_use]
    pub fn pick_other_at(
        &self,
        current: &ProxyEntry,
        site: Option<&str>,
        now: SystemTime,
    ) -> Option<&ProxyEntry> {
        self.choose(&|_| true, site, self.index_of(current), now)
    }

    /// Records what happened to a request through `entry`. A success clears
    /// its failures; any other outcome adds one, and at the
    /// [quarantine](Self::with_quarantine) limit the proxy rests. An entry the list
    /// does not hold is ignored.
    pub fn report(&self, entry: &ProxyEntry, outcome: ProxyOutcome) {
        self.report_at(entry, outcome, SystemTime::now());
    }

    /// Like [`report`](Self::report), at the time `now` instead of the time of the clock: a
    /// proxy that comes to rest rests until `now` plus the rest (see
    /// [`pick_at`](Self::pick_at)).
    pub fn report_at(&self, entry: &ProxyEntry, outcome: ProxyOutcome, now: SystemTime) {
        let Some(index) = self.index_of(entry) else {
            return;
        };
        let mut state = locked(&self.state);
        state.fit(self.entries.len());
        let Some(health) = state.health.get_mut(index) else {
            return;
        };
        if outcome == ProxyOutcome::Success {
            health.failures = 0;
            return;
        }
        health.failures = health.failures.saturating_add(1);
        if let Some((limit, rest)) = self.quarantine
            && health.failures >= limit
        {
            health.resting_until = Some(now.checked_add(rest).unwrap_or(now));
            health.failures = 0;
        }
    }

    /// The pick of every strategy: among the proxies that are not resting and
    /// are not `except`.
    fn choose(
        &self,
        accept: &dyn Fn(&ProxyEntry) -> bool,
        site: Option<&str>,
        except: Option<usize>,
        now: SystemTime,
    ) -> Option<&ProxyEntry> {
        // `accept` runs before the lock is taken, so that it may use this list.
        let passing: Vec<usize> = self
            .entries
            .iter()
            .enumerate()
            .filter(|&(index, entry)| Some(index) != except && accept(entry))
            .map(|(index, _)| index)
            .collect();
        let mut state = locked(&self.state);
        state.fit(self.entries.len());
        let open: Vec<usize> = passing
            .into_iter()
            .filter(|&index| !state.health[index].resting(now))
            .collect();
        if open.is_empty() {
            return None;
        }
        let index = match self.rotation {
            RotationStrategy::RoundRobin => self.next_of(&open),
            RotationStrategy::Random { seed } => {
                open[Self::random_index(&mut state, seed, open.len())]
            }
            RotationStrategy::LeastFailures => self.least_failed(&state, &open),
            RotationStrategy::StickyPerSite => match site {
                Some(site) => self.sticky(&mut state, site, &open),
                None => self.next_of(&open),
            },
        };
        self.entries.get(index)
    }

    /// A random position below `len`. With a seed, each draw has its own
    /// generator, seeded from the seed and the draw number: the order repeats
    /// for the seed, and the list stays cloneable.
    fn random_index(state: &mut Rotation, seed: Option<u64>, len: usize) -> usize {
        match seed {
            Some(seed) => {
                let mut rng = StdRng::seed_from_u64(seed.wrapping_add(state.draws));
                state.draws = state.draws.wrapping_add(1);
                rng.random_range(0..len)
            }
            None => rand::rng().random_range(0..len),
        }
    }

    /// The next in turn of the `open` proxies with the fewest failures.
    fn least_failed(&self, state: &Rotation, open: &[usize]) -> usize {
        let failures = |index: usize| state.health[index].failures;
        let fewest = open
            .iter()
            .map(|&index| failures(index))
            .min()
            .unwrap_or_default();
        let tied: Vec<usize> = open
            .iter()
            .copied()
            .filter(|&index| failures(index) == fewest)
            .collect();
        self.next_of(&tied)
    }

    /// The proxy `site` stuck to when it is still open, otherwise the next in
    /// turn, which the site then sticks to.
    fn sticky(&self, state: &mut Rotation, site: &str, open: &[usize]) -> usize {
        if let Some(&kept) = state.sticky.get(site)
            && open.contains(&kept)
        {
            return kept;
        }
        let chosen = self.next_of(open);
        if state.sticky.len() >= MAX_STICKY_SITES {
            state.sticky.clear();
        }
        state.sticky.insert(site.to_owned(), chosen);
        chosen
    }

    /// The next of `candidates` (positions in the list, ascending, never empty) in turn: the
    /// first at or after where the shared turn stands, or the first of all when none is, and
    /// the turn then stands just after it. So a proxy that rests leaves a gap in the cycle and
    /// no other proxy is picked twice because of it.
    fn next_of(&self, candidates: &[usize]) -> usize {
        let length = self.entries.len().max(1);
        let mut chosen = 0;
        let _ = self
            .cursor
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |turn| {
                let start = turn % length;
                chosen = candidates
                    .iter()
                    .copied()
                    .find(|&position| position >= start)
                    .or_else(|| candidates.first().copied())
                    .unwrap_or_default();
                Some(chosen + 1)
            });
        chosen
    }

    /// Where `entry` is in the list: the entry itself, else one with the same
    /// proxy.
    fn index_of(&self, entry: &ProxyEntry) -> Option<usize> {
        self.entries
            .iter()
            .position(|held| std::ptr::eq(held, entry))
            .or_else(|| {
                self.entries
                    .iter()
                    .position(|held| held.spec() == entry.spec())
            })
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
    use std::time::UNIX_EPOCH;

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
    fn round_robin_is_the_pick_by_default() {
        let list = list(&["a.test:1", "b.test:2", "c.test:3"]);
        let order: Vec<_> = (0..4).map(|_| list.pick(None).unwrap().server()).collect();
        assert_eq!(order, ["a.test:1", "b.test:2", "c.test:3", "a.test:1"]);
    }

    #[test]
    fn a_seeded_random_pick_repeats_its_order_for_the_seed() {
        let specs = ["a.test:1", "b.test:2", "c.test:3", "d.test:4"];
        let order = |seed| {
            let list = list(&specs).with_rotation(RotationStrategy::Random { seed: Some(seed) });
            (0..12)
                .map(|_| list.pick(None).unwrap().server().to_owned())
                .collect::<Vec<_>>()
        };
        assert_eq!(order(7), order(7));
        assert_ne!(order(7), order(8), "another seed, another order");
        assert!(order(7)
            .iter()
            .all(|server| specs.contains(&server.as_str())));
    }

    #[test]
    fn least_failures_keeps_away_from_a_proxy_that_failed() {
        let list = list(&["a.test:1", "b.test:2"]).with_rotation(RotationStrategy::LeastFailures);
        let a = list.iter().next().unwrap().clone();
        list.report(&a, ProxyOutcome::ConnectFailed);
        for _ in 0..4 {
            assert_eq!(list.pick(None).unwrap().server(), "b.test:2");
        }
        list.report(&a, ProxyOutcome::Success);
        let both: std::collections::HashSet<_> =
            (0..4).map(|_| list.pick(None).unwrap().server()).collect();
        assert_eq!(both.len(), 2, "a success puts it back in turn");
    }

    #[test]
    fn a_site_sticks_to_its_proxy_and_moves_with_pick_other() {
        let list = list(&["a.test:1", "b.test:2", "c.test:3"])
            .with_rotation(RotationStrategy::StickyPerSite);
        let first = list.pick(Some("shop.test")).unwrap().clone();
        assert_eq!(
            list.pick(Some("shop.test")).unwrap().server(),
            first.server()
        );
        assert_ne!(
            list.pick(Some("news.test")).unwrap().server(),
            first.server()
        );
        let moved = list.pick_other(&first, Some("shop.test")).unwrap().clone();
        assert_ne!(moved.server(), first.server());
        assert_eq!(
            list.pick(Some("shop.test")).unwrap().server(),
            moved.server()
        );
    }

    #[test]
    fn a_proxy_rests_after_its_failures_and_a_short_rest_ends() {
        let list = list(&["a.test:1", "b.test:2"]).with_quarantine(2, Duration::from_secs(600));
        let a = list.iter().next().unwrap().clone();
        list.report(&a, ProxyOutcome::Blocked);
        assert!(
            (0..4).any(|_| list.pick(None).unwrap().server() == a.server()),
            "one failure is not enough"
        );
        list.report(&a, ProxyOutcome::Challenged);
        assert!((0..6).all(|_| list.pick(None).unwrap().server() == "b.test:2"));
        let b = list.iter().nth(1).unwrap().clone();
        list.report(&b, ProxyOutcome::ConnectFailed);
        list.report(&b, ProxyOutcome::ConnectFailed);
        assert!(list.pick(None).is_none(), "every proxy is resting");

        let quick = self::list(&["a.test:1"]).with_quarantine(1, Duration::ZERO);
        let only = quick.iter().next().unwrap().clone();
        quick.report(&only, ProxyOutcome::ConnectFailed);
        assert_eq!(
            quick.pick(None).map(ProxyEntry::server),
            Some(only.server()),
            "a rest of zero is already over"
        );
    }

    #[test]
    fn pick_other_never_returns_the_current_proxy() {
        let list = list(&["a.test:1", "b.test:2"]);
        let a = list.iter().next().unwrap().clone();
        assert!((0..5).all(|_| list.pick_other(&a, None).unwrap().server() == "b.test:2"));
        let alone = self::list(&["a.test:1"]);
        let only = alone.iter().next().unwrap().clone();
        assert!(alone.pick_other(&only, None).is_none());
        assert!(ProxyList::new().pick(None).is_none());
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

    #[test]
    fn round_robin_holds_its_order_around_a_proxy_that_starts_to_rest_mid_cycle() {
        let list = list(&["a.test:1", "b.test:2", "c.test:3"])
            .with_quarantine(1, Duration::from_secs(600));
        let a = list.iter().next().unwrap().clone();
        assert_eq!(list.pick(None).unwrap().server(), "a.test:1");
        assert_eq!(list.pick(None).unwrap().server(), "b.test:2");
        list.report(&a, ProxyOutcome::Blocked);
        let order: Vec<_> = (0..4).map(|_| list.pick(None).unwrap().server()).collect();
        assert_eq!(
            order,
            ["c.test:3", "b.test:2", "c.test:3", "b.test:2"],
            "c is next after b; the resting a is skipped, and b is not picked twice in a row"
        );
    }

    // The forms that take the time (`pick_at`, `pick_other_at`, `pick_where_at`, `report_at`):
    // a caller that keeps its own clock, such as the stealth layer's proxy pool, drives them with
    // a time of its own, and so do these tests.

    /// The moment `secs` seconds after the Unix epoch.
    fn at(secs: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(secs)
    }

    const P1: &str = "p1.test:8080";
    const P2: &str = "p2.test:8080";
    const P3: &str = "p3.test:8080";

    fn three() -> ProxyList {
        list(&[P1, P2, P3])
    }

    /// A copy of the entry of `list` that is served by `server`.
    fn entry_of(list: &ProxyList, server: &str) -> ProxyEntry {
        list.iter()
            .find(|entry| entry.server() == server)
            .expect("a proxy of the list")
            .clone()
    }

    /// The servers of `count` picks in a row, all made at the time `now` (in seconds).
    fn servers_at(list: &ProxyList, count: usize, now: u64) -> Vec<String> {
        (0..count)
            .map(|_| {
                list.pick_at(None, at(now))
                    .map(|entry| entry.server().to_owned())
                    .unwrap_or_default()
            })
            .collect()
    }

    #[test]
    fn a_list_can_be_shared_between_threads() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ProxyList>();
    }

    #[test]
    fn nothing_is_picked_from_an_empty_list_at_any_time() {
        let list = ProxyList::new();
        let stray = entry_of(&three(), P1);
        assert!(list.pick_at(None, at(0)).is_none());
        assert!(list.pick_where_at(|_| true, None, at(0)).is_none());
        assert!(list.pick_other_at(&stray, None, at(0)).is_none());
    }

    #[test]
    fn round_robin_at_a_time_skips_a_resting_proxy_without_repeating_another() {
        let list = three().with_quarantine(1, Duration::from_secs(600));
        list.report_at(&entry_of(&list, P2), ProxyOutcome::ConnectFailed, at(0));
        assert_eq!(
            servers_at(&list, 6, 1),
            [P1, P3].repeat(3),
            "the cycle holds its order around the gap"
        );
    }

    #[test]
    fn a_proxy_that_has_rested_long_enough_rejoins_the_cycle() {
        let list = three().with_quarantine(1, Duration::from_secs(600));
        list.report_at(&entry_of(&list, P2), ProxyOutcome::ConnectFailed, at(0));
        let _ = servers_at(&list, 2, 1);
        let mut after = servers_at(&list, 3, 600);
        after.sort();
        assert_eq!(after, [P1, P2, P3]);
    }

    #[test]
    fn failures_rest_a_proxy_until_its_time_is_up_to_the_second() {
        let list = list(&[P1, P2]).with_quarantine(3, Duration::from_secs(600));
        let p1 = entry_of(&list, P1);
        for _ in 0..3 {
            list.report_at(&p1, ProxyOutcome::ConnectFailed, at(100));
        }
        assert_eq!(servers_at(&list, 4, 699), [P2; 4]);
        let released = servers_at(&list, 2, 700);
        assert!(released.contains(&P1.to_owned()), "{released:?}");
    }

    #[test]
    fn a_released_proxy_starts_again_with_no_failures() {
        let list = list(&[P1]).with_quarantine(2, Duration::from_secs(10));
        let p1 = entry_of(&list, P1);
        for _ in 0..2 {
            list.report_at(&p1, ProxyOutcome::ConnectFailed, at(0));
        }
        assert!(list.pick_at(None, at(9)).is_none());
        list.report_at(&p1, ProxyOutcome::ConnectFailed, at(10));
        assert!(
            list.pick_at(None, at(11)).is_some(),
            "one failure after the rest is below the limit of two"
        );
    }

    #[test]
    fn a_limit_of_zero_means_the_first_failure() {
        let list = list(&[P1]).with_quarantine(0, Duration::from_secs(60));
        assert!(list.pick_at(None, at(0)).is_some());
        list.report_at(&entry_of(&list, P1), ProxyOutcome::ConnectFailed, at(0));
        assert!(list.pick_at(None, at(1)).is_none());
    }

    #[test]
    fn every_failing_outcome_counts_and_a_success_clears_the_count() {
        for failing in [
            ProxyOutcome::ConnectFailed,
            ProxyOutcome::Challenged,
            ProxyOutcome::Blocked,
        ] {
            let list = list(&[P1]).with_quarantine(3, Duration::from_secs(60));
            let p1 = entry_of(&list, P1);
            list.report_at(&p1, failing, at(0));
            list.report_at(&p1, failing, at(0));
            list.report_at(&p1, ProxyOutcome::Success, at(0));
            list.report_at(&p1, failing, at(0));
            list.report_at(&p1, failing, at(0));
            assert!(
                list.pick_at(None, at(0)).is_some(),
                "two failures after a success must not rest the proxy ({failing:?})"
            );
            list.report_at(&p1, failing, at(0));
            assert!(list.pick_at(None, at(0)).is_none(), "the third one does");
        }
    }

    #[test]
    fn a_report_for_a_proxy_the_list_does_not_hold_changes_nothing() {
        let list = three().with_quarantine(1, Duration::from_secs(600));
        let stray = entry_of(&self::list(&["elsewhere.test:1"]), "elsewhere.test:1");
        list.report_at(&stray, ProxyOutcome::Blocked, at(0));
        assert_eq!(servers_at(&list, 3, 0), [P1, P2, P3]);
    }

    #[test]
    fn a_rest_of_any_length_neither_panics_nor_ends() {
        let list = three().with_quarantine(1, Duration::MAX);
        list.report_at(&entry_of(&list, P1), ProxyOutcome::Blocked, at(0));
        assert!(servers_at(&list, 4, 3_000_000_000)
            .iter()
            .all(|server| server != P1));
    }

    #[test]
    fn least_failures_follows_the_healthiest_proxy() {
        let list = three().with_rotation(RotationStrategy::LeastFailures);
        let (p1, p2, p3) = (
            entry_of(&list, P1),
            entry_of(&list, P2),
            entry_of(&list, P3),
        );
        list.report_at(&p1, ProxyOutcome::ConnectFailed, at(0));
        list.report_at(&p1, ProxyOutcome::ConnectFailed, at(0));
        list.report_at(&p2, ProxyOutcome::ConnectFailed, at(0));
        assert_eq!(servers_at(&list, 3, 0), [P3; 3]);
        list.report_at(&p3, ProxyOutcome::ConnectFailed, at(0));
        assert_eq!(servers_at(&list, 4, 0), [P2, P3, P2, P3]);
    }

    #[test]
    fn least_failures_rotates_among_equals() {
        let list = three().with_rotation(RotationStrategy::LeastFailures);
        assert_eq!(servers_at(&list, 6, 0), [P1, P2, P3].repeat(2));
    }

    #[test]
    fn sticky_sites_are_independent_and_new_ones_move_on() {
        let list = three().with_rotation(RotationStrategy::StickyPerSite);
        let pick = |site: &str| {
            list.pick_at(Some(site), at(0))
                .map(|entry| entry.server().to_owned())
                .unwrap_or_default()
        };
        let (a, b, c) = (pick("a.test"), pick("b.test"), pick("c.test"));
        assert_eq!([a.as_str(), b.as_str(), c.as_str()], [P1, P2, P3]);
        assert_eq!(pick("a.test"), a);
        assert_eq!(pick("b.test"), b);
    }

    #[test]
    fn sticky_without_a_site_rotates_like_round_robin() {
        let list = three().with_rotation(RotationStrategy::StickyPerSite);
        assert_eq!(servers_at(&list, 3, 0), [P1, P2, P3]);
    }

    #[test]
    fn a_sticky_proxy_that_starts_to_rest_gives_way() {
        let list = three()
            .with_rotation(RotationStrategy::StickyPerSite)
            .with_quarantine(1, Duration::from_secs(600));
        let first = list.pick_at(Some("a.test"), at(0)).unwrap().clone();
        list.report_at(&first, ProxyOutcome::Blocked, at(0));
        let next = list
            .pick_at(Some("a.test"), at(1))
            .unwrap()
            .server()
            .to_owned();
        assert_ne!(next, first.server());
        assert_eq!(
            list.pick_at(Some("a.test"), at(1)).unwrap().server(),
            next,
            "the site now sticks to the replacement"
        );
    }

    #[test]
    fn the_sticky_table_stays_bounded() {
        let list = three().with_rotation(RotationStrategy::StickyPerSite);
        for index in 0..MAX_STICKY_SITES + 25 {
            let site = format!("site{index}.test");
            assert!(list.pick_at(Some(&site), at(0)).is_some());
        }
        let remembered = locked(&list.state).sticky.len();
        assert!(
            (1..=MAX_STICKY_SITES).contains(&remembered),
            "{remembered} sites remembered"
        );
    }

    #[test]
    fn pick_other_at_never_returns_the_current_proxy_and_moves_a_sticky_site() {
        let list = three();
        let p2 = entry_of(&list, P2);
        assert!((0..6).all(|_| list.pick_other_at(&p2, None, at(0)).unwrap().server() != P2));

        let sticky = three().with_rotation(RotationStrategy::StickyPerSite);
        let first = sticky.pick_at(Some("a.test"), at(0)).unwrap().clone();
        let moved = sticky
            .pick_other_at(&first, Some("a.test"), at(0))
            .unwrap()
            .clone();
        assert_ne!(moved.server(), first.server());
        assert_eq!(
            sticky.pick_at(Some("a.test"), at(0)).unwrap().server(),
            moved.server()
        );
    }

    #[test]
    fn pick_other_at_with_nothing_else_left_is_none() {
        let alone = list(&[P1]);
        assert!(alone
            .pick_other_at(&entry_of(&alone, P1), None, at(0))
            .is_none());
    }

    #[test]
    fn a_seeded_random_pick_at_a_time_gives_one_order_for_one_seed() {
        let order = |seed| {
            let list = three().with_rotation(RotationStrategy::Random { seed: Some(seed) });
            servers_at(&list, 10, 0)
        };
        assert_eq!(order(11), order(11));
    }

    #[test]
    fn random_uses_every_proxy_and_never_a_resting_one() {
        let list = three().with_rotation(RotationStrategy::Random { seed: Some(3) });
        let seen = servers_at(&list, 40, 0);
        for server in [P1, P2, P3] {
            assert!(
                seen.iter().any(|seen| seen == server),
                "{server} never drawn"
            );
        }
        let list = three()
            .with_rotation(RotationStrategy::Random { seed: Some(3) })
            .with_quarantine(1, Duration::from_secs(600));
        list.report_at(&entry_of(&list, P2), ProxyOutcome::Blocked, at(0));
        assert!(!servers_at(&list, 60, 1).contains(&P2.to_owned()));
    }

    /// Three proxies named for a country, the French one first.
    fn by_country() -> ProxyList {
        ProxyList::parse("fr = fr.test:1\nde1 = de1.test:1\nde2 = de2.test:1\n")
            .expect("a valid list")
    }

    fn german(entry: &ProxyEntry) -> bool {
        entry.name().is_some_and(|name| name.starts_with("de"))
    }

    #[test]
    fn pick_where_at_only_returns_proxies_that_pass_and_takes_turns_among_them() {
        let list = by_country();
        let order: Vec<_> = (0..4)
            .map(|_| list.pick_where_at(german, None, at(0)).unwrap().server())
            .collect();
        assert_eq!(
            order,
            ["de1.test:1", "de2.test:1", "de1.test:1", "de2.test:1"]
        );
        assert!(list.pick_where_at(|_| false, None, at(0)).is_none());
    }

    #[test]
    fn pick_where_at_finds_nothing_when_every_proxy_that_passes_rests() {
        let list = by_country().with_quarantine(1, Duration::from_secs(600));
        for name in ["de1", "de2"] {
            let entry = list.get(name).unwrap().clone();
            list.report_at(&entry, ProxyOutcome::Blocked, at(0));
        }
        assert!(list.pick_where_at(german, None, at(1)).is_none());
        assert_eq!(
            list.pick_at(None, at(1)).map(ProxyEntry::server),
            Some("fr.test:1"),
            "the other country still serves"
        );
    }

    #[test]
    fn a_sticky_site_moves_to_a_proxy_that_passes_and_sticks_to_it() {
        let list = by_country().with_rotation(RotationStrategy::StickyPerSite);
        assert_eq!(
            list.pick_at(Some("shop.test"), at(0)).unwrap().server(),
            "fr.test:1"
        );
        let moved = list
            .pick_where_at(german, Some("shop.test"), at(0))
            .unwrap()
            .clone();
        assert!(german(&moved));
        assert_eq!(
            list.pick_at(Some("shop.test"), at(0)).unwrap().server(),
            moved.server()
        );
    }

    #[test]
    fn the_clock_forms_and_the_timed_forms_share_what_a_report_recorded() {
        let list = three().with_quarantine(1, Duration::from_secs(600));
        let now = SystemTime::now();
        list.report(&entry_of(&list, P1), ProxyOutcome::Blocked);
        assert!(
            (0..6).all(|_| list
                .pick_at(None, now + Duration::from_secs(590))
                .unwrap()
                .server()
                != P1),
            "a rest recorded by the clock is still on at the time it was set to last"
        );
        assert!(
            (0..6).any(|_| list
                .pick_at(None, now + Duration::from_secs(700))
                .unwrap()
                .server()
                == P1),
            "and over after it"
        );

        let other = three().with_quarantine(1, Duration::from_secs(600));
        other.report_at(
            &entry_of(&other, P1),
            ProxyOutcome::Blocked,
            SystemTime::now(),
        );
        assert!((0..6).all(|_| other.pick(None).unwrap().server() != P1));
    }
}
