//! The mitigation policy that turns a detection and the attempt context into one next `Action`.
//!
//! The policy is a pure function: it reads no clock (the caller passes `now`) and keeps no
//! state (the caller passes how many challenges in a row the site has shown). The decision
//! table, in the order it is applied:
//!
//! | Detection | Decision |
//! |---|---|
//! | [`ChallengeKind::None`] | [`Action::Proceed`] |
//! | any other, and `attempt >= max_challenge_attempts` | [`Action::GiveUp`] |
//! | [`ChallengeKind::RateLimited`] | [`Action::Cooldown`] for `Retry-After`, capped |
//! | [`ChallengeKind::Blocked`] | [`Action::RotateEgress`], or [`Action::Cooldown`] when it cannot |
//! | script or interactive challenge in [`Mode::Http`] | [`Action::Escalate`] to the browser |
//! | script or interactive challenge in [`Mode::Browser`] | [`Action::Solve`], then rotate |
//!
//! The rotation ladder rises with `consecutive_challenges`: at `rotate_egress_after` the egress
//! changes (if it can), at `rotate_profile_after` the browser profile changes.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::detect::{ChallengeKind, Detection};
use crate::utilities::retry::RetryPolicy;

/// How many attempts at one URL the default policy makes before giving up.
pub const DEFAULT_MAX_CHALLENGE_ATTEMPTS: u32 = 3;

/// Challenges in a row after which the default policy changes the egress.
pub const DEFAULT_ROTATE_EGRESS_AFTER: u32 = 2;

/// Challenges in a row after which the default policy changes the browser profile.
pub const DEFAULT_ROTATE_PROFILE_AFTER: u32 = 3;

/// Seconds the default policy leaves a site alone after an outright block.
pub const DEFAULT_BLOCK_COOLDOWN_SECS: u64 = 900;

/// Seconds to leave a site alone after a rate limit that named no `Retry-After`.
pub const DEFAULT_RATE_LIMIT_COOLDOWN_SECS: u64 = 60;

/// The longest `Retry-After` honored, in seconds. A server cannot park a scraper for longer.
pub const MAX_RETRY_AFTER_SECS: u64 = 3600;

/// How a request is being made.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Mode {
    /// Through a real browser.
    Browser,
    /// Through plain HTTP requests with an impersonated client.
    Http,
}

/// Where the fetch stands when a detection comes back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttemptContext {
    /// The attempt that produced the detection, counting from one.
    pub attempt: u32,
    /// How the request was made.
    pub mode: Mode,
    /// Challenges the site has shown in a row, this one included.
    pub consecutive_challenges: u32,
    /// Whether another egress is available.
    pub can_rotate_egress: bool,
    /// Whether a browser is available to take over from [`Mode::Http`].
    pub can_escalate: bool,
    /// The time now, in seconds since the Unix epoch.
    pub now: u64,
}

/// What to do next.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Action {
    /// The response is real; use it.
    Proceed,
    /// Try again after the wait.
    Retry {
        /// How long to wait first.
        after: Duration,
    },
    /// Hand the request to the browser.
    Escalate,
    /// Let the browser solve the challenge and wait for the clearance.
    Solve,
    /// Leave through another egress.
    RotateEgress,
    /// Present a new browser profile.
    RotateProfile,
    /// Leave the site alone until the time given, in seconds since the Unix epoch.
    Cooldown {
        /// When the site may be tried again.
        until: u64,
    },
    /// Stop; the request failed.
    GiveUp(GiveUpReason),
}

/// Why the policy stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum GiveUpReason {
    /// The attempts allowed are used.
    AttemptsExhausted,
    /// The attempts allowed are used, and the last answer was an outright block.
    Blocked,
    /// Only a different egress could help, and there is none.
    NoEgressLeft,
}

/// The thresholds of the decision table.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MitigationPolicy {
    /// Attempts at one URL before giving up. Zero is read as one.
    pub max_challenge_attempts: u32,
    /// The wait before a retry that has no better reason to wait.
    pub backoff: RetryPolicy,
    /// Challenges in a row after which the egress changes.
    pub rotate_egress_after: u32,
    /// Challenges in a row after which the browser profile changes.
    pub rotate_profile_after: u32,
    /// Seconds to leave a site alone after an outright block.
    pub block_cooldown_secs: u64,
}

impl Default for MitigationPolicy {
    fn default() -> Self {
        Self {
            max_challenge_attempts: DEFAULT_MAX_CHALLENGE_ATTEMPTS,
            backoff: RetryPolicy::network(),
            rotate_egress_after: DEFAULT_ROTATE_EGRESS_AFTER,
            rotate_profile_after: DEFAULT_ROTATE_PROFILE_AFTER,
            block_cooldown_secs: DEFAULT_BLOCK_COOLDOWN_SECS,
        }
    }
}

impl MitigationPolicy {
    /// Decides the next step for `detection` in `context`. See the module documentation for the
    /// table.
    #[must_use]
    pub fn decide(&self, detection: &Detection, context: &AttemptContext) -> Action {
        let kind = detection.kind;
        if kind == ChallengeKind::None {
            return Action::Proceed;
        }
        if context.attempt >= self.max_challenge_attempts.max(1) {
            return Action::GiveUp(if kind == ChallengeKind::Blocked {
                GiveUpReason::Blocked
            } else {
                GiveUpReason::AttemptsExhausted
            });
        }
        match kind {
            ChallengeKind::None => Action::Proceed,
            ChallengeKind::RateLimited { retry_after_secs } => {
                let wait = retry_after_secs.map_or(DEFAULT_RATE_LIMIT_COOLDOWN_SECS, |s| {
                    s.min(MAX_RETRY_AFTER_SECS)
                });
                Action::Cooldown {
                    until: context.now.saturating_add(wait),
                }
            }
            ChallengeKind::Blocked => {
                if context.can_rotate_egress {
                    Action::RotateEgress
                } else {
                    Action::Cooldown {
                        until: context.now.saturating_add(self.block_cooldown_secs),
                    }
                }
            }
            ChallengeKind::JsChallenge | ChallengeKind::Interactive => self.challenged(context),
        }
    }

    /// The decision for a script or interactive challenge.
    fn challenged(&self, context: &AttemptContext) -> Action {
        let streak = context.consecutive_challenges;
        match context.mode {
            Mode::Http if context.can_escalate => Action::Escalate,
            Mode::Http => {
                if streak >= self.rotate_profile_after {
                    Action::RotateProfile
                } else if streak >= self.rotate_egress_after {
                    if context.can_rotate_egress {
                        Action::RotateEgress
                    } else {
                        Action::GiveUp(GiveUpReason::NoEgressLeft)
                    }
                } else {
                    Action::Retry {
                        after: self.retry_delay(context.attempt),
                    }
                }
            }
            Mode::Browser => {
                if streak >= self.rotate_profile_after {
                    Action::RotateProfile
                } else if streak >= self.rotate_egress_after && context.can_rotate_egress {
                    Action::RotateEgress
                } else {
                    Action::Solve
                }
            }
        }
    }

    /// The wait before the attempt after `attempt`.
    fn retry_delay(&self, attempt: u32) -> Duration {
        let next = usize::try_from(attempt.saturating_add(1)).unwrap_or(usize::MAX);
        self.backoff.delay_for_attempt(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scrape::challenge::detect::Evidence;

    const NOW: u64 = 1_000_000;

    fn detection(kind: ChallengeKind) -> Detection {
        Detection {
            kind,
            evidence: Evidence::default(),
        }
    }

    fn context(mode: Mode) -> AttemptContext {
        AttemptContext {
            attempt: 1,
            mode,
            consecutive_challenges: 1,
            can_rotate_egress: true,
            can_escalate: true,
            now: NOW,
        }
    }

    fn decide(kind: ChallengeKind, context: &AttemptContext) -> Action {
        MitigationPolicy::default().decide(&detection(kind), context)
    }

    const JS: ChallengeKind = ChallengeKind::JsChallenge;

    #[test]
    fn the_defaults_are_the_named_constants() {
        let policy = MitigationPolicy::default();
        assert_eq!(policy.max_challenge_attempts, 3);
        assert_eq!(policy.rotate_egress_after, 2);
        assert_eq!(policy.rotate_profile_after, 3);
        assert_eq!(policy.block_cooldown_secs, 900);
        assert_eq!(policy.backoff, RetryPolicy::network());
        assert_eq!(
            policy.max_challenge_attempts,
            DEFAULT_MAX_CHALLENGE_ATTEMPTS
        );
        assert_eq!(policy.block_cooldown_secs, DEFAULT_BLOCK_COOLDOWN_SECS);
    }

    #[test]
    fn a_page_that_is_not_a_challenge_proceeds_whatever_the_attempt() {
        for attempt in [0, 1, 3, 100, u32::MAX] {
            let ctx = AttemptContext {
                attempt,
                ..context(Mode::Http)
            };
            assert_eq!(decide(ChallengeKind::None, &ctx), Action::Proceed);
        }
    }

    #[test]
    fn a_script_challenge_in_http_mode_escalates_to_the_browser() {
        assert_eq!(decide(JS, &context(Mode::Http)), Action::Escalate);
        assert_eq!(
            decide(ChallengeKind::Interactive, &context(Mode::Http)),
            Action::Escalate
        );
    }

    #[test]
    fn a_challenge_in_the_browser_is_solved_first() {
        let ctx = AttemptContext {
            consecutive_challenges: 1,
            ..context(Mode::Browser)
        };
        assert_eq!(decide(JS, &ctx), Action::Solve);
        assert_eq!(decide(ChallengeKind::Interactive, &ctx), Action::Solve);
        let first = AttemptContext {
            consecutive_challenges: 0,
            ..ctx
        };
        assert_eq!(decide(JS, &first), Action::Solve);
    }

    #[test]
    fn the_browser_ladder_rises_with_the_streak() {
        let at = |streak, can_rotate_egress| AttemptContext {
            consecutive_challenges: streak,
            can_rotate_egress,
            ..context(Mode::Browser)
        };
        assert_eq!(decide(JS, &at(2, true)), Action::RotateEgress);
        assert_eq!(
            decide(JS, &at(2, false)),
            Action::Solve,
            "nothing to rotate to"
        );
        assert_eq!(decide(JS, &at(3, true)), Action::RotateProfile);
        assert_eq!(decide(JS, &at(3, false)), Action::RotateProfile);
        assert_eq!(decide(JS, &at(40, true)), Action::RotateProfile);
        assert_eq!(decide(JS, &at(u32::MAX, true)), Action::RotateProfile);
    }

    #[test]
    fn http_without_a_browser_retries_then_rotates_then_has_nothing_left() {
        let at = |streak, can_rotate_egress| AttemptContext {
            consecutive_challenges: streak,
            can_rotate_egress,
            can_escalate: false,
            ..context(Mode::Http)
        };
        assert_eq!(
            decide(JS, &at(1, true)),
            Action::Retry {
                after: Duration::from_millis(500)
            },
            "the first retry waits the base delay of the network policy"
        );
        assert_eq!(decide(JS, &at(2, true)), Action::RotateEgress);
        assert_eq!(
            decide(JS, &at(2, false)),
            Action::GiveUp(GiveUpReason::NoEgressLeft)
        );
        assert_eq!(decide(JS, &at(3, false)), Action::RotateProfile);
    }

    #[test]
    fn the_retry_wait_grows_with_the_attempt_and_stops_at_the_cap() {
        let policy = MitigationPolicy {
            max_challenge_attempts: 10_000,
            ..MitigationPolicy::default()
        };
        let wait = |attempt| {
            let ctx = AttemptContext {
                attempt,
                consecutive_challenges: 1,
                can_escalate: false,
                ..context(Mode::Http)
            };
            match policy.decide(&detection(JS), &ctx) {
                Action::Retry { after } => after,
                other => panic!("expected a retry, got {other:?}"),
            }
        };
        assert_eq!(wait(1), Duration::from_millis(500));
        assert_eq!(wait(2), Duration::from_secs(1));
        assert_eq!(wait(3), Duration::from_secs(2));
        assert_eq!(
            wait(60),
            Duration::from_secs(10),
            "the cap of the network policy"
        );
        assert_eq!(wait(5_000), Duration::from_secs(10));
    }

    #[test]
    fn a_block_rotates_the_egress_or_leaves_the_site_alone() {
        let rotate = context(Mode::Http);
        assert_eq!(
            decide(ChallengeKind::Blocked, &rotate),
            Action::RotateEgress
        );
        let stuck = AttemptContext {
            can_rotate_egress: false,
            ..rotate
        };
        assert_eq!(
            decide(ChallengeKind::Blocked, &stuck),
            Action::Cooldown { until: NOW + 900 }
        );
        let in_browser = AttemptContext {
            mode: Mode::Browser,
            ..stuck
        };
        assert_eq!(
            decide(ChallengeKind::Blocked, &in_browser),
            Action::Cooldown { until: NOW + 900 }
        );
    }

    #[test]
    fn a_rate_limit_waits_for_retry_after_but_never_for_long() {
        let limited = |secs| ChallengeKind::RateLimited {
            retry_after_secs: secs,
        };
        let ctx = context(Mode::Http);
        assert_eq!(
            decide(limited(Some(30)), &ctx),
            Action::Cooldown { until: NOW + 30 }
        );
        assert_eq!(
            decide(limited(Some(0)), &ctx),
            Action::Cooldown { until: NOW }
        );
        assert_eq!(
            decide(limited(None), &ctx),
            Action::Cooldown {
                until: NOW + DEFAULT_RATE_LIMIT_COOLDOWN_SECS
            }
        );
        assert_eq!(
            decide(limited(Some(MAX_RETRY_AFTER_SECS + 1)), &ctx),
            Action::Cooldown {
                until: NOW + MAX_RETRY_AFTER_SECS
            }
        );
        assert_eq!(
            decide(limited(Some(u64::MAX)), &ctx),
            Action::Cooldown {
                until: NOW + MAX_RETRY_AFTER_SECS
            }
        );
    }

    #[test]
    fn a_cooldown_saturates_instead_of_wrapping() {
        let ctx = AttemptContext {
            now: u64::MAX - 5,
            ..context(Mode::Http)
        };
        assert_eq!(
            decide(
                ChallengeKind::RateLimited {
                    retry_after_secs: Some(100)
                },
                &ctx
            ),
            Action::Cooldown { until: u64::MAX }
        );
    }

    #[test]
    fn the_attempts_run_out() {
        for attempt in [3, 4, 1000, u32::MAX] {
            let ctx = AttemptContext {
                attempt,
                ..context(Mode::Browser)
            };
            assert_eq!(
                decide(JS, &ctx),
                Action::GiveUp(GiveUpReason::AttemptsExhausted)
            );
            assert_eq!(
                decide(ChallengeKind::Interactive, &ctx),
                Action::GiveUp(GiveUpReason::AttemptsExhausted)
            );
            assert_eq!(
                decide(
                    ChallengeKind::RateLimited {
                        retry_after_secs: Some(1)
                    },
                    &ctx
                ),
                Action::GiveUp(GiveUpReason::AttemptsExhausted)
            );
            assert_eq!(
                decide(ChallengeKind::Blocked, &ctx),
                Action::GiveUp(GiveUpReason::Blocked)
            );
        }
        let last = AttemptContext {
            attempt: 2,
            ..context(Mode::Browser)
        };
        assert_eq!(
            decide(JS, &last),
            Action::Solve,
            "the third attempt is still allowed"
        );
    }

    #[test]
    fn zero_attempts_allowed_is_read_as_one() {
        let policy = MitigationPolicy {
            max_challenge_attempts: 0,
            ..MitigationPolicy::default()
        };
        let ctx = context(Mode::Browser);
        assert_eq!(
            policy.decide(&detection(JS), &ctx),
            Action::GiveUp(GiveUpReason::AttemptsExhausted)
        );
        assert_eq!(
            policy.decide(&detection(ChallengeKind::None), &ctx),
            Action::Proceed
        );
    }

    #[test]
    fn the_thresholds_are_the_policys_own() {
        let policy = MitigationPolicy {
            rotate_egress_after: 1,
            rotate_profile_after: 5,
            block_cooldown_secs: 10,
            ..MitigationPolicy::default()
        };
        let at = |streak| AttemptContext {
            consecutive_challenges: streak,
            ..context(Mode::Browser)
        };
        assert_eq!(policy.decide(&detection(JS), &at(1)), Action::RotateEgress);
        assert_eq!(policy.decide(&detection(JS), &at(4)), Action::RotateEgress);
        assert_eq!(policy.decide(&detection(JS), &at(5)), Action::RotateProfile);
        let stuck = AttemptContext {
            can_rotate_egress: false,
            ..context(Mode::Browser)
        };
        assert_eq!(
            policy.decide(&detection(ChallengeKind::Blocked), &stuck),
            Action::Cooldown { until: NOW + 10 }
        );
    }

    #[test]
    fn evidence_does_not_change_the_decision() {
        let with_evidence = Detection {
            kind: JS,
            evidence: Evidence {
                vendor: Some(crate::scrape::challenge::detect::Vendor::Cloudflare),
                markers: vec!["cf_chl_opt"],
                widget: None,
            },
        };
        let ctx = context(Mode::Http);
        assert_eq!(
            MitigationPolicy::default().decide(&with_evidence, &ctx),
            decide(JS, &ctx)
        );
    }

    #[test]
    fn every_input_gets_an_answer_that_keeps_its_promises() {
        let kinds = [
            ChallengeKind::None,
            JS,
            ChallengeKind::Interactive,
            ChallengeKind::Blocked,
            ChallengeKind::RateLimited {
                retry_after_secs: None,
            },
            ChallengeKind::RateLimited {
                retry_after_secs: Some(u64::MAX),
            },
        ];
        let policy = MitigationPolicy::default();
        for kind in kinds {
            for mode in [Mode::Browser, Mode::Http] {
                for (can_rotate_egress, can_escalate) in
                    [(false, false), (false, true), (true, false), (true, true)]
                {
                    for attempt in 0..6 {
                        for streak in 0..6 {
                            let ctx = AttemptContext {
                                attempt,
                                mode,
                                consecutive_challenges: streak,
                                can_rotate_egress,
                                can_escalate,
                                now: NOW,
                            };
                            let action = policy.decide(&detection(kind), &ctx);
                            let note = format!("{kind:?} {ctx:?} -> {action:?}");
                            assert_eq!(
                                action == Action::Proceed,
                                kind == ChallengeKind::None,
                                "{note}"
                            );
                            if action == Action::RotateEgress {
                                assert!(can_rotate_egress, "{note}");
                            }
                            if action == Action::Escalate {
                                assert!(can_escalate && mode == Mode::Http, "{note}");
                            }
                            if action == Action::Solve {
                                assert_eq!(mode, Mode::Browser, "{note}");
                            }
                            if attempt >= 3 && kind != ChallengeKind::None {
                                assert!(matches!(action, Action::GiveUp(_)), "{note}");
                            }
                            if let Action::Cooldown { until } = action {
                                assert!(until >= NOW, "{note}");
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn actions_and_modes_survive_serde() {
        let actions = [
            Action::Proceed,
            Action::Retry {
                after: Duration::from_millis(1500),
            },
            Action::Escalate,
            Action::Solve,
            Action::RotateEgress,
            Action::RotateProfile,
            Action::Cooldown { until: 42 },
            Action::GiveUp(GiveUpReason::NoEgressLeft),
        ];
        for action in actions {
            let json = serde_json::to_string(&action).expect("serializes");
            assert_eq!(
                serde_json::from_str::<Action>(&json).expect("parses"),
                action
            );
        }
        for mode in [Mode::Browser, Mode::Http] {
            let json = serde_json::to_string(&mode).expect("serializes");
            assert_eq!(serde_json::from_str::<Mode>(&json).expect("parses"), mode);
        }
    }
}
