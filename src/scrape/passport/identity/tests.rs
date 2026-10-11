//! Tests of the identity: construction, serde, clearances, and the egress and emulation checks.

use super::fixtures::{
    egress, emulation, fingerprint, identity, identity_with_clearance, set_cookie, MAJOR, NOW,
};
use super::*;
use crate::scrape::egress::geo::CountryCode;
use crate::scrape::jar::store::StoreOutcome;
use crate::stealth::fingerprint::{BatteryProfile, ProxyConfig};
use crate::stealth::humanize::Rng;

/// A proxy password the tests look for in every output.
const PASSWORD: &str = "hunter2-proxy-password";

fn proxy_with(password: &str) -> ProxyConfig {
    ProxyConfig {
        r#type: "http".to_owned(),
        host: "proxy.example".to_owned(),
        port: 8080,
        username: Some("alice".to_owned()),
        password: Some(password.to_owned()),
        save_traffic: false,
    }
}

fn json_of(identity: &StealthIdentity) -> String {
    serde_json::to_string(identity).expect("an identity serializes")
}

#[test]
fn a_new_identity_is_empty_and_current() {
    let fresh = identity();
    assert_eq!(fresh.schema, IDENTITY_SCHEMA);
    assert_eq!(fresh.created_at, NOW);
    assert!(fresh.cookies.is_empty() && fresh.clearances.is_empty() && fresh.accept_ch.is_empty());
    assert_eq!(fresh.egress, Some(egress("p1.example")));
    assert_eq!(fresh.emulation, emulation());
}

#[test]
fn new_clears_the_proxy_because_it_may_hold_a_password() {
    let mut with_proxy = fingerprint();
    with_proxy.proxy = Some(proxy_with(PASSWORD));
    let made = StealthIdentity::new(with_proxy, emulation(), None, NOW).expect("coherent");
    assert!(made.fingerprint.proxy.is_none());
    assert!(!json_of(&made).contains(PASSWORD));
    assert!(!format!("{made:?}").contains(PASSWORD));
}

#[test]
fn a_proxy_set_after_construction_is_still_not_serialized() {
    let mut made = identity();
    made.fingerprint.proxy = Some(proxy_with(PASSWORD));
    let json = json_of(&made);
    for secret in [PASSWORD, "alice", "proxy.example"] {
        assert!(!json.contains(secret), "{secret} leaked into {json}");
    }
}

#[test]
fn verify_refuses_an_identity_that_carries_a_proxy() {
    let mut made = identity();
    made.fingerprint.proxy = Some(proxy_with(PASSWORD));
    let error = made.verify().expect_err("a proxy is refused");
    assert!(matches!(error, IdentityError::Incoherent(_)), "{error:?}");
    assert!(!error.to_string().contains(PASSWORD));
}

#[test]
fn no_random_proxy_password_reaches_the_json_the_debug_or_an_error() {
    let mut rng = Rng::new(0x005E_C2E7);
    for _ in 0..64 {
        let secret = format!("pw-{:016x}", rng.next_u64());
        let mut with_proxy = fingerprint();
        with_proxy.proxy = Some(proxy_with(&secret));
        let made = StealthIdentity::new(with_proxy, emulation(), None, NOW).expect("coherent");
        let mismatch = made
            .check_egress(Some(&egress("other.example")))
            .expect_err("a proxy is not the direct connection");
        for text in [json_of(&made), format!("{made:?}"), mismatch.to_string()] {
            assert!(!text.contains(&secret), "{secret} leaked into {text}");
        }
    }
}

#[test]
fn new_refuses_an_incoherent_fingerprint() {
    let mut fp = fingerprint();
    fp.platform = Some("Win32".to_owned());
    let error = StealthIdentity::new(fp, emulation(), None, NOW).expect_err("incoherent");
    match error {
        IdentityError::Incoherent(reasons) => {
            assert!(
                reasons.iter().any(|reason| reason.contains("platform")),
                "{reasons:?}"
            );
        }
        other => panic!("expected Incoherent, got {other:?}"),
    }
}

#[test]
fn new_refuses_a_fingerprint_that_cannot_be_saved_and_read_back() {
    let mut fp = fingerprint();
    fp.battery = Some(BatteryProfile::default());
    let error = StealthIdentity::new(fp, emulation(), None, NOW).expect_err("infinity is not JSON");
    match error {
        IdentityError::Incoherent(reasons) => {
            assert!(
                reasons.iter().any(|reason| reason.contains("saved")),
                "{reasons:?}"
            );
        }
        other => panic!("expected Incoherent, got {other:?}"),
    }
}

#[test]
fn new_refuses_a_number_that_json_turns_into_nothing() {
    let mut fp = fingerprint();
    fp.device_memory = Some(f64::INFINITY);
    let error = StealthIdentity::new(fp, emulation(), None, NOW).expect_err("infinity is not JSON");
    match error {
        IdentityError::Incoherent(reasons) => {
            assert!(reasons[0].contains("not finite"), "{reasons:?}");
        }
        other => panic!("expected Incoherent, got {other:?}"),
    }
}

#[test]
fn an_identity_survives_a_serde_round_trip() {
    let mut made = identity_with_clearance();
    made.accept_ch.insert(
        "https://example.com".to_owned(),
        vec!["Sec-CH-UA-Platform-Version".to_owned()],
    );
    let back: StealthIdentity = serde_json::from_str(&json_of(&made)).expect("reads back");
    assert_eq!(back, made);
    assert_eq!(back.clearances.len(), 1);
    assert_eq!(back.emulation.major, MAJOR);
}

#[test]
fn an_identity_written_without_the_optional_parts_reads_with_defaults() {
    let mut value = serde_json::to_value(identity()).expect("serializes");
    let object = value.as_object_mut().expect("an object");
    for key in ["accept_ch", "egress", "clearances"] {
        object.remove(key);
    }
    let back: StealthIdentity = serde_json::from_value(value).expect("reads");
    assert!(back.accept_ch.is_empty() && back.egress.is_none() && back.clearances.is_empty());
}

#[test]
fn a_tampered_jar_does_not_deserialize() {
    let mut value = serde_json::to_value(identity_with_clearance()).expect("serializes");
    value["cookies"]["cookies"][0]["path"] = serde_json::json!("no-leading-slash");
    let result: Result<StealthIdentity, _> = serde_json::from_value(value);
    assert!(result.is_err(), "a cookie the jar refuses must not be read");
}

#[test]
fn the_emulation_reads_and_writes_a_plain_browser_name() {
    let json = serde_json::to_value(emulation()).expect("serializes");
    assert_eq!(json["kind"], "chrome");
    assert_eq!(json["major"], MAJOR);
    let back: EmulationRef = serde_json::from_value(json.clone()).expect("reads");
    assert_eq!(back, emulation());
    let mut unknown = json;
    unknown["kind"] = serde_json::json!("netscape");
    assert!(serde_json::from_value::<EmulationRef>(unknown).is_err());
}

#[test]
fn debug_shows_the_cookie_count_and_the_egress_and_nothing_else() {
    let made = identity_with_clearance();
    let text = format!("{made:?}");
    assert!(text.contains("cookies: 1"), "{text}");
    assert!(text.contains("p1.example"), "{text}");
    for hidden in ["SECRET-CLEARANCE-VALUE", "cf_clearance", "Mozilla", "Apple"] {
        assert!(!text.contains(hidden), "{hidden} leaked into {text}");
    }
}

#[test]
fn verify_accepts_a_good_identity_and_refuses_another_schema() {
    assert_eq!(identity().verify(), Ok(()));
    let mut future = identity();
    future.schema = IDENTITY_SCHEMA + 1;
    assert_eq!(
        future.verify(),
        Err(IdentityError::Schema(IDENTITY_SCHEMA + 1))
    );
}

#[test]
fn verify_refuses_a_fingerprint_that_no_longer_fits_its_os() {
    let mut made = identity();
    made.fingerprint.platform = Some("Win32".to_owned());
    assert!(matches!(made.verify(), Err(IdentityError::Incoherent(_))));
}

#[test]
fn the_clearance_cookie_of_a_vendor_is_found_in_the_jar() {
    let made = identity_with_clearance();
    assert_eq!(
        made.clearances,
        vec![Clearance {
            site: "example.com".to_owned(),
            vendor: Vendor::Cloudflare,
            cookie: "cf_clearance".to_owned(),
            expires_at: NOW + 3600,
        }]
    );
    let found = made.clearance_for("example.com").expect("a clearance");
    assert_eq!(found.expires_at, NOW + 3600);
}

#[test]
fn a_clearance_is_found_by_any_host_of_the_site_in_any_case() {
    let made = identity_with_clearance();
    for host in ["www.example.com", "EXAMPLE.com", "a.b.example.com"] {
        assert!(made.clearance_for(host).is_some(), "{host}");
    }
    for other in ["example.org", "notexample.com", "com", ""] {
        assert!(made.clearance_for(other).is_none(), "{other}");
    }
}

#[test]
fn only_clearance_cookies_count_and_the_record_holds_no_value() {
    let mut made = identity();
    set_cookie(
        &mut made,
        "https://example.com/",
        "theme=dark; Max-Age=3600",
    );
    set_cookie(
        &mut made,
        "https://example.com/",
        "datadome=SECRET-DD; Max-Age=7200; Secure",
    );
    made.refresh_clearances(NOW);
    assert_eq!(made.clearances.len(), 1);
    assert_eq!(made.clearances[0].vendor, Vendor::DataDome);
    assert_eq!(made.clearances[0].cookie, "datadome");
    assert!(!format!("{:?}", made.clearances).contains("SECRET-DD"));
    assert_eq!(made.cookies.len(), 2, "the ordinary cookie is stored too");
}

#[test]
fn an_expired_cookie_is_not_a_clearance() {
    let mut made = identity_with_clearance();
    made.refresh_clearances(NOW + 3600);
    assert_eq!(made.clearances, Vec::new());
    assert!(made.clearance_for("example.com").is_none());
}

#[test]
fn a_session_clearance_has_no_known_end() {
    let mut made = identity();
    let outcome = set_cookie(
        &mut made,
        "https://example.com/",
        "_px3=session-value; Path=/",
    );
    assert_eq!(outcome, StoreOutcome::Stored);
    made.refresh_clearances(NOW);
    let found = made.clearance_for("example.com").expect("a clearance");
    assert_eq!(found.expires_at, NO_EXPIRY);
    assert_eq!(found.vendor, Vendor::PerimeterX);
}

#[test]
fn the_latest_expiry_per_cookie_wins_and_the_list_is_sorted() {
    let mut made = identity();
    set_cookie(
        &mut made,
        "https://a.example.com/x/",
        "cf_clearance=one; Path=/x/; Max-Age=100",
    );
    set_cookie(
        &mut made,
        "https://a.example.com/y/",
        "cf_clearance=two; Path=/y/; Max-Age=900",
    );
    set_cookie(
        &mut made,
        "https://shop.test/",
        "datadome=three; Max-Age=500",
    );
    made.refresh_clearances(NOW);
    let summary: Vec<(&str, &str, u64)> = made
        .clearances
        .iter()
        .map(|c| (c.site.as_str(), c.cookie.as_str(), c.expires_at))
        .collect();
    assert_eq!(
        summary,
        vec![
            ("example.com", "cf_clearance", NOW + 900),
            ("shop.test", "datadome", NOW + 500)
        ]
    );
}

#[test]
fn a_site_cleared_by_two_vendors_answers_with_the_longer_lived() {
    let mut made = identity();
    set_cookie(
        &mut made,
        "https://example.com/",
        "cf_clearance=a; Max-Age=100",
    );
    set_cookie(&mut made, "https://example.com/", "datadome=b; Max-Age=900");
    made.refresh_clearances(NOW);
    assert_eq!(made.clearances.len(), 2);
    let found = made.clearance_for("example.com").expect("a clearance");
    assert_eq!(found.vendor, Vendor::DataDome);
}

#[test]
fn refreshing_replaces_the_old_list() {
    let mut made = identity_with_clearance();
    made.clearances.push(Clearance {
        site: "stale.test".to_owned(),
        vendor: Vendor::Akamai,
        cookie: "_abck".to_owned(),
        expires_at: NOW + 10,
    });
    made.refresh_clearances(NOW);
    assert_eq!(made.clearances.len(), 1);
    assert_eq!(made.clearances[0].site, "example.com");
}

#[test]
fn every_vendor_with_a_clearance_cookie_is_listed() {
    // An exhaustive match: a new vendor stops this from compiling until it is added to VENDORS.
    fn counted(vendor: Vendor) -> bool {
        match vendor {
            Vendor::Cloudflare
            | Vendor::DataDome
            | Vendor::PerimeterX
            | Vendor::Akamai
            | Vendor::Imperva
            | Vendor::Kasada
            | Vendor::AwsWaf => VENDORS.contains(&vendor),
        }
    }
    assert!(VENDORS.into_iter().all(counted));
    for vendor in VENDORS {
        if let Some(name) = clearance_cookie(vendor) {
            assert_eq!(vendor_of_cookie(name), Some(vendor));
        }
    }
    assert_eq!(vendor_of_cookie("sessionid"), None);
}

#[test]
fn the_same_egress_is_accepted() {
    let made = identity();
    assert_eq!(made.check_egress(Some(&egress("p1.example"))), Ok(()));
}

#[test]
fn another_egress_is_refused_with_both_sides_named() {
    let made = identity();
    let other = egress("p2.example");
    let error = made.check_egress(Some(&other)).expect_err("another proxy");
    assert_eq!(
        error,
        IdentityError::EgressMismatch {
            expected: Some(egress("p1.example")),
            found: Some(other),
        }
    );
    let text = error.to_string();
    assert!(
        text.contains("p1.example") && text.contains("p2.example"),
        "{text}"
    );
}

#[test]
fn the_direct_connection_matches_only_the_direct_connection() {
    let direct = StealthIdentity::new(fingerprint(), emulation(), None, NOW).expect("coherent");
    assert_eq!(direct.check_egress(None), Ok(()));
    let error = direct
        .check_egress(Some(&egress("p1.example")))
        .expect_err("a proxy is not direct");
    assert!(matches!(
        error,
        IdentityError::EgressMismatch { expected: None, .. }
    ));
    assert!(error.to_string().contains("no proxy"));
    let bound = identity();
    assert!(matches!(
        bound.check_egress(None),
        Err(IdentityError::EgressMismatch { found: None, .. })
    ));
}

#[test]
fn a_port_or_scheme_change_is_another_egress() {
    let made = identity();
    let mut other_port = egress("p1.example");
    other_port.port = 9090;
    let mut other_scheme = egress("p1.example");
    other_scheme.scheme = "socks5".to_owned();
    assert!(made.check_egress(Some(&other_port)).is_err());
    assert!(made.check_egress(Some(&other_scheme)).is_err());
}

#[test]
fn an_unknown_country_matches_but_two_known_ones_must_agree() {
    let made = identity();
    let mut unknown = egress("p1.example");
    unknown.country = None;
    assert_eq!(made.check_egress(Some(&unknown)), Ok(()));
    let mut elsewhere = egress("p1.example");
    elsewhere.country = CountryCode::new("FR").ok();
    assert!(made.check_egress(Some(&elsewhere)).is_err());
}

#[test]
fn the_emulation_check_names_the_majors() {
    let made = identity();
    assert_eq!(made.check_emulation(&emulation()), Ok(()));
    let newer = EmulationRef {
        major: MAJOR + 1,
        ..emulation()
    };
    assert_eq!(
        made.check_emulation(&newer),
        Err(IdentityError::EmulationMismatch {
            identity: MAJOR,
            measured: MAJOR + 1
        })
    );
    let other_os = EmulationRef {
        os: OsType::Windows,
        ..emulation()
    };
    assert!(matches!(
        made.check_emulation(&other_os),
        Err(IdentityError::Incoherent(_))
    ));
}

#[test]
fn error_messages_are_readable() {
    assert!(IdentityError::Schema(9).to_string().contains('9'));
    let text = IdentityError::Incoherent(vec!["a".to_owned(), "b".to_owned()]).to_string();
    assert!(text.contains("a; b"), "{text}");
}
