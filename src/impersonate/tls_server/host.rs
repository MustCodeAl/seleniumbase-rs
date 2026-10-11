//! The host a leaf names: one DNS name or one IP address, never a wildcard.

use std::fmt;
use std::net::IpAddr;

use url::Host;

use super::TlsServerError;

/// The longest DNS name a leaf may carry (RFC 1035, without a trailing dot).
const MAX_DNS_NAME_LEN: usize = 253;
/// The longest DNS label (RFC 1035).
const MAX_DNS_LABEL_LEN: usize = 63;

/// The subject of one leaf: a validated CONNECT host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum LeafName {
    /// A lowercase ASCII DNS name (IDNA names arrive as punycode from `url::Host::parse`).
    Dns(String),
    /// An IPv4 or IPv6 address.
    Ip(IpAddr),
}

impl LeafName {
    /// Validates `host` for a SAN: no wildcard, no empty label, at most 253 bytes.
    pub(super) fn from_host(host: &Host<String>) -> Result<Self, TlsServerError> {
        match host {
            Host::Domain(name) => validate_dns(name).map(Self::Dns),
            Host::Ipv4(ip) => Ok(Self::Ip(IpAddr::V4(*ip))),
            Host::Ipv6(ip) => Ok(Self::Ip(IpAddr::V6(*ip))),
        }
    }

    /// The leaf cache key; the prefix keeps a DNS name and an address apart.
    pub(super) fn cache_key(&self) -> String {
        match self {
            Self::Dns(name) => format!("dns:{name}"),
            Self::Ip(ip) => format!("ip:{ip}"),
        }
    }

    /// Accepts the client's SNI when it names this host: equal to a DNS name (ASCII
    /// case-insensitive), absent for an IP address (TLS clients never send an address as SNI).
    pub(super) fn check_sni(&self, sni: Option<&str>) -> Result<(), TlsServerError> {
        let matches = match (self, sni) {
            (Self::Dns(name), Some(sni)) => name.eq_ignore_ascii_case(sni),
            (Self::Ip(_), None) => true,
            _ => false,
        };
        if matches {
            Ok(())
        } else {
            Err(TlsServerError::SniMismatch {
                expected: self.to_string(),
                sni: sni.map(|sni| sni.chars().take(MAX_DNS_NAME_LEN).collect()),
            })
        }
    }
}

impl fmt::Display for LeafName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dns(name) => f.write_str(name),
            Self::Ip(ip) => fmt::Display::fmt(ip, f),
        }
    }
}

/// Lowercases `name` and checks every label.
fn validate_dns(name: &str) -> Result<String, TlsServerError> {
    if name.is_empty() {
        return Err(TlsServerError::InvalidHost("the name is empty"));
    }
    if name.len() > MAX_DNS_NAME_LEN {
        return Err(TlsServerError::InvalidHost(
            "the name is longer than 253 bytes",
        ));
    }
    let name = name.to_ascii_lowercase();
    name.split('.').try_for_each(validate_label)?;
    Ok(name)
}

/// One label: 1 to 63 bytes of `a-z`, `0-9`, `-` or `_`, not starting or ending with `-`.
fn validate_label(label: &str) -> Result<(), TlsServerError> {
    let problem = if label.is_empty() {
        Some("a label is empty")
    } else if label.len() > MAX_DNS_LABEL_LEN {
        Some("a label is longer than 63 bytes")
    } else if label.contains('*') {
        Some("wildcard names are never issued")
    } else if !label
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
    {
        Some("a label holds a byte other than a-z, 0-9, '-' or '_'")
    } else if label.starts_with('-') || label.ends_with('-') {
        Some("a label starts or ends with '-'")
    } else {
        None
    };
    problem.map_or(Ok(()), |reason| Err(TlsServerError::InvalidHost(reason)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn domain(name: &str) -> Host<String> {
        Host::Domain(name.to_owned())
    }

    fn invalid(host: &Host<String>) -> bool {
        matches!(
            LeafName::from_host(host),
            Err(TlsServerError::InvalidHost(_))
        )
    }

    #[test]
    fn dns_names_are_lowercased_and_keyed_apart_from_addresses() {
        let name = LeafName::from_host(&domain("Example.TEST")).expect("valid name");
        assert_eq!(name, LeafName::Dns("example.test".to_owned()));
        assert_eq!(name.cache_key(), "dns:example.test");
        let parsed = Host::parse("127.0.0.1").expect("address");
        let ip = LeafName::from_host(&parsed).expect("valid address");
        assert_eq!(ip.cache_key(), "ip:127.0.0.1");
        let numeric = LeafName::from_host(&domain("127.0.0.1")).expect("numeric labels");
        assert_ne!(numeric.cache_key(), ip.cache_key());
    }

    #[test]
    fn wildcards_bad_labels_and_long_names_are_refused() {
        assert!(invalid(&domain("*.example.test")));
        assert!(invalid(&domain("")));
        assert!(invalid(&domain("a..test")));
        assert!(invalid(&domain("example.test.")));
        assert!(invalid(&domain("-a.test")));
        assert!(invalid(&domain("a-.test")));
        assert!(invalid(&domain("sp ace.test")));
        assert!(invalid(&domain(&format!("{}.test", "a".repeat(64)))));
        let long = vec!["a".repeat(63); 4].join(".");
        assert_eq!(long.len(), 255);
        assert!(invalid(&domain(&long)));
        let longest = format!("{}.{}", vec!["a".repeat(63); 3].join("."), "b".repeat(61));
        assert_eq!(longest.len(), MAX_DNS_NAME_LEN);
        assert!(!invalid(&domain(&longest)));
        assert!(!invalid(&domain("under_score.test")));
    }

    #[test]
    fn sni_must_name_the_dns_host_and_be_absent_for_an_address() {
        let name = LeafName::from_host(&domain("example.test")).expect("valid name");
        assert!(name.check_sni(Some("example.test")).is_ok());
        assert!(name.check_sni(Some("EXAMPLE.test")).is_ok());
        assert!(name.check_sni(Some("other.test")).is_err());
        assert!(name.check_sni(None).is_err());
        let ip = LeafName::from_host(&Host::parse("[::1]").expect("address")).expect("valid");
        assert_eq!(ip.to_string(), "::1");
        assert!(ip.check_sni(None).is_ok());
        assert!(ip.check_sni(Some("::1")).is_err());
    }

    #[test]
    fn a_reported_sni_is_cut_to_the_longest_dns_name() {
        let name = LeafName::from_host(&domain("example.test")).expect("valid name");
        let hostile = "x".repeat(4 * MAX_DNS_NAME_LEN);
        match name.check_sni(Some(&hostile)) {
            Err(TlsServerError::SniMismatch { expected, sni }) => {
                assert_eq!(expected, "example.test");
                assert_eq!(sni.map(|s| s.len()), Some(MAX_DNS_NAME_LEN));
            }
            other => panic!("expected an SNI mismatch, got {other:?}"),
        }
    }
}
