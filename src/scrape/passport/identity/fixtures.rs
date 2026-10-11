//! Shared builders for the tests of the passport, the events, the tracing sink and the identity file.

use url::Url;

use super::{EmulationRef, StealthIdentity};
use crate::scrape::egress::geo::CountryCode;
use crate::scrape::egress::route::EgressKey;
use crate::scrape::jar::store::StoreOutcome;
use crate::stealth::fingerprint::{Fingerprint, OsType};
use crate::wire::BrowserKind;

/// An arbitrary "now", in seconds since the Unix epoch.
pub(crate) const NOW: u64 = 1_800_000_000;

/// The Chrome major the sample identity claims.
pub(crate) const MAJOR: u32 = 155;

/// A coherent macOS fingerprint claiming [`MAJOR`].
pub(crate) fn fingerprint() -> Fingerprint {
    Fingerprint::randomized_with_major(OsType::Macos, 7, MAJOR)
}

/// The emulation the sample identity was measured against.
pub(crate) fn emulation() -> EmulationRef {
    EmulationRef {
        kind: BrowserKind::Chrome,
        major: MAJOR,
        os: OsType::Macos,
    }
}

/// A proxy egress in Germany on `host`.
pub(crate) fn egress(host: &str) -> EgressKey {
    EgressKey {
        scheme: "http".to_owned(),
        host: host.to_owned(),
        port: 8080,
        country: CountryCode::new("DE").ok(),
    }
}

/// A fresh identity behind `p1.example`, holding no cookie.
pub(crate) fn identity() -> StealthIdentity {
    StealthIdentity::new(fingerprint(), emulation(), Some(egress("p1.example")), NOW)
        .expect("the sample fingerprint is coherent")
}

/// Stores one `Set-Cookie` header received from `url` in the jar of `identity`, at [`NOW`].
pub(crate) fn set_cookie(identity: &mut StealthIdentity, url: &str, header: &str) -> StoreOutcome {
    let url = Url::parse(url).expect("a valid url");
    identity.cookies.store(&url, header, NOW)
}

/// An identity that earned a Cloudflare clearance for `example.com` lasting an hour from [`NOW`].
pub(crate) fn identity_with_clearance() -> StealthIdentity {
    let mut identity = identity();
    let outcome = set_cookie(
        &mut identity,
        "https://www.example.com/",
        "cf_clearance=SECRET-CLEARANCE-VALUE; Domain=example.com; Path=/; Secure; HttpOnly; Max-Age=3600",
    );
    assert_eq!(outcome, StoreOutcome::Stored);
    identity.refresh_clearances(NOW);
    identity
}
