//! An identity going into a session: the egress it belongs to, the Chrome version it claims, and
//! the systems that were never measured.

use seleniumbase_rs::impersonate::client::ImpersonateError;
use seleniumbase_rs::impersonate::http_session::{HttpError, HttpOptions, HttpSession};
use seleniumbase_rs::scrape::passport::identity::{EmulationRef, IdentityError};
use seleniumbase_rs::stealth::fingerprint::OsType;
use seleniumbase_rs::wire::BrowserKind;

use crate::site::{egress, fingerprint, identity, identity_of, measured_major};

const SPEC: &str = "http://alice:s3cret@127.0.0.1:9";

fn emulation(major: u32, os: OsType) -> EmulationRef {
    EmulationRef {
        kind: BrowserKind::Chrome,
        major,
        os,
    }
}

fn behind(spec: Option<&str>) -> seleniumbase_rs::scrape::passport::identity::StealthIdentity {
    let major = measured_major();
    identity_of(
        fingerprint(OsType::Macos, major),
        emulation(major, OsType::Macos),
        spec.map(|spec| egress(spec).key().clone()),
    )
}

#[test]
fn an_identity_that_belongs_to_a_proxy_is_refused_on_a_direct_connection_and_the_other_way_round() {
    let proxied = behind(Some(SPEC));
    let direct = behind(None);

    let on_direct =
        HttpSession::from_identity(&proxied, None, HttpOptions::new()).expect_err("mismatch");
    let on_proxy = HttpSession::from_identity(&direct, Some(egress(SPEC)), HttpOptions::new())
        .expect_err("mismatch");

    for error in [on_direct, on_proxy] {
        assert!(
            matches!(
                &error,
                HttpError::Identity(IdentityError::EgressMismatch { .. })
            ),
            "{error}"
        );
        let shown = format!("{error} {error:?}");
        assert!(
            !shown.contains("s3cret") && !shown.contains("alice"),
            "{shown}"
        );
    }
}

#[test]
fn an_identity_for_one_proxy_is_refused_behind_another() {
    let proxied = behind(Some(SPEC));
    let other = egress("http://alice:s3cret@127.0.0.1:10");

    let error = HttpSession::from_identity(&proxied, Some(other), HttpOptions::new())
        .expect_err("mismatch");

    assert!(
        matches!(
            error,
            HttpError::Identity(IdentityError::EgressMismatch { .. })
        ),
        "{error}"
    );
}

#[test]
fn the_matching_proxy_is_accepted_and_its_password_stays_out_of_every_view() {
    let proxied = behind(Some(SPEC));

    let session = HttpSession::from_identity(&proxied, Some(egress(SPEC)), HttpOptions::new())
        .expect("a session");

    assert_eq!(session.egress().map(|key| key.port), Some(9));
    let shown = format!("{session:?} {:?}", session.egress());
    for secret in ["s3cret", "alice"] {
        assert!(!shown.contains(secret), "{secret} in {shown}");
    }
}

#[test]
fn an_identity_that_claims_another_chrome_than_the_measured_one_is_refused() {
    let measured = measured_major();
    let newer = measured + 1;
    let claim = identity_of(
        fingerprint(OsType::Macos, newer),
        emulation(newer, OsType::Macos),
        None,
    );

    let error = HttpSession::from_identity(&claim, None, HttpOptions::new()).expect_err("refused");

    assert!(
        matches!(error, HttpError::VersionMismatch { browser, measured: m } if browser == newer && m == measured),
        "{error}"
    );
}

#[test]
fn an_identity_whose_emulation_was_recorded_for_another_version_is_refused() {
    let measured = measured_major();
    let stale = identity_of(
        fingerprint(OsType::Macos, measured),
        emulation(measured - 1, OsType::Macos),
        None,
    );

    let error = HttpSession::from_identity(&stale, None, HttpOptions::new()).expect_err("refused");

    assert!(
        matches!(error, HttpError::VersionMismatch { browser, measured: m } if browser == measured - 1 && m == measured),
        "{error}"
    );
}

#[test]
fn a_system_that_was_not_measured_is_refused_not_approximated() {
    let major = measured_major();
    for os in [OsType::Windows, OsType::Linux] {
        let persona = identity_of(fingerprint(os, major), emulation(major, os), None);

        let error = HttpSession::from_identity(&persona, None, HttpOptions::new())
            .expect_err("not measured");

        assert!(
            matches!(
                &error,
                HttpError::Impersonate(ImpersonateError::Emulation(_))
            ),
            "{os:?}: {error}"
        );
    }
}

#[test]
fn the_installed_chrome_must_be_the_measured_one() {
    let measured = measured_major();

    let stale = HttpSession::from_identity(
        &identity(),
        None,
        HttpOptions::new().installed_chrome_major(measured + 1),
    )
    .expect_err("the measurement is stale");
    let current = HttpSession::from_identity(
        &identity(),
        None,
        HttpOptions::new().installed_chrome_major(measured),
    );

    assert!(
        matches!(stale, HttpError::VersionMismatch { browser, measured: m } if browser == measured + 1 && m == measured),
        "{stale}"
    );
    assert!(current.is_ok());
}

#[test]
fn an_identity_that_carries_a_proxy_in_its_fingerprint_is_refused() {
    let mut tainted = identity();
    tainted.fingerprint.proxy = Some(seleniumbase_rs::ProxyConfig::default());

    let error =
        HttpSession::from_identity(&tainted, None, HttpOptions::new()).expect_err("refused");

    assert!(
        matches!(error, HttpError::Identity(IdentityError::Incoherent(_))),
        "{error}"
    );
}

#[test]
fn an_identity_with_another_schema_is_refused() {
    let mut future = identity();
    future.schema += 1;

    let error = HttpSession::from_identity(&future, None, HttpOptions::new()).expect_err("refused");

    assert!(
        matches!(error, HttpError::Identity(IdentityError::Schema(_))),
        "{error}"
    );
}

#[test]
fn the_session_speaks_with_the_identitys_user_agent_and_languages() {
    let base = identity();

    let session = HttpSession::from_identity(&base, None, HttpOptions::new()).expect("a session");

    assert_eq!(
        Some(session.profile().user_agent()),
        base.fingerprint.user_agent.as_deref()
    );
    assert_eq!(session.profile().major(), measured_major());
}
