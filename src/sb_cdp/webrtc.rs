//! Keeping WebRTC from revealing a machine's network addresses.
//!
//! A page can open a `RTCPeerConnection` without asking, and the ICE
//! candidates it gathers list the machine's addresses: private ones, `.local`
//! names that stand in for them, and, via STUN, the public one. That is the
//! classic way a proxy or VPN is bypassed.
//!
//! Chrome's `--force-webrtc-ip-handling-policy` flag (the
//! [`WebRtcPolicy`](crate::WebRtcPolicy) a `Fingerprint` carries) limits which
//! addresses are offered, but on Chrome 155 every value of it still lists a
//! `.local` host candidate, which tells a script the machine is gathering
//! candidates at all. Making WebRTC relay-only, with
//! [`Page::shield_webrtc`] or
//! [`LaunchOptionsBuilder::shield_webrtc`](super::LaunchOptionsBuilder::shield_webrtc),
//! gathers none. [`Page::webrtc_report`] checks the result by gathering
//! candidates the way a tracking script would.
//!
//! # Examples
//!
//! ```no_run
//! use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
//!
//! # async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
//! let browser = Browser::launch(
//!     LaunchOptions::builder().shield_webrtc(true).build()?,
//! )
//! .await?;
//! let page = browser.default_page().await?;
//! assert!(page.webrtc_report().await?.is_clean());
//! # Ok(())
//! # }
//! ```

use std::net::IpAddr;

use serde_json::json;

use super::Page;
use crate::error::SeleniumBaseError;
use crate::stealth::fingerprint::WebRtcPolicy;

/// The Chrome flag that applies `policy` to every tab.
pub(super) fn chrome_flag(policy: WebRtcPolicy) -> &'static str {
    match policy {
        WebRtcPolicy::DisableNonProxiedUdp => {
            "--force-webrtc-ip-handling-policy=disable_non_proxied_udp"
        }
        WebRtcPolicy::PublicInterfaceOnly => {
            "--force-webrtc-ip-handling-policy=default_public_interface_only"
        }
        WebRtcPolicy::PublicAndPrivateInterfaces => {
            "--force-webrtc-ip-handling-policy=default_public_and_private_interfaces"
        }
    }
}

/// Makes every `RTCPeerConnection` relay-only and drops its STUN servers, so
/// it gathers no candidate of its own and contacts no third party.
pub(super) const RELAY_ONLY_SHIM: &str = r"(() => {
  if (window.__sbWebRtcShield || !window.RTCPeerConnection) return;
  window.__sbWebRtcShield = true;
  const Native = window.RTCPeerConnection;
  const isRelay = (server) => [].concat(server.urls || server.url || [])
    .every((url) => /^turns?:/i.test(url));
  function Shielded(config, constraints) {
    const given = config || {};
    const shielded = Object.assign({}, given, {
      iceTransportPolicy: 'relay',
      iceServers: (given.iceServers || []).filter(isRelay),
    });
    return new Native(shielded, constraints);
  }
  Shielded.prototype = Native.prototype;
  Object.setPrototypeOf(Shielded, Native);
  if (Native.generateCertificate) Shielded.generateCertificate = Native.generateCertificate.bind(Native);
  for (const name of ['RTCPeerConnection', 'webkitRTCPeerConnection']) {
    if (name in window) {
      Object.defineProperty(window, name, { value: Shielded, writable: true, configurable: true });
    }
  }
})();";

/// Gathers ICE candidates the way a tracking script would: no STUN server, so
/// nothing leaves the machine, just the local candidates a page can learn.
const PROBE: &str = r"(async () => {
  const found = [];
  const pc = new RTCPeerConnection({ iceServers: [] });
  pc.createDataChannel('probe');
  pc.onicecandidate = (e) => { if (e.candidate && e.candidate.candidate) found.push(e.candidate.candidate); };
  await pc.setLocalDescription(await pc.createOffer());
  await new Promise((resolve) => {
    if (pc.iceGatheringState === 'complete') return resolve();
    pc.onicegatheringstatechange = () => { if (pc.iceGatheringState === 'complete') resolve(); };
    setTimeout(resolve, 3000);
  });
  pc.close();
  return found;
})()";

/// What produced an ICE candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CandidateKind {
    /// An address of one of this machine's interfaces.
    Host,
    /// The public address a STUN server saw.
    ServerReflexive,
    /// An address learned from the other end of a connection.
    PeerReflexive,
    /// An address on a relay (TURN) server: not this machine's.
    Relay,
}

/// What an ICE candidate's address reveals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AddressKind {
    /// A `.local` name standing in for a private address.
    Mdns,
    /// A private, loopback or link-local address.
    Private,
    /// A public address.
    Public,
}

/// One ICE candidate a page gathered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IceCandidate {
    /// The candidate line as the page saw it.
    pub raw: String,
    /// How it was produced.
    pub kind: CandidateKind,
    /// The address or `.local` name.
    pub address: String,
    /// What the address reveals.
    pub address_kind: AddressKind,
}

impl IceCandidate {
    /// Parses an SDP candidate line such as `candidate:1 1 udp 2113937151
    /// 192.168.1.5 54321 typ host`. Returns `None` for anything else.
    #[must_use]
    pub fn parse(line: &str) -> Option<Self> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 8 || parts[6] != "typ" || !parts[0].starts_with("candidate:") {
            return None;
        }
        let kind = match parts[7] {
            "host" => CandidateKind::Host,
            "srflx" => CandidateKind::ServerReflexive,
            "prflx" => CandidateKind::PeerReflexive,
            "relay" => CandidateKind::Relay,
            _ => return None,
        };
        let address = parts[4];
        Some(Self {
            raw: line.to_owned(),
            kind,
            address: address.to_owned(),
            address_kind: classify_address(address),
        })
    }

    /// Whether the candidate reveals something about this machine: anything
    /// but a relay's address.
    #[must_use]
    pub fn leaks(&self) -> bool {
        self.kind != CandidateKind::Relay
    }
}

fn classify_address(address: &str) -> AddressKind {
    if address.to_ascii_lowercase().ends_with(".local") {
        return AddressKind::Mdns;
    }
    // Anything unrecognisable is treated as public: the cautious reading.
    let Ok(ip) = address.parse::<IpAddr>() else {
        return AddressKind::Public;
    };
    let private = match ip {
        IpAddr::V4(v4) => {
            let [a, b, ..] = v4.octets();
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_unspecified()
                // Carrier-grade NAT, 100.64.0.0/10.
                || (a == 100 && (64..128).contains(&b))
        }
        IpAddr::V6(v6) => {
            let first = v6.segments()[0];
            v6.is_loopback()
                || v6.is_unspecified()
                // Unique local fc00::/7 and link-local fe80::/10.
                || (first & 0xfe00) == 0xfc00
                || (first & 0xffc0) == 0xfe80
        }
    };
    if private {
        AddressKind::Private
    } else {
        AddressKind::Public
    }
}

/// The candidates a page gathered when probed; see
/// [`Page::webrtc_report`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WebRtcReport {
    candidates: Vec<IceCandidate>,
}

impl WebRtcReport {
    /// A report of these candidates.
    #[must_use]
    pub fn new(candidates: Vec<IceCandidate>) -> Self {
        Self { candidates }
    }

    /// Every candidate gathered.
    #[must_use]
    pub fn candidates(&self) -> &[IceCandidate] {
        &self.candidates
    }

    /// The candidates that reveal something about this machine.
    #[must_use]
    pub fn leaks(&self) -> Vec<&IceCandidate> {
        self.candidates.iter().filter(|c| c.leaks()).collect()
    }

    /// Whether nothing about this machine was revealed.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.candidates.iter().all(|c| !c.leaks())
    }
}

impl Page {
    /// Makes WebRTC relay-only in this tab, now and on every page it loads.
    ///
    /// The tab then gathers no host, `.local` or public candidate and contacts
    /// no STUN server, so it cannot reveal the machine's addresses. Pages that
    /// need a WebRTC call to connect will not, unless they bring a TURN relay.
    /// To shield every tab from launch, use
    /// [`LaunchOptionsBuilder::shield_webrtc`](super::LaunchOptionsBuilder::shield_webrtc).
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser refuses.
    pub async fn shield_webrtc(&self) -> Result<(), SeleniumBaseError> {
        self.execute(
            "Page.addScriptToEvaluateOnNewDocument",
            json!({ "source": RELAY_ONLY_SHIM }),
        )
        .await?;
        self.evaluate(RELAY_ONLY_SHIM).await?;
        Ok(())
    }

    /// Gathers ICE candidates the way a tracking script would and reports what
    /// they reveal.
    ///
    /// No STUN server is contacted, so only what a page learns locally is
    /// probed. The page must be on an origin that may use WebRTC (not
    /// `about:blank` in every Chrome version).
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the probe cannot run, for
    /// example because the page has no `RTCPeerConnection`.
    pub async fn webrtc_report(&self) -> Result<WebRtcReport, SeleniumBaseError> {
        let lines: Vec<String> = self.evaluate_as(PROBE).await?;
        Ok(WebRtcReport::new(
            lines
                .iter()
                .filter_map(|line| IceCandidate::parse(line))
                .collect(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(line: &str) -> IceCandidate {
        IceCandidate::parse(line).unwrap_or_else(|| panic!("{line:?} should parse"))
    }

    #[test]
    fn a_private_host_candidate_is_a_leak() {
        let c =
            parse("candidate:842163049 1 udp 1677729535 192.168.1.20 54321 typ host generation 0");
        assert_eq!(
            (c.kind, c.address_kind),
            (CandidateKind::Host, AddressKind::Private)
        );
        assert_eq!(c.address, "192.168.1.20");
        assert!(c.leaks());
    }

    #[test]
    fn an_mdns_name_is_recognised_whatever_its_case() {
        for name in ["9b1c4d2e-aaaa-bbbb-cccc-0123456789ab.local", "ABC.LOCAL"] {
            let c = parse(&format!(
                "candidate:1 1 udp 2113937151 {name} 50000 typ host"
            ));
            assert_eq!(c.address_kind, AddressKind::Mdns, "{name}");
            assert!(c.leaks());
        }
    }

    #[test]
    fn a_server_reflexive_candidate_exposes_a_public_address() {
        let c = parse(
            "candidate:2 1 udp 1685987071 203.0.113.9 61000 typ srflx raddr 10.0.0.2 rport 61000",
        );
        assert_eq!(
            (c.kind, c.address_kind),
            (CandidateKind::ServerReflexive, AddressKind::Public)
        );
        assert!(c.leaks());
    }

    #[test]
    fn a_relay_candidate_reveals_nothing_about_this_machine() {
        let c =
            parse("candidate:3 1 udp 41885439 198.51.100.7 3478 typ relay raddr 0.0.0.0 rport 0");
        assert_eq!(c.kind, CandidateKind::Relay);
        assert!(!c.leaks());
    }

    #[test]
    fn address_ranges_are_classified() {
        for (address, expected) in [
            ("10.1.2.3", AddressKind::Private),
            ("172.16.0.9", AddressKind::Private),
            ("172.32.0.9", AddressKind::Public),
            ("192.168.0.1", AddressKind::Private),
            ("127.0.0.1", AddressKind::Private),
            ("169.254.10.10", AddressKind::Private),
            ("100.64.0.1", AddressKind::Private),
            ("100.128.0.1", AddressKind::Public),
            ("8.8.8.8", AddressKind::Public),
            ("::1", AddressKind::Private),
            ("fe80::1", AddressKind::Private),
            ("fd12:3456::1", AddressKind::Private),
            ("2001:db8::1", AddressKind::Public),
            ("not-an-address", AddressKind::Public),
        ] {
            assert_eq!(classify_address(address), expected, "{address}");
        }
    }

    #[test]
    fn lines_that_are_not_candidates_do_not_parse() {
        for line in [
            "",
            "a=ice-ufrag:abc",
            "candidate:1 1 udp 1 1.2.3.4 5",
            "candidate:1 1 udp 1 1.2.3.4 5 typ bogus",
            "x 1 udp 1 1.2.3.4 5 typ host",
        ] {
            assert!(IceCandidate::parse(line).is_none(), "{line:?}");
        }
    }

    #[test]
    fn a_report_is_clean_only_when_every_candidate_is_a_relay() {
        let relay = parse("candidate:3 1 udp 41885439 198.51.100.7 3478 typ relay");
        let host = parse("candidate:1 1 udp 1 10.0.0.2 5000 typ host");
        assert!(
            WebRtcReport::default().is_clean(),
            "no candidates leaks nothing"
        );
        assert!(WebRtcReport::new(vec![relay.clone()]).is_clean());
        let mixed = WebRtcReport::new(vec![relay, host.clone()]);
        assert!(!mixed.is_clean());
        assert_eq!(mixed.leaks(), [&host]);
        assert_eq!(mixed.candidates().len(), 2);
    }

    #[test]
    fn each_policy_maps_to_its_chrome_flag() {
        for (policy, value) in [
            (
                WebRtcPolicy::DisableNonProxiedUdp,
                "disable_non_proxied_udp",
            ),
            (
                WebRtcPolicy::PublicInterfaceOnly,
                "default_public_interface_only",
            ),
            (
                WebRtcPolicy::PublicAndPrivateInterfaces,
                "default_public_and_private_interfaces",
            ),
        ] {
            assert_eq!(
                chrome_flag(policy),
                format!("--force-webrtc-ip-handling-policy={value}")
            );
        }
    }

    #[test]
    fn the_shim_leaves_only_relay_servers_so_no_stun_server_is_contacted() {
        let shim = RELAY_ONLY_SHIM;
        assert!(shim.contains("iceTransportPolicy: 'relay'"));
        assert!(shim.contains("turns?:"), "only TURN servers survive");
        assert!(
            shim.contains("window.__sbWebRtcShield"),
            "installing twice is harmless"
        );
    }

    #[test]
    fn chrome_is_left_alone_unless_a_policy_is_chosen() {
        use super::super::LaunchOptions;
        let args = |builder: super::super::LaunchOptionsBuilder| {
            builder
                .build()
                .unwrap()
                .browser_args(std::path::Path::new("/tmp/profile"))
        };
        assert!(!args(LaunchOptions::builder())
            .iter()
            .any(|arg| arg.contains("webrtc")));
        assert!(
            !args(LaunchOptions::builder().shield_webrtc(true))
                .iter()
                .any(|arg| arg.contains("webrtc")),
            "the shield is a script, not a flag"
        );
        assert!(
            args(LaunchOptions::builder().webrtc_policy(WebRtcPolicy::PublicInterfaceOnly))
                .contains(
                    &"--force-webrtc-ip-handling-policy=default_public_interface_only".to_owned()
                )
        );
    }
}
