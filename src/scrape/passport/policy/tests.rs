//! Tests of the session policy: every row of both tables, and the edges of the margin.

use super::*;
use crate::scrape::challenge::detect::{Evidence, Vendor};
use crate::scrape::challenge::mitigation::GiveUpReason;
use crate::scrape::passport::identity::fixtures::{identity, identity_with_clearance, NOW};
use crate::scrape::passport::identity::{Clearance, NO_EXPIRY};

const SITE: &str = "example.com";

/// An identity holding a clearance for [`SITE`] that ends at `expires_at`.
fn cleared_until(expires_at: u64) -> StealthIdentity {
    let mut made = identity();
    made.clearances.push(Clearance {
        site: SITE.to_owned(),
        vendor: Vendor::Cloudflare,
        cookie: "cf_clearance".to_owned(),
        expires_at,
    });
    made
}

/// The state of a site whose history prefers `mode`.
fn site_preferring(mode: Mode) -> DomainState {
    DomainState {
        preferred_mode: Some(mode),
        ..DomainState::new(SITE)
    }
}

fn seen(kind: ChallengeKind) -> Detection {
    Detection {
        kind,
        evidence: Evidence::default(),
    }
}

fn before(
    policy: &SessionPolicy,
    mode: Mode,
    id: &StealthIdentity,
    st: Option<&DomainState>,
) -> Transition {
    policy.before_fetch(mode, id, SITE, st, NOW)
}

fn expiring(remaining_secs: u64) -> Transition {
    Transition::Escalate(EscalateReason::ClearanceExpiring { remaining_secs })
}

#[test]
fn the_defaults_are_the_documented_ones() {
    let policy = SessionPolicy::default();
    assert!(!policy.require_clearance_for_http);
    assert_eq!(policy.clearance_margin, Duration::from_secs(120));
    assert_eq!(
        policy.clearance_margin.as_secs(),
        DEFAULT_CLEARANCE_MARGIN_SECS
    );
    assert!(policy.demote_after_solve);
    assert_eq!(policy.prefer, Mode::Http);
}

#[test]
fn an_http_session_without_history_or_clearance_stays() {
    let policy = SessionPolicy::default();
    assert_eq!(
        before(&policy, Mode::Http, &identity(), None),
        Transition::Stay
    );
    let fresh = DomainState::new(SITE);
    assert_eq!(
        before(&policy, Mode::Http, &identity(), Some(&fresh)),
        Transition::Stay
    );
}

#[test]
fn a_required_clearance_that_is_missing_escalates() {
    let policy = SessionPolicy {
        require_clearance_for_http: true,
        ..SessionPolicy::default()
    };
    assert_eq!(
        before(&policy, Mode::Http, &identity(), None),
        Transition::Escalate(EscalateReason::NoClearance)
    );
}

#[test]
fn a_clearance_for_another_site_does_not_count() {
    let policy = SessionPolicy {
        require_clearance_for_http: true,
        ..SessionPolicy::default()
    };
    assert_eq!(
        policy.before_fetch(
            Mode::Http,
            &cleared_until(NOW + 1000),
            "other.test",
            None,
            NOW
        ),
        Transition::Escalate(EscalateReason::NoClearance)
    );
}

#[test]
fn a_site_that_needs_a_browser_escalates() {
    let policy = SessionPolicy::default();
    let needs_browser = site_preferring(Mode::Browser);
    assert_eq!(
        before(&policy, Mode::Http, &identity(), Some(&needs_browser)),
        Transition::Escalate(EscalateReason::SiteNeedsBrowser)
    );
    let works_over_http = site_preferring(Mode::Http);
    assert_eq!(
        before(&policy, Mode::Http, &identity(), Some(&works_over_http)),
        Transition::Stay
    );
}

#[test]
fn a_missing_required_clearance_is_named_before_the_site_history() {
    let policy = SessionPolicy {
        require_clearance_for_http: true,
        ..SessionPolicy::default()
    };
    let needs_browser = site_preferring(Mode::Browser);
    assert_eq!(
        before(&policy, Mode::Http, &identity(), Some(&needs_browser)),
        Transition::Escalate(EscalateReason::NoClearance)
    );
}

#[test]
fn a_valid_clearance_keeps_an_http_session_even_when_the_site_needs_a_browser() {
    for require in [false, true] {
        let policy = SessionPolicy {
            require_clearance_for_http: require,
            ..SessionPolicy::default()
        };
        let needs_browser = site_preferring(Mode::Browser);
        assert_eq!(
            before(
                &policy,
                Mode::Http,
                &cleared_until(NOW + 1000),
                Some(&needs_browser)
            ),
            Transition::Stay,
            "require = {require}"
        );
    }
}

#[test]
fn the_real_clearance_of_a_jar_keeps_an_http_session() {
    let policy = SessionPolicy::default();
    let made = identity_with_clearance();
    assert_eq!(before(&policy, Mode::Http, &made, None), Transition::Stay);
    // An hour from NOW the jar's cookie ends; 100 s before that it is expiring.
    assert_eq!(
        policy.before_fetch(Mode::Http, &made, "www.example.com", None, NOW + 3500),
        expiring(100)
    );
}

#[test]
fn the_margin_is_inclusive_at_120_seconds() {
    let policy = SessionPolicy::default();
    let at = |remaining: u64| before(&policy, Mode::Http, &cleared_until(NOW + remaining), None);
    assert_eq!(at(121), Transition::Stay);
    assert_eq!(at(120), expiring(120));
    assert_eq!(at(119), expiring(119));
    assert_eq!(at(1), expiring(1));
}

#[test]
fn an_expired_clearance_escalates_with_nothing_left() {
    let policy = SessionPolicy::default();
    assert_eq!(
        before(&policy, Mode::Http, &cleared_until(NOW), None),
        expiring(0),
        "it ends exactly now"
    );
    assert_eq!(
        before(&policy, Mode::Http, &cleared_until(NOW - 5), None),
        expiring(0)
    );
    assert_eq!(
        before(&policy, Mode::Http, &cleared_until(0), None),
        expiring(0)
    );
}

#[test]
fn a_zero_margin_escalates_only_once_the_clearance_has_ended() {
    let policy = SessionPolicy {
        clearance_margin: Duration::ZERO,
        ..SessionPolicy::default()
    };
    assert_eq!(
        before(&policy, Mode::Http, &cleared_until(NOW + 1), None),
        Transition::Stay
    );
    assert_eq!(
        before(&policy, Mode::Http, &cleared_until(NOW), None),
        expiring(0)
    );
}

#[test]
fn a_longer_margin_escalates_sooner_and_sub_seconds_are_dropped() {
    let long = SessionPolicy {
        clearance_margin: Duration::from_secs(600),
        ..SessionPolicy::default()
    };
    assert_eq!(
        before(&long, Mode::Http, &cleared_until(NOW + 500), None),
        expiring(500)
    );
    let fractional = SessionPolicy {
        clearance_margin: Duration::from_millis(1999),
        ..SessionPolicy::default()
    };
    assert_eq!(
        before(&fractional, Mode::Http, &cleared_until(NOW + 2), None),
        Transition::Stay,
        "1999 ms is read as one second"
    );
    assert_eq!(
        before(&fractional, Mode::Http, &cleared_until(NOW + 1), None),
        expiring(1)
    );
}

#[test]
fn a_clearance_with_no_known_end_never_expires() {
    let policy = SessionPolicy::default();
    assert_eq!(
        before(&policy, Mode::Http, &cleared_until(NO_EXPIRY), None),
        Transition::Stay
    );
    assert_eq!(
        policy.before_fetch(Mode::Http, &cleared_until(NO_EXPIRY), SITE, None, u64::MAX),
        expiring(0),
        "even a session cookie is spent at the end of time"
    );
}

#[test]
fn a_browser_session_never_leaves_before_a_fetch() {
    let policies = [
        SessionPolicy::default(),
        SessionPolicy {
            require_clearance_for_http: true,
            ..SessionPolicy::default()
        },
    ];
    let needs_browser = site_preferring(Mode::Browser);
    for policy in policies {
        for id in [identity(), cleared_until(NOW + 1000), cleared_until(NOW)] {
            assert_eq!(
                before(&policy, Mode::Browser, &id, Some(&needs_browser)),
                Transition::Stay
            );
        }
    }
}

#[test]
fn a_cooldown_is_left_to_the_caller() {
    let mut cooling = DomainState::new(SITE);
    cooling.start_cooldown(NOW + 1000);
    assert!(cooling.in_cooldown(NOW));
    assert_eq!(
        before(
            &SessionPolicy::default(),
            Mode::Http,
            &identity(),
            Some(&cooling)
        ),
        Transition::Stay
    );
}

#[test]
fn a_challenge_escalates_an_http_session_with_the_kind_met() {
    let policy = SessionPolicy::default();
    for kind in [
        ChallengeKind::JsChallenge,
        ChallengeKind::Interactive,
        ChallengeKind::Blocked,
    ] {
        assert_eq!(
            policy.after_fetch(Mode::Http, &seen(kind), &Action::Escalate, &identity()),
            Transition::Escalate(EscalateReason::Challenge(kind))
        );
    }
}

#[test]
fn every_other_action_leaves_an_http_session_alone() {
    let policy = SessionPolicy::default();
    let actions = [
        Action::Proceed,
        Action::Retry {
            after: Duration::from_secs(1),
        },
        Action::Solve,
        Action::RotateEgress,
        Action::RotateProfile,
        Action::Cooldown { until: NOW + 60 },
        Action::GiveUp(GiveUpReason::AttemptsExhausted),
    ];
    for action in actions {
        assert_eq!(
            policy.after_fetch(
                Mode::Http,
                &seen(ChallengeKind::JsChallenge),
                &action,
                &identity_with_clearance()
            ),
            Transition::Stay,
            "{action:?}"
        );
    }
}

#[test]
fn a_solve_in_the_browser_demotes_when_a_clearance_was_earned() {
    let policy = SessionPolicy::default();
    assert_eq!(
        policy.after_fetch(
            Mode::Browser,
            &seen(ChallengeKind::Interactive),
            &Action::Solve,
            &identity_with_clearance()
        ),
        Transition::Demote(DemoteReason::ChallengeSolved)
    );
}

#[test]
fn a_solve_does_not_demote_without_a_clearance_to_carry() {
    let policy = SessionPolicy::default();
    assert_eq!(
        policy.after_fetch(
            Mode::Browser,
            &seen(ChallengeKind::Interactive),
            &Action::Solve,
            &identity()
        ),
        Transition::Stay
    );
}

#[test]
fn a_solve_does_not_demote_when_the_policy_says_not_to() {
    let keep_browser = SessionPolicy {
        demote_after_solve: false,
        ..SessionPolicy::default()
    };
    let prefers_browser = SessionPolicy {
        prefer: Mode::Browser,
        ..SessionPolicy::default()
    };
    for policy in [keep_browser, prefers_browser] {
        assert_eq!(
            policy.after_fetch(
                Mode::Browser,
                &seen(ChallengeKind::JsChallenge),
                &Action::Solve,
                &identity_with_clearance()
            ),
            Transition::Stay,
            "{policy:?}"
        );
    }
}

#[test]
fn other_actions_in_the_browser_stay() {
    let policy = SessionPolicy::default();
    for action in [
        Action::Proceed,
        Action::Escalate,
        Action::RotateEgress,
        Action::RotateProfile,
        Action::Cooldown { until: NOW },
        Action::GiveUp(GiveUpReason::Blocked),
    ] {
        assert_eq!(
            policy.after_fetch(
                Mode::Browser,
                &seen(ChallengeKind::None),
                &action,
                &identity_with_clearance()
            ),
            Transition::Stay,
            "{action:?}"
        );
    }
}

#[test]
fn a_transition_names_the_mode_it_leads_to() {
    let escalate = Transition::Escalate(EscalateReason::Manual);
    let demote = Transition::Demote(DemoteReason::Manual);
    assert_eq!(escalate.resulting_mode(Mode::Http), Mode::Browser);
    assert_eq!(demote.resulting_mode(Mode::Browser), Mode::Http);
    assert_eq!(Transition::Stay.resulting_mode(Mode::Http), Mode::Http);
    assert_eq!(
        Transition::Stay.resulting_mode(Mode::Browser),
        Mode::Browser
    );
}

#[test]
fn a_transition_serializes_as_snake_case_json() {
    let json = |transition: Transition| serde_json::to_value(transition).expect("serializes");
    assert_eq!(json(Transition::Stay), serde_json::json!("stay"));
    assert_eq!(
        json(expiring(5)),
        serde_json::json!({"escalate": {"clearance_expiring": {"remaining_secs": 5}}})
    );
    assert_eq!(
        json(Transition::Escalate(EscalateReason::NoClearance)),
        serde_json::json!({"escalate": "no_clearance"})
    );
    assert_eq!(
        json(Transition::Demote(DemoteReason::ChallengeSolved)),
        serde_json::json!({"demote": "challenge_solved"})
    );
}
