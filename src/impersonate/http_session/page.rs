//! The page leg of a session: what a Chrome page tells the session about itself, and the cookies
//! the session writes back.
//!
//! [`HttpSession::from_page`] reads three things from the page: its `navigator.userAgent` and
//! `navigator.languages`, the cookies of its browser profile, and the proxy of its browser
//! context. The measured TLS and HTTP/2 belong to one Chrome major version on one system; a page
//! whose user agent or browser is another major version is refused with
//! [`HttpError::VersionMismatch`], and a system that was not measured is refused with
//! [`NotMeasured`](crate::wire::emulation::EmulationError::NotMeasured).
//!
//! A browser-wide proxy (`--proxy-server`) is not visible to a page, but the command line of the
//! browser process is: `SystemInfo.getInfo` reports it. A session built from a page of such a
//! browser must not go direct, because the cookies the page earned through the proxy would then
//! leave from this machine's own address; it is refused with [`HttpError::BrowserProxy`] unless it
//! is given the egress the proxy really leaves through, or told that the proxy leaves directly.

use std::fmt::Write as _;
use std::net::IpAddr;

use serde_json::{json, Value};

use super::cookies::Pending;
use super::error::HttpError;
use super::options::HttpOptions;
use super::HttpSession;
use crate::error::SeleniumBaseError;
use crate::impersonate::client::{ImpersonateError, ImpersonationProfile};
use crate::sb_cdp::{Page, Proxy};
use crate::scrape::egress::route::{EgressKey, EgressRef};
use crate::scrape::jar::store::CookieJar;
use crate::scrape::passport::identity::IdentityError;
use crate::stealth::fingerprint::OsType;
use crate::wire::emulation;
use crate::wire::BrowserKind;

/// What a handoff reads from the page in one round trip.
const PROBE_JS: &str = "({ userAgent: navigator.userAgent, \
     languages: Array.from(navigator.languages || [navigator.language]).filter(Boolean) })";
/// User agent tokens of browsers that are not Chrome, or whose TLS stack is not Chrome's, though
/// their user agent also says `Chrome/`.
const NOT_CHROME: [&str; 8] = [
    "Edg/",
    "EdgA/",
    "EdgiOS/",
    "OPR/",
    "CriOS/",
    "FxiOS/",
    "Firefox/",
    "SamsungBrowser/",
];
/// The token that carries the Chrome major version in a user agent.
const CHROME_TOKEN: &str = "Chrome/";
/// The token a headless Chrome puts in its user agent in place of `Chrome`.
const HEADLESS_TOKEN: &str = "HeadlessChrome";
/// How many languages `Accept-Language` lists, as Chrome writes it.
const MAX_LANGUAGES: usize = 10;

/// What the page reported about itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Probe {
    pub(super) user_agent: String,
    pub(super) languages: Vec<String>,
    /// The major version of the browser itself, from `Browser.getVersion`, when it answered.
    pub(super) product_major: Option<u32>,
}

/// The Chrome major version a user agent names.
pub(super) fn chrome_major(user_agent: &str) -> Option<u32> {
    let (_, rest) = user_agent.split_once(CHROME_TOKEN)?;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// The operating system a user agent names.
pub(super) fn os_of(user_agent: &str) -> Result<OsType, ImpersonateError> {
    let named = |tokens: &[&str]| tokens.iter().any(|token| user_agent.contains(token));
    if named(&["iPhone", "iPad", "iPod"]) {
        Ok(OsType::Ios)
    } else if named(&["Android"]) {
        Ok(OsType::Android)
    } else if named(&["Windows NT"]) {
        Ok(OsType::Windows)
    } else if named(&["Macintosh"]) {
        Ok(OsType::Macos)
    } else if named(&["Linux", "X11", "CrOS"]) {
        Ok(OsType::Linux)
    } else {
        Err(ImpersonateError::Fingerprint(
            "the operating system cannot be told from its user agent",
        ))
    }
}

/// An `Accept-Language` value built the way Chrome builds it from its language list: the first
/// language plain, then `q=0.9`, `q=0.8`, and so on down to `q=0.1`.
pub(super) fn accept_language(languages: &[String]) -> Option<String> {
    let mut seen: Vec<&str> = Vec::new();
    for language in languages {
        let language = language.trim();
        if !language.is_empty() && !seen.contains(&language) {
            seen.push(language);
        }
    }
    let mut header = String::new();
    for (index, language) in seen.iter().take(MAX_LANGUAGES).enumerate() {
        if index > 0 {
            header.push(',');
        }
        header.push_str(language);
        if index > 0 {
            // At most nine entries follow the first, so this is 0.9 down to 0.1.
            let _written = write!(header, ";q=0.{}", 10 - index.min(9));
        }
    }
    (!header.is_empty()).then_some(header)
}

/// The profile for what a page reported.
///
/// # Errors
///
/// Returns [`HttpError::VersionMismatch`] when the user agent or the browser is another Chrome
/// major than the measured one, and [`HttpError::Impersonate`] for a browser that is not Chrome,
/// one that announces itself as headless (its user agent would give the session away), or a
/// system that was not measured.
pub(super) fn profile_of(probe: &Probe) -> Result<ImpersonationProfile, HttpError> {
    let user_agent = probe.user_agent.as_str();
    if NOT_CHROME.iter().any(|token| user_agent.contains(token)) {
        return Err(ImpersonateError::Fingerprint("the browser is not Chrome").into());
    }
    if user_agent.contains(HEADLESS_TOKEN) {
        return Err(ImpersonateError::Fingerprint(
            "the browser announces itself as headless Chrome; give it a fingerprint or run it headed",
        )
        .into());
    }
    let claimed = chrome_major(user_agent).ok_or(ImpersonateError::Fingerprint(
        "its user agent names no Chrome version",
    ))?;
    let os = os_of(user_agent)?;
    let measured = emulation::get(BrowserKind::Chrome, os)
        .map_err(ImpersonateError::from)?
        .major;
    for browser in std::iter::once(claimed).chain(probe.product_major) {
        if browser != measured {
            return Err(HttpError::VersionMismatch { browser, measured });
        }
    }
    let mut builder = ImpersonationProfile::custom()
        .base_on(BrowserKind::Chrome, os)
        .user_agent(user_agent);
    if let Some(language) = accept_language(&probe.languages) {
        builder = builder.accept_language(language);
    }
    Ok(builder.build()?)
}

/// Chrome's bypass patterns as a `no_proxy` list. `<-loopback>` and `<local>` are Chrome keywords
/// with no equivalent here and are left out.
pub(super) fn bypass_hosts(patterns: &[String]) -> Vec<String> {
    patterns
        .iter()
        .map(|pattern| pattern.trim())
        .filter(|pattern| !pattern.is_empty() && !pattern.starts_with('<'))
        .map(|pattern| match pattern.strip_prefix('*') {
            Some(suffix) => suffix.to_owned(),
            None => pattern.to_owned(),
        })
        .collect()
}

/// An egress for the proxy of a browser context.
fn egress_of(proxy: &Proxy) -> Result<EgressRef, HttpError> {
    let server = proxy.server();
    let spec = match (proxy.credentials(), server.split_once("://")) {
        (Some((user, password)), Some((scheme, rest))) => {
            format!("{scheme}://{user}:{password}@{rest}")
        }
        (Some((user, password)), None) => format!("{user}:{password}@{server}"),
        (None, _) => server.to_owned(),
    };
    EgressRef::parse(&spec, None).map_err(|_error| HttpError::Client {
        kind: "the proxy of the page cannot be read",
    })
}

/// Whether two keys name the same exit: the scheme (`socks5` and `socks5h` alike), host and port.
fn same_exit(left: &EgressKey, right: &EgressKey) -> bool {
    let family = |scheme: &str| {
        if scheme.starts_with("socks5") {
            "socks5"
        } else {
            "http"
        }
        .to_owned()
    };
    family(&left.scheme) == family(&right.scheme)
        && left.host == right.host
        && left.port == right.port
}

/// The egress a session built from a page leaves through.
///
/// A page whose browser context has a proxy of its own leaves through it, so the session must
/// too: with no egress given it takes that one, and with another given it is refused.
///
/// # Errors
///
/// Returns [`IdentityError::EgressMismatch`] (as [`HttpError::Identity`]) when the egress given is
/// not the proxy of the page's context.
pub(super) fn reconcile(
    given: Option<EgressRef>,
    context_proxy: Option<&Proxy>,
) -> Result<Option<EgressRef>, HttpError> {
    let Some(proxy) = context_proxy else {
        return Ok(given);
    };
    let routed = egress_of(proxy)?;
    match given {
        None => Ok(Some(routed)),
        Some(given) if same_exit(given.key(), routed.key()) => Ok(Some(given)),
        Some(given) => Err(IdentityError::EgressMismatch {
            expected: Some(routed.key().clone()),
            found: Some(given.key().clone()),
        }
        .into()),
    }
}

/// The switch that gives a browser its browser-wide proxy.
const PROXY_SERVER_SWITCH: &str = "--proxy-server=";
/// The switch that turns every proxy off, whatever other switches say.
const NO_PROXY_SWITCH: &str = "--no-proxy-server";
/// The value of `--proxy-server` that means no proxy.
const DIRECT: &str = "direct://";

/// The browser-wide proxy a browser was started with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct BrowserProxy {
    /// The proxy as the command line names it. Chrome takes no credentials there, so it holds
    /// none.
    pub(super) server: String,
    /// Whether the proxy runs on this machine. Then it forwards somewhere else (a MITM proxy, a
    /// local forwarder), and where is not visible.
    pub(super) local: bool,
}

impl BrowserProxy {
    /// The exit the proxy names, when its address can be read.
    fn exit(&self) -> Option<EgressKey> {
        let spec = if self.server.contains("://") {
            self.server.clone()
        } else {
            format!("http://{}", self.server)
        };
        EgressRef::parse(&spec, None)
            .ok()
            .map(|egress| egress.key().clone())
    }
}

/// Whether the host of a proxy address (`[scheme://]host[:port]`) is this machine.
fn names_this_machine(address: &str) -> bool {
    let authority = address.split_once("://").map_or(address, |(_, rest)| rest);
    let authority = authority.split('/').next().unwrap_or_default();
    let host = match authority.strip_prefix('[') {
        Some(inner) => inner.split(']').next().unwrap_or_default(),
        None => authority
            .rsplit_once(':')
            .map_or(authority, |(host, _port)| host),
    };
    host.eq_ignore_ascii_case("localhost")
        || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

/// The browser-wide proxy in the command line of a browser process, `None` for a browser that
/// goes direct.
///
/// The last `--proxy-server` counts, as it does for Chrome, `--no-proxy-server` overrides it,
/// and `direct://` is no proxy. Of a list such as `http=a:1;https=b:2` the first entry is read.
pub(super) fn browser_proxy_in(command_line: &str) -> Option<BrowserProxy> {
    let words: Vec<&str> = command_line.split_whitespace().collect();
    if words.contains(&NO_PROXY_SWITCH) {
        return None;
    }
    let value = words
        .iter()
        .rev()
        .find_map(|word| word.strip_prefix(PROXY_SERVER_SWITCH))?
        .trim_matches(['"', '\'']);
    let entry = value.split(';').next().unwrap_or_default();
    let entry = match entry.split_once('=') {
        Some((scheme, address)) if !scheme.contains([':', '/']) => address,
        _ => entry,
    };
    if entry.is_empty() || entry.eq_ignore_ascii_case(DIRECT) {
        return None;
    }
    Some(BrowserProxy {
        server: entry.to_owned(),
        local: names_this_machine(entry),
    })
}

/// The egress a session built from a page of a browser with a browser-wide proxy leaves through.
///
/// The proxy does not show on a page, so a session with no egress would go direct, from this
/// machine's own address, with the cookies the page earned through the proxy. Unless it is told
/// that the proxy leaves directly, that is refused. An egress given for a proxy on another
/// machine must be that proxy; for a proxy on this machine (a MITM) it is the exit behind it,
/// which is taken as given.
///
/// # Errors
///
/// Returns [`HttpError::BrowserProxy`] when the browser has a proxy and no egress was given, and
/// [`IdentityError::EgressMismatch`] (as [`HttpError::Identity`]) when the egress given is not a
/// remote browser proxy.
pub(super) fn reconcile_browser_wide(
    given: Option<EgressRef>,
    browser_proxy: Option<&BrowserProxy>,
    exits_directly: bool,
) -> Result<Option<EgressRef>, HttpError> {
    let Some(wide) = browser_proxy else {
        return Ok(given);
    };
    match given {
        Some(given) => {
            let own = wide.exit().filter(|_| !wide.local);
            match own {
                Some(own) if !same_exit(given.key(), &own) => Err(IdentityError::EgressMismatch {
                    expected: Some(own),
                    found: Some(given.key().clone()),
                }
                .into()),
                _ => Ok(Some(given)),
            }
        }
        None if exits_directly => Ok(None),
        None => Err(HttpError::BrowserProxy {
            proxy: wide.server.clone(),
        }),
    }
}

/// The browser-wide proxy the page's browser was started with, read from the command line of its
/// process. `None` when it has none or will not say.
async fn browser_proxy(page: &Page) -> Option<BrowserProxy> {
    let info = page
        .browser()
        .execute("SystemInfo.getInfo", json!({}))
        .await
        .ok()?;
    browser_proxy_in(info["commandLine"].as_str()?)
}

/// Reads the user agent, the languages and the browser version from the page.
async fn probe(page: &Page) -> Result<Probe, HttpError> {
    let value = page.evaluate(PROBE_JS).await?;
    let user_agent = value["userAgent"]
        .as_str()
        .ok_or_else(|| SeleniumBaseError::cdp_driver("the page reported no navigator.userAgent"))?
        .to_owned();
    let languages = value["languages"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|language| language.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    let version = page
        .browser()
        .execute("Browser.getVersion", json!({}))
        .await
        .ok();
    let product_major = version
        .as_ref()
        .and_then(|reply| reply["product"].as_str())
        .and_then(|product| product.split_once('/'))
        .and_then(|(_, number)| number.split('.').next())
        .and_then(|major| major.parse().ok());
    Ok(Probe {
        user_agent,
        languages,
        product_major,
    })
}

/// The proxy of the page's browser context, when the context was made with one.
async fn context_proxy(page: &Page) -> Result<(Option<Proxy>, Vec<String>), HttpError> {
    let info = page
        .browser()
        .execute("Target.getTargetInfo", json!({ "targetId": page.id() }))
        .await?;
    let route = info["targetInfo"]["browserContextId"]
        .as_str()
        .and_then(|id| page.browser().context_route(id));
    Ok(match route {
        Some(route) => (Some(route.proxy), route.bypass),
        None => (None, Vec::new()),
    })
}

impl HttpSession {
    /// Continues the session of `page` as plain HTTP.
    ///
    /// Copies the cookies of the page's browser profile into the jar, reads the page's
    /// `navigator.userAgent` and `navigator.languages`, and follows the route its traffic takes:
    /// the proxy of its browser context. `egress` is the exit the page leaves through; leave it
    /// `None` to take the context's own proxy or go direct.
    ///
    /// A browser-wide proxy, such as a MITM proxy the browser was launched with, is not visible to
    /// a page, but it is on the command line of the browser, which is read. A browser that has
    /// one is not taken for a direct one: pass the `egress` it really leaves through (for a proxy
    /// on another machine, that proxy), or say with
    /// [`HttpOptions::browser_proxy_exits_directly`] that it leaves directly. A browser that will
    /// not report its command line is taken as it was before, as a direct one.
    ///
    /// # Errors
    ///
    /// Returns [`HttpError::VersionMismatch`] when the page's Chrome is not the measured major,
    /// [`HttpError::Impersonate`] for another browser or a system that was not measured,
    /// [`HttpError::Identity`] when `egress` is not the proxy of the page's context or of the
    /// browser, [`HttpError::BrowserProxy`] when the browser has a browser-wide proxy and no
    /// egress was given, [`HttpError::Options`] for a bad option, and [`HttpError::Browser`] when
    /// the page cannot be read.
    #[tracing::instrument(level = "debug", skip_all)]
    pub async fn from_page(
        page: &Page,
        egress: Option<EgressRef>,
        mut options: HttpOptions,
    ) -> Result<Self, HttpError> {
        options.validate()?;
        let probe = probe(page).await?;
        let (proxy, bypass) = context_proxy(page).await?;
        let egress = if proxy.is_some() {
            reconcile(egress, proxy.as_ref())?
        } else {
            // The context has no proxy of its own, so it leaves the way the browser does.
            let wide = browser_proxy(page).await;
            reconcile_browser_wide(egress, wide.as_ref(), options.direct_behind_browser_proxy)?
        };
        options.bypass.extend(bypass_hosts(&bypass));
        let profile = profile_of(&probe)?;
        let now = (options.clock)();
        let jar = CookieJar::from_cdp(&page.cookies().all().await?, now);
        Self::assemble(profile, jar, egress, options)
    }

    /// Writes into `page`'s browser the cookies that responses set or removed since the last
    /// sync, so the session carries on there. Returns how many cookies were written or removed.
    ///
    /// Only the changes of this session are sent; what the page changed itself is left alone.
    /// If the browser refuses, the changes are kept for the next attempt.
    ///
    /// # Errors
    ///
    /// Returns [`HttpError::Browser`] when the browser rejects a cookie.
    #[tracing::instrument(level = "debug", skip_all)]
    pub async fn sync_cookies_back(&self, page: &Page) -> Result<usize, HttpError> {
        let pending = self.inner().jar.take_pending(self.now());
        let count = pending.cookies.len() + pending.gone.len();
        if count == 0 {
            return Ok(0);
        }
        match write(page, &pending).await {
            Ok(()) => Ok(count),
            Err(error) => {
                self.inner().jar.requeue(&pending);
                Err(error.into())
            }
        }
    }
}

/// Stores the cookies first, then removes the ones that are gone.
async fn write(page: &Page, pending: &Pending) -> Result<(), SeleniumBaseError> {
    if !pending.cookies.is_empty() {
        let cookies: Vec<Value> = pending
            .cookies
            .iter()
            .map(super::cookies::set_params)
            .collect();
        page.execute("Network.setCookies", json!({ "cookies": cookies }))
            .await?;
    }
    for key in &pending.gone {
        page.execute("Network.deleteCookies", key.delete_params())
            .await?;
    }
    Ok(())
}

#[cfg(test)]
mod mocked;

#[cfg(test)]
mod tests {
    use super::*;

    const MAC_155: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/155.0.0.0 Safari/537.36";

    fn probe_with(user_agent: &str) -> Probe {
        Probe {
            user_agent: user_agent.to_owned(),
            languages: Vec::new(),
            product_major: None,
        }
    }

    #[test]
    fn the_chrome_major_is_read_from_the_user_agent() {
        assert_eq!(chrome_major(MAC_155), Some(155));
        assert_eq!(chrome_major("Mozilla/5.0 Firefox/130.0"), None);
        assert_eq!(chrome_major("Chrome/"), None);
        assert_eq!(
            chrome_major("Chrome/9999999999999"),
            None,
            "an overflow is no version"
        );
    }

    #[test]
    fn the_system_is_told_from_the_user_agent() {
        for (agent, os) in [
            (MAC_155, OsType::Macos),
            (
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/155.0.0.0",
                OsType::Windows,
            ),
            (
                "Mozilla/5.0 (X11; Linux x86_64) Chrome/155.0.0.0",
                OsType::Linux,
            ),
            (
                "Mozilla/5.0 (Linux; Android 14) Chrome/155.0.0.0 Mobile",
                OsType::Android,
            ),
            (
                "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) Safari",
                OsType::Ios,
            ),
            (
                "Mozilla/5.0 (X11; CrOS x86_64 14541.0.0) Chrome/155.0.0.0",
                OsType::Linux,
            ),
        ] {
            assert_eq!(os_of(agent).expect(agent), os, "{agent}");
        }
        assert!(os_of("curl/8").is_err());
    }

    #[test]
    fn accept_language_is_built_the_way_chrome_builds_it() {
        let list = |items: &[&str]| items.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert_eq!(
            accept_language(&list(&["en-US", "en"])).as_deref(),
            Some("en-US,en;q=0.9")
        );
        assert_eq!(
            accept_language(&list(&["de-DE", "de", "en-US", "en"])).as_deref(),
            Some("de-DE,de;q=0.9,en-US;q=0.8,en;q=0.7")
        );
        assert_eq!(accept_language(&list(&["fr"])).as_deref(), Some("fr"));
        assert_eq!(accept_language(&list(&[" ", ""])), None);
        assert_eq!(accept_language(&[]), None);
        assert_eq!(
            accept_language(&list(&["en", "en", " en "])).as_deref(),
            Some("en"),
            "repeats collapse"
        );
        let many: Vec<String> = (0..30).map(|i| format!("l{i}")).collect();
        let header = accept_language(&many).expect("a header");
        assert_eq!(header.matches(',').count(), MAX_LANGUAGES - 1);
        assert!(header.ends_with("q=0.1"), "{header}");
    }

    #[test]
    fn the_measured_major_is_accepted_and_the_user_agent_is_kept() {
        let measured = emulation::get(BrowserKind::Chrome, OsType::Macos)
            .expect("measured")
            .major;
        let agent = MAC_155.replace("155", &measured.to_string());
        let profile = profile_of(&Probe {
            languages: vec!["de-DE".to_owned(), "de".to_owned()],
            ..probe_with(&agent)
        })
        .expect("a profile");
        assert_eq!(profile.user_agent(), agent);
        assert_eq!(profile.accept_language(), Some("de-DE,de;q=0.9"));
        assert_eq!(profile.major(), measured);
    }

    #[test]
    fn another_chrome_major_is_a_version_mismatch_whoever_reports_it() {
        let measured = emulation::get(BrowserKind::Chrome, OsType::Macos)
            .expect("measured")
            .major;
        let newer = measured + 1;
        let by_agent = probe_with(&MAC_155.replace("155", &newer.to_string()));
        assert!(matches!(
            profile_of(&by_agent),
            Err(HttpError::VersionMismatch { browser, measured: m }) if browser == newer && m == measured
        ));
        let agent = MAC_155.replace("155", &measured.to_string());
        let by_product = Probe {
            product_major: Some(newer),
            ..probe_with(&agent)
        };
        assert!(matches!(
            profile_of(&by_product),
            Err(HttpError::VersionMismatch { browser, .. }) if browser == newer
        ));
    }

    #[test]
    fn a_system_that_was_not_measured_and_a_browser_that_is_not_chrome_are_refused() {
        let windows = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/155.0.0.0 Safari/537.36";
        let error = profile_of(&probe_with(windows)).expect_err("not measured");
        assert!(
            matches!(
                &error,
                HttpError::Impersonate(ImpersonateError::Emulation(_))
            ),
            "{error}"
        );
        for agent in [
            "Mozilla/5.0 (Macintosh) Chrome/155.0.0.0 Edg/155.0.0.0",
            "Mozilla/5.0 (Macintosh) Chrome/155.0.0.0 OPR/110.0.0.0",
            "Mozilla/5.0 (Macintosh; rv:130.0) Gecko/20100101 Firefox/130.0",
            "Mozilla/5.0 (Macintosh) Safari/605.1.15",
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/155.0.0.0 Safari/537.36",
        ] {
            assert!(
                matches!(
                    profile_of(&probe_with(agent)),
                    Err(HttpError::Impersonate(_))
                ),
                "{agent}"
            );
        }
    }

    #[test]
    fn chromes_bypass_patterns_become_a_no_proxy_list() {
        let list = bypass_hosts(&[
            "*.internal.example.com".to_owned(),
            "<-loopback>".to_owned(),
            "<local>".to_owned(),
            " intranet ".to_owned(),
            String::new(),
        ]);
        assert_eq!(list, [".internal.example.com", "intranet"]);
    }

    #[test]
    fn the_page_proxy_is_adopted_or_must_match_the_egress_given() {
        let proxy = Proxy::parse("alice:s3cret@proxy.example:8080").expect("a proxy");
        let adopted = reconcile(None, Some(&proxy))
            .expect("ok")
            .expect("an egress");
        assert_eq!(adopted.key().host, "proxy.example");
        assert_eq!(adopted.key().port, 8080);
        assert!(
            adopted.proxy().has_credentials(),
            "the credentials come along"
        );

        let same = EgressRef::parse("http://other:pw@PROXY.example:8080", None).expect("spec");
        let kept = reconcile(Some(same), Some(&proxy))
            .expect("ok")
            .expect("an egress");
        assert_eq!(kept.key().port, 8080);

        let other = EgressRef::parse("http://elsewhere.example:8080", None).expect("spec");
        let error = reconcile(Some(other), Some(&proxy)).expect_err("mismatch");
        assert!(matches!(
            error,
            HttpError::Identity(IdentityError::EgressMismatch { .. })
        ));
        let shown = format!("{error} {error:?}");
        assert!(
            !shown.contains("s3cret") && !shown.contains("alice"),
            "{shown}"
        );

        let none = reconcile(None, None).expect("ok");
        assert!(none.is_none());
        let given = EgressRef::parse("http://elsewhere.example:8080", None).expect("spec");
        assert!(
            reconcile(Some(given), None).expect("ok").is_some(),
            "no context proxy: no check"
        );
    }

    #[test]
    fn socks5_and_socks5h_name_the_same_exit_but_not_http() {
        let key = |spec: &str| EgressRef::parse(spec, None).expect(spec).key().clone();
        assert!(same_exit(
            &key("socks5://h.example:1080"),
            &key("socks5h://h.example:1080")
        ));
        assert!(!same_exit(
            &key("socks5://h.example:1080"),
            &key("http://h.example:1080")
        ));
        assert!(!same_exit(
            &key("http://h.example:80"),
            &key("http://h.example:8080")
        ));
    }

    #[test]
    fn the_browser_wide_proxy_is_read_from_the_command_line() {
        let cases = [
            (
                "--proxy-server=http://127.0.0.1:43210",
                Some(("http://127.0.0.1:43210", true)),
            ),
            (
                "--proxy-server=proxy.example:8080",
                Some(("proxy.example:8080", false)),
            ),
            (
                "--proxy-server=socks5://[::1]:1080",
                Some(("socks5://[::1]:1080", true)),
            ),
            (
                "--proxy-server=\"http://localhost:9\"",
                Some(("http://localhost:9", true)),
            ),
            (
                "--proxy-server=http=a.example:1;https=b.example:2",
                Some(("a.example:1", false)),
            ),
            (
                "--proxy-server=a.example:1 --proxy-server=b.example:2",
                Some(("b.example:2", false)),
            ),
            ("--proxy-server=direct://", None),
            ("--proxy-server=a.example:1 --no-proxy-server", None),
            ("--proxy-server=", None),
            ("--proxy-bypass-list=<-loopback>", None),
            ("", None),
        ];
        for (switches, expected) in cases {
            let line = format!("/chrome --remote-debugging-port=0 {switches} --disable-quic");
            let found = browser_proxy_in(&line).map(|p| (p.server, p.local));
            let expected = expected.map(|(server, local)| (server.to_owned(), local));
            assert_eq!(found, expected, "{switches}");
        }
    }

    #[test]
    fn a_browser_proxy_needs_an_egress_or_the_word_that_it_leaves_directly() {
        let remote = browser_proxy_in("--proxy-server=http://proxy.example:8080").expect("a proxy");
        let local = browser_proxy_in("--proxy-server=http://127.0.0.1:43210").expect("a proxy");
        let egress = |spec: &str| EgressRef::parse(spec, None).expect(spec);

        for wide in [&remote, &local] {
            let error = reconcile_browser_wide(None, Some(wide), false).expect_err("not direct");
            assert!(
                matches!(&error, HttpError::BrowserProxy { proxy } if *proxy == wide.server),
                "{error}"
            );
            assert!(reconcile_browser_wide(None, Some(wide), true)
                .expect("said to be direct")
                .is_none());
        }
        let own = egress("http://alice:s3cret@PROXY.example:8080");
        assert!(reconcile_browser_wide(Some(own), Some(&remote), false)
            .expect("its own proxy")
            .is_some());
        let other = egress("http://alice:s3cret@elsewhere.example:8080");
        let error = reconcile_browser_wide(Some(other.clone()), Some(&remote), false)
            .expect_err("not its proxy");
        assert!(matches!(
            error,
            HttpError::Identity(IdentityError::EgressMismatch { .. })
        ));
        let shown = format!("{error} {error:?}");
        assert!(
            !shown.contains("s3cret") && !shown.contains("alice"),
            "{shown}"
        );
        assert!(reconcile_browser_wide(Some(other), Some(&local), false)
            .expect("the exit behind a local proxy is taken as given")
            .is_some());
        assert!(reconcile_browser_wide(None, None, false)
            .expect("no proxy")
            .is_none());
    }
}
