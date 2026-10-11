//! Where the proxy may send a request: which addresses count as "inside".
//!
//! Chrome never resolves a name it hands to a proxy, so its own checks for private and local
//! network access never see the real address of a target; only this proxy does. Without a rule
//! here, any page the browser loads could use the proxy to reach the machine's loopback
//! services, a neighbour on the local network or the cloud metadata address, either by naming
//! them or through a name whose DNS answer flips to one of them. The rule is: the destinations of
//! a direct (no egress) proxy are public addresses unless [`MitmOptions::allow_private_destinations`]
//! says otherwise.
//!
//! * Names are resolved by [`PublicOnly`], which gives the connector only the addresses that
//!   pass [`is_internal`]. The connector connects to exactly the addresses this resolver
//!   returned, so there is no second lookup an attacker's DNS could answer differently.
//! * Literal addresses never reach a resolver, so the proxy refuses them before it opens a
//!   tunnel ([`refused_host`]), as it does the `localhost` names.
//!
//! Through an egress the names are resolved on the egress's side and only the literal check
//! applies: the proxy cannot see where the egress will connect.
//!
//! [`MitmOptions::allow_private_destinations`]: super::MitmOptions::allow_private_destinations

use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::future::ready;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;

use wreq::dns::{Addrs, GaiResolver, Name, Resolve, Resolving};

/// The Azure platform address (WireServer): public on paper, a metadata service in practice.
const AZURE_WIRESERVER: Ipv4Addr = Ipv4Addr::new(168, 63, 129, 16);

/// Whether `ip` is loopback, private, link-local, shared (carrier-grade NAT), multicast,
/// reserved, a cloud metadata address, or an IPv6 form that wraps one of these.
pub(super) fn is_internal(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_internal_v4(v4),
        IpAddr::V6(v6) => is_internal_v6(v6),
    }
}

/// The IPv4 rule: `0/8`, `10/8`, `100.64/10`, `127/8`, `169.254/16`, `172.16/12`, `192.168/16`,
/// multicast (`224/4`), reserved and broadcast (`240/4`), and the Azure `WireServer`.
fn is_internal_v4(ip: Ipv4Addr) -> bool {
    let [first, second, ..] = ip.octets();
    ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_multicast()
        || ip.is_broadcast()
        || first == 0
        || first >= 240
        || (first == 100 && (64..=127).contains(&second))
        || ip == AZURE_WIRESERVER
}

/// The IPv6 rule: unspecified, loopback, link-local (`fe80::/10`), site-local (`fec0::/10`),
/// unique-local (`fc00::/7`), multicast, and every form that carries an IPv4 address (mapped,
/// compatible, NAT64 `64:ff9b::/96`, 6to4 `2002::/16`) whose IPv4 address is internal.
fn is_internal_v6(ip: Ipv6Addr) -> bool {
    let segments = ip.segments();
    let first = segments[0];
    if ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_multicast()
        || (first & 0xffc0) == 0xfe80
        || (first & 0xffc0) == 0xfec0
        || (first & 0xfe00) == 0xfc00
    {
        return true;
    }
    embedded_v4(ip).is_some_and(is_internal_v4)
}

/// The IPv4 address an IPv6 address carries, for the forms that carry one.
fn embedded_v4(ip: Ipv6Addr) -> Option<Ipv4Addr> {
    let segments = ip.segments();
    let address = |high: u16, low: u16| {
        let [first, second] = high.to_be_bytes();
        let [third, fourth] = low.to_be_bytes();
        Ipv4Addr::new(first, second, third, fourth)
    };
    if segments[..5] == [0; 5] && (segments[5] == 0xffff || segments[5] == 0) {
        // `::ffff:a.b.c.d` (mapped) and `::a.b.c.d` (compatible).
        return Some(address(segments[6], segments[7]));
    }
    if segments[0] == 0x0064 && segments[1] == 0xff9b && segments[2..6] == [0; 4] {
        // `64:ff9b::a.b.c.d` (NAT64).
        return Some(address(segments[6], segments[7]));
    }
    if segments[0] == 0x2002 {
        // `2002:aabb:ccdd::` (6to4).
        return Some(address(segments[1], segments[2]));
    }
    None
}

/// Whether `name` is `localhost` or below it (RFC 6761): such a name means the machine itself,
/// whatever the resolver says.
pub(super) fn is_localhost_name(name: &str) -> bool {
    let name = name.trim_end_matches('.');
    name.eq_ignore_ascii_case("localhost")
        || name
            .len()
            .checked_sub(".localhost".len())
            .and_then(|start| name.get(start..))
            .is_some_and(|tail| tail.eq_ignore_ascii_case(".localhost"))
}

/// Why a host (as a URL writes it: IPv6 in brackets) may not be a destination, `None` when it
/// may be. Names other than `localhost` are judged by [`PublicOnly`] when they are resolved.
pub(super) fn refused_host(host: &str) -> Option<&'static str> {
    let bare = host.trim_start_matches('[').trim_end_matches(']');
    if is_localhost_name(bare) {
        return Some("localhost is not a destination");
    }
    match bare.parse::<IpAddr>() {
        Ok(ip) if is_internal(ip) => {
            Some("a local, private or reserved address is not a destination")
        }
        _ => None,
    }
}

/// The resolver failed because every address of the name is internal.
#[derive(Debug)]
pub(super) struct InternalDestination;

impl fmt::Display for InternalDestination {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the name resolves only to local, private or reserved addresses")
    }
}

impl Error for InternalDestination {}

/// Whether `error` (or something it wraps) is [`InternalDestination`].
pub(super) fn is_internal_destination(error: &(dyn Error + 'static)) -> bool {
    let mut link = Some(error);
    while let Some(current) = link {
        if current.is::<InternalDestination>() {
            return true;
        }
        link = current.source();
    }
    false
}

/// A resolver that hands the connector public addresses only.
///
/// Fixed overrides (the caller's `resolve` entries) go through the same filter; a caller who
/// wants a private address for a name says so with `allow_private_destinations`, which leaves
/// this resolver out.
pub(super) struct PublicOnly {
    /// The resolver behind the filter: the system's in production.
    inner: Arc<dyn Resolve>,
    /// Names with a fixed answer.
    overrides: HashMap<String, Vec<SocketAddr>>,
}

impl PublicOnly {
    /// The filter around the system resolver, with the fixed answers of `overrides`.
    pub(super) fn system(overrides: &[(String, SocketAddr)]) -> Self {
        Self::over(Arc::new(GaiResolver::new()), overrides)
    }

    /// The filter around `inner`.
    pub(super) fn over(inner: Arc<dyn Resolve>, overrides: &[(String, SocketAddr)]) -> Self {
        let mut fixed: HashMap<String, Vec<SocketAddr>> = HashMap::new();
        for (host, addr) in overrides {
            fixed.entry(host.clone()).or_default().push(*addr);
        }
        Self {
            inner,
            overrides: fixed,
        }
    }
}

impl Resolve for PublicOnly {
    fn resolve(&self, name: Name) -> Resolving {
        let answer: Resolving = match self.overrides.get(name.as_str()) {
            Some(fixed) => {
                let addrs: Addrs = Box::new(fixed.clone().into_iter());
                Box::pin(ready(Ok(addrs)))
            }
            None => self.inner.resolve(name),
        };
        Box::pin(async move {
            let mut dropped = false;
            let public: Vec<SocketAddr> = answer
                .await?
                .filter(|addr| {
                    let internal = is_internal(addr.ip());
                    dropped |= internal;
                    !internal
                })
                .collect();
            if public.is_empty() && dropped {
                return Err(Box::new(InternalDestination) as Box<dyn Error + Send + Sync>);
            }
            Ok(Box::new(public.into_iter()) as Addrs)
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    fn ip(text: &str) -> IpAddr {
        text.parse().expect("an IP address")
    }

    /// A resolver that answers every name with `answers`, and remembers the names asked.
    struct Stub {
        answers: Vec<SocketAddr>,
        asked: Mutex<Vec<String>>,
    }

    impl Stub {
        fn new(answers: &[&str]) -> Arc<Self> {
            Arc::new(Self {
                answers: answers
                    .iter()
                    .map(|text| text.parse().expect("a socket address"))
                    .collect(),
                asked: Mutex::new(Vec::new()),
            })
        }
    }

    impl Resolve for Stub {
        fn resolve(&self, name: Name) -> Resolving {
            self.asked
                .lock()
                .expect("not poisoned")
                .push(name.as_str().to_owned());
            let addrs: Addrs = Box::new(self.answers.clone().into_iter());
            Box::pin(ready(Ok(addrs)))
        }
    }

    async fn resolved(filter: &PublicOnly, name: &str) -> Result<Vec<SocketAddr>, String> {
        filter
            .resolve(Name::from(name))
            .await
            .map(Iterator::collect)
            .map_err(|error| error.to_string())
    }

    #[test]
    fn internal_addresses_are_recognised() {
        let internal = [
            "0.0.0.0",
            "0.1.2.3",
            "10.0.0.1",
            "10.255.255.255",
            "100.64.0.1",
            "100.100.100.200",
            "100.127.255.255",
            "127.0.0.1",
            "127.255.0.9",
            "169.254.169.254",
            "169.254.0.1",
            "172.16.0.1",
            "172.31.255.255",
            "192.168.1.1",
            "224.0.0.1",
            "239.255.255.250",
            "240.0.0.1",
            "255.255.255.255",
            "168.63.129.16",
            "::",
            "::1",
            "fe80::1",
            "febf::1",
            "fec0::1",
            "fc00::1",
            "fd00:ec2::254",
            "fdff::1",
            "ff02::1",
            "::ffff:127.0.0.1",
            "::ffff:169.254.169.254",
            "::ffff:10.1.2.3",
            "::127.0.0.1",
            "::10.0.0.1",
            "64:ff9b::7f00:1",
            "64:ff9b::a9fe:a9fe",
            "2002:7f00:1::",
            "2002:a9fe:a9fe::1",
        ];
        for text in internal {
            assert!(is_internal(ip(text)), "{text} must be internal");
        }
    }

    #[test]
    fn public_addresses_are_not_internal() {
        let public = [
            "1.1.1.1",
            "8.8.8.8",
            "93.184.216.34",
            "100.63.255.255",
            "100.128.0.1",
            "172.15.255.255",
            "172.32.0.1",
            "169.253.0.1",
            "168.63.129.15",
            "192.167.1.1",
            "198.18.0.1",
            "223.255.255.255",
            "2606:4700:4700::1111",
            "2001:4860:4860::8888",
            "::ffff:93.184.216.34",
            "64:ff9b::5db8:d822",
            "2002:5db8:d822::1",
            "fe00::1",
            "fb00::1",
        ];
        for text in public {
            assert!(!is_internal(ip(text)), "{text} must be public");
        }
    }

    #[test]
    fn localhost_names_are_recognised() {
        for name in [
            "localhost",
            "LOCALHOST",
            "localhost.",
            "app.localhost",
            "a.b.LocalHost.",
        ] {
            assert!(is_localhost_name(name), "{name}");
        }
        for name in [
            "localhost.example",
            "mylocalhost",
            "notlocalhost.com",
            "",
            ".",
        ] {
            assert!(!is_localhost_name(name), "{name}");
        }
    }

    #[test]
    fn literal_hosts_are_refused_the_way_a_url_writes_them() {
        for host in [
            "127.0.0.1",
            "169.254.169.254",
            "10.0.0.5",
            "[::1]",
            "[fd00:ec2::254]",
            "[::ffff:127.0.0.1]",
            "localhost",
            "x.localhost",
        ] {
            assert!(refused_host(host).is_some(), "{host}");
        }
        for host in ["example.test", "93.184.216.34", "[2606:4700:4700::1111]"] {
            assert_eq!(refused_host(host), None, "{host}");
        }
    }

    #[tokio::test]
    async fn only_the_public_addresses_of_an_answer_reach_the_connector() {
        let stub = Stub::new(&["127.0.0.1:0", "93.184.216.34:0", "10.0.0.1:0", "[::1]:0"]);
        let filter = PublicOnly::over(stub, &[]);

        let addrs = resolved(&filter, "mixed.test")
            .await
            .expect("one is public");

        assert_eq!(addrs, ["93.184.216.34:0".parse::<SocketAddr>().unwrap()]);
    }

    #[tokio::test]
    async fn a_name_that_resolves_only_inside_is_an_error_that_says_so() {
        let stub = Stub::new(&["127.0.0.1:0", "169.254.169.254:0"]);
        let filter = PublicOnly::over(stub, &[]);

        let error = filter
            .resolve(Name::from("rebind.test"))
            .await
            .map(Iterator::collect::<Vec<_>>)
            .expect_err("nothing public is left");

        assert!(is_internal_destination(error.as_ref()), "{error}");
    }

    #[tokio::test]
    async fn an_empty_answer_is_left_for_the_connector_to_report() {
        let filter = PublicOnly::over(Stub::new(&[]), &[]);

        let addrs = resolved(&filter, "nothing.test").await.expect("passes");

        assert_eq!(addrs, Vec::<SocketAddr>::new());
    }

    #[tokio::test]
    async fn fixed_answers_skip_the_system_but_not_the_filter() {
        let stub = Stub::new(&["93.184.216.34:0"]);
        let overrides = [
            ("fixed.test".to_owned(), "93.184.216.35:0".parse().unwrap()),
            ("inside.test".to_owned(), "127.0.0.1:9".parse().unwrap()),
        ];
        let filter = PublicOnly::over(Arc::clone(&stub) as Arc<dyn Resolve>, &overrides);

        let fixed = resolved(&filter, "fixed.test").await.expect("public");
        let inside = filter.resolve(Name::from("inside.test")).await;

        assert_eq!(fixed, ["93.184.216.35:0".parse::<SocketAddr>().unwrap()]);
        assert!(inside.is_err(), "a fixed private answer is still refused");
        assert!(
            stub.asked.lock().expect("not poisoned").is_empty(),
            "the system resolver was asked about a fixed name"
        );
    }

    #[test]
    fn the_error_is_found_through_a_chain() {
        #[derive(Debug)]
        struct Wrapper(Box<dyn Error + Send + Sync>);
        impl fmt::Display for Wrapper {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("wrapped")
            }
        }
        impl Error for Wrapper {
            fn source(&self) -> Option<&(dyn Error + 'static)> {
                Some(self.0.as_ref())
            }
        }

        assert!(is_internal_destination(&Wrapper(Box::new(
            InternalDestination
        ))));
        assert!(!is_internal_destination(&Wrapper(Box::new(fmt::Error))));
    }
}
