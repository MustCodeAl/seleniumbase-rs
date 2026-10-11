//! The fetch loop of the session: load, detect, decide, act, and load again, in whichever mode the
//! session is in.

use std::fmt;
use std::time::Instant;

use super::{Leg, StealthError, StealthSession};
use crate::impersonate::http_session::{Attempt, Fetched, HttpReply, HttpSession};
use crate::scrape::challenge::detect::{ChallengeKind, Detection};
use crate::scrape::challenge::mitigation::{Action, AttemptContext, GiveUpReason, Mode};
use crate::scrape::egress::pool::EgressOutcome;
use crate::scrape::egress::redact::RedactedUrl;
use crate::scrape::egress::route::EgressKey;
use crate::scrape::error::ScrapeError;
use crate::scrape::jar::store::site_of;
use crate::scrape::ledger::events::ScraperEvent;
use crate::scrape::ledger::state::DomainState;
use crate::scrape::passport::policy::{EscalateReason, Transition};
use crate::scrape::runtime::cdp::{NavResponse, PageSnapshot};
use crate::utils::urls::normalize_page_url;

/// Why a URL is refused without echoing it.
const INVALID_URL: &str = "the URL is not valid";

/// Why a URL with no host is refused.
const NO_HOST: &str = "the URL has no host";

/// A page fetched by a [`StealthSession`](super::StealthSession), by whichever leg got it.
#[derive(Clone)]
pub struct StealthFetch {
    /// The URL the answer came from, after redirects.
    pub url: String,
    /// The HTTP status of the answer.
    pub status: u16,
    /// The response headers as (name, value) pairs.
    pub headers: Vec<(String, String)>,
    /// The page: the body of the response in HTTP mode, the rendered page in browser mode.
    pub body: String,
    /// What detection found. The kind is [`ChallengeKind::None`] for a page returned by a fetch.
    pub detection: Detection,
    /// The mode the page was got in. A page that took a challenge to reach is usually an HTTP
    /// page: the browser earned the clearance and the session went back to HTTP to use it.
    pub mode: Mode,
    /// How many loads the fetch took, counting from one.
    pub attempts: u32,
    /// The challenge that was solved on the way, if one was.
    pub solved: Option<ChallengeKind>,
    /// The egress the page came through; `None` for the direct connection.
    pub egress: Option<EgressKey>,
}

impl fmt::Debug for StealthFetch {
    /// The URL redacted, the body and headers counted: either can hold a credential.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StealthFetch")
            .field("url", &RedactedUrl::new(&self.url))
            .field("status", &self.status)
            .field("headers", &self.headers.len())
            .field("body_bytes", &self.body.len())
            .field("detection", &self.detection.kind)
            .field("mode", &self.mode)
            .field("attempts", &self.attempts)
            .field("solved", &self.solved)
            .field("egress", &self.egress)
            .finish()
    }
}

impl StealthFetch {
    /// A page got over HTTP. A body that is not UTF-8 is read lossily: the bytes of a page that
    /// is not text are not what a fetch is for.
    fn from_reply(reply: &HttpReply, detection: Detection, egress: Option<EgressKey>) -> Self {
        Self {
            url: reply.url().as_str().to_owned(),
            status: reply.status(),
            headers: reply.headers().as_pairs().to_vec(),
            body: String::from_utf8_lossy(reply.bytes()).into_owned(),
            detection,
            mode: Mode::Http,
            attempts: 0,
            solved: None,
            egress,
        }
    }

    /// A page got through the browser.
    fn from_page(response: NavResponse, snapshot: PageSnapshot, egress: Option<EgressKey>) -> Self {
        Self {
            url: response.url,
            status: response.status,
            headers: response.headers,
            body: snapshot.body,
            detection: snapshot.detection,
            mode: Mode::Browser,
            attempts: 0,
            solved: None,
            egress,
        }
    }
}

/// A page to fetch: the URL both legs are given, the site it belongs to, and its redacted form.
pub(super) struct Target {
    pub(super) url: String,
    pub(super) site: String,
    redacted: RedactedUrl,
}

impl Target {
    /// Reads `raw` the way [`Page::goto`](crate::sb_cdp::Page::goto) does, so a bare host gets
    /// `https://`, and finds the registrable domain of its host.
    pub(super) fn parse(raw: &str) -> Result<Self, ScrapeError> {
        let refused = |reason: &str| ScrapeError::Navigation {
            url: RedactedUrl::new(raw),
            reason: reason.to_owned(),
        };
        let url = normalize_page_url(raw).map_err(|_error| refused(INVALID_URL))?;
        let parsed = url::Url::parse(&url).map_err(|_error| refused(INVALID_URL))?;
        let host = parsed.host_str().ok_or_else(|| refused(NO_HOST))?;
        let site = site_of(host).unwrap_or_else(|| host.to_ascii_lowercase());
        Ok(Self {
            redacted: RedactedUrl::new(&url),
            url,
            site,
        })
    }
}

/// What a challenge solved on the way is remembered by.
struct Solved {
    kind: ChallengeKind,
    since: Instant,
}

/// How far a fetch has got.
#[derive(Default)]
struct Progress {
    /// The load now under way, counting from one.
    attempt: u32,
    solved: Option<Solved>,
}

/// One load and what the policies made of it.
struct Loaded {
    fetch: StealthFetch,
    action: Action,
    /// What the session policy said about the load: only an HTTP verdict carries one.
    transition: Transition,
}

/// What a loop does after an action was carried out.
enum Step {
    /// The page is real; return it.
    Finish(Box<StealthFetch>),
    /// Load the page again, in whatever mode the session is in now.
    Again,
}

/// What `load_once` takes out of the leg before it lets go of it.
enum Raw {
    Http(HttpSession),
    Browser(Result<(NavResponse, PageSnapshot), ScrapeError>),
}

impl StealthSession {
    /// Fetches `url` in whichever mode the session is in, moving between HTTP and the browser as
    /// the policies decide. See the [module documentation](super) for the loop.
    ///
    /// A site in cooldown is not requested at all. The state of the site (successes, challenges,
    /// cooldown) is read before the first load and saved after the last, whether the fetch worked
    /// or not.
    ///
    /// # Errors
    ///
    /// - [`ScrapeError::CoolingDown`] when the site is to be left alone, before or because of this
    ///   fetch;
    /// - [`ScrapeError::Unsolved`] when a challenge was still there after the attempts allowed;
    /// - [`ScrapeError::GaveUp`] when the policy stopped for another reason;
    /// - [`ScrapeError::Navigation`] when the URL is not valid or the browser could not load it;
    /// - [`StealthError::Http`] when an HTTP request failed, and [`StealthError::Mitm`],
    ///   [`StealthError::Impersonate`] and [`StealthError::Scrape`] when a change of mode, a
    ///   rotation, the store or the browser failed.
    #[tracing::instrument(skip_all, fields(url = %RedactedUrl::new(url)))]
    pub async fn fetch(&mut self, url: &str) -> Result<StealthFetch, StealthError> {
        let target = Target::parse(url)?;
        if let Some(limits) = &self.wiring.limits {
            limits.acquire(&target.site).await;
        }
        let mut state = self
            .wiring
            .store
            .get(&target.site)
            .await?
            .unwrap_or_else(|| DomainState::new(&target.site));
        let now = self.now();
        if let Some(until) = state.cooldown_until.filter(|until| now < *until) {
            return Err(ScrapeError::CoolingDown {
                site: target.site,
                until,
            }
            .into());
        }
        self.egress.site = Some(target.site.clone());
        let outcome = self.attempts(&target, &mut state).await;
        let saved = self.wiring.store.put(state).await;
        let fetched = outcome?;
        saved?;
        Ok(fetched)
    }

    /// Loads, decides and acts until the page is real or the policy stops.
    async fn attempts(
        &mut self,
        target: &Target,
        state: &mut DomainState,
    ) -> Result<StealthFetch, StealthError> {
        self.preflight(target, state).await?;
        let mut progress = Progress::default();
        loop {
            progress.attempt = progress.attempt.saturating_add(1);
            let loaded = self.load_once(target, state, progress.attempt).await?;
            match self.step(loaded, target, state, &mut progress).await? {
                Step::Finish(fetched) => return Ok(*fetched),
                Step::Again => {}
            }
        }
    }

    /// Asks the session policy, before the request, whether the browser should take it.
    async fn preflight(
        &mut self,
        target: &Target,
        state: &DomainState,
    ) -> Result<(), StealthError> {
        let transition = match &self.leg {
            Leg::Http(http) => {
                http.preflight(&target.url, &self.identity, Some(state), &self.wiring.rules)?
            }
            Leg::Browser(_) => Transition::Stay,
        };
        if matches!(transition, Transition::Escalate(_)) {
            self.switch_to(Mode::Browser, transition).await?;
        }
        Ok(())
    }

    /// One load by the leg in use, judged by the policies.
    async fn load_once(
        &mut self,
        target: &Target,
        state: &mut DomainState,
        attempt: u32,
    ) -> Result<Loaded, StealthError> {
        let raw = match &mut self.leg {
            Leg::Http(http) => Raw::Http(http.clone()),
            Leg::Browser(leg) => Raw::Browser(leg.load(&target.url).await),
        };
        match raw {
            Raw::Http(http) => self.http_load(&http, target, state, attempt).await,
            Raw::Browser(loaded) => self.browser_judge(loaded, target, state, attempt),
        }
    }

    /// One request over HTTP. The HTTP session detects the challenge, asks the mitigation policy
    /// and the session policy, and reports the request and the challenge to the sink.
    async fn http_load(
        &mut self,
        http: &HttpSession,
        target: &Target,
        state: &mut DomainState,
        attempt: u32,
    ) -> Result<Loaded, StealthError> {
        let context = Attempt {
            number: attempt,
            previous_challenges: state.consecutive_challenges,
            can_rotate_egress: self.egress.size > 1,
            can_escalate: true,
        };
        let fetched = http
            .fetch(&target.url, &context, &self.wiring.rules, &self.identity)
            .await;
        let Fetched { reply, verdict } = match fetched {
            Ok(fetched) => fetched,
            Err(error) => {
                self.report(EgressOutcome::ConnectError);
                return Err(error.into());
            }
        };
        state.record(&verdict.detection, Mode::Http, self.now());
        state.last_egress = self.egress_key().cloned();
        self.report(outcome_of(verdict.detection.kind));
        Ok(Loaded {
            fetch: StealthFetch::from_reply(
                &reply,
                verdict.detection.clone(),
                self.egress_key().cloned(),
            ),
            action: verdict.action,
            transition: verdict.transition,
        })
    }

    /// Judges a page the browser loaded. The requests themselves are reported by the MITM proxy,
    /// which sees every one the browser makes.
    fn browser_judge(
        &mut self,
        loaded: Result<(NavResponse, PageSnapshot), ScrapeError>,
        target: &Target,
        state: &mut DomainState,
        attempt: u32,
    ) -> Result<Loaded, StealthError> {
        let (response, snapshot) = match loaded {
            Ok(page) => page,
            Err(error) => {
                self.report(EgressOutcome::ConnectError);
                return Err(error.into());
            }
        };
        let detection = &snapshot.detection;
        state.record(detection, Mode::Browser, self.now());
        state.last_egress = self.egress_key().cloned();
        if detection.kind != ChallengeKind::None {
            self.emit(&ScraperEvent::ChallengeDetected {
                url: target.redacted.clone(),
                kind: detection.kind,
                vendor: detection.evidence.vendor,
                mode: Mode::Browser,
            });
        }
        self.report(outcome_of(detection.kind));
        let action = self.decide_in_browser(target, state, detection, attempt);
        Ok(Loaded {
            fetch: StealthFetch::from_page(response, snapshot, self.egress_key().cloned()),
            action,
            transition: Transition::Stay,
        })
    }

    /// Asks the mitigation policy what to do about a page the browser loaded, and says so.
    fn decide_in_browser(
        &self,
        target: &Target,
        state: &DomainState,
        detection: &Detection,
        attempt: u32,
    ) -> Action {
        let context = AttemptContext {
            attempt,
            mode: Mode::Browser,
            consecutive_challenges: state.consecutive_challenges,
            can_rotate_egress: self.egress.size > 1,
            can_escalate: false,
            now: self.now(),
        };
        let action = self.wiring.rules.mitigation.decide(detection, &context);
        self.emit(&ScraperEvent::ActionChosen {
            site: target.site.clone(),
            action,
        });
        action
    }

    /// Carries out the action chosen for a load.
    async fn step(
        &mut self,
        loaded: Loaded,
        target: &Target,
        state: &mut DomainState,
        progress: &mut Progress,
    ) -> Result<Step, StealthError> {
        let Loaded {
            fetch,
            action,
            transition,
        } = loaded;
        let detection = &fetch.detection;
        let attempt = progress.attempt;
        match action {
            Action::Proceed => {
                return Ok(Step::Finish(Box::new(
                    self.finished(target, fetch, progress),
                )));
            }
            Action::Solve => self.solve(detection, progress).await?,
            Action::Escalate => self.hand_over(state, detection, transition).await?,
            Action::Retry { after } => tokio::time::sleep(after).await,
            Action::RotateEgress => self.rotate_or_give_up(target, detection, attempt).await?,
            Action::RotateProfile => self.rotate_profile().await?,
            Action::Cooldown { until } => return Err(self.cool_down(target, state, until)),
            Action::GiveUp(reason) => {
                return Err(give_up(target, detection, reason, attempt).into());
            }
        }
        Ok(Step::Again)
    }

    /// Solves the challenge in the browser, moves the cookies it earned into the identity, and
    /// goes back to HTTP when the session policy says the identity can carry on from there.
    async fn solve(
        &mut self,
        detection: &Detection,
        progress: &mut Progress,
    ) -> Result<(), StealthError> {
        progress.solved.get_or_insert_with(|| Solved {
            kind: detection.kind,
            since: Instant::now(),
        });
        let (seed, timeout) = (self.persona.seed, self.wiring.solve_timeout);
        let Leg::Browser(leg) = &self.leg else {
            return Ok(());
        };
        leg.solve(detection, seed, timeout).await?;
        let identity = self.synced_identity().await?;
        let transition = self.wiring.rules.session.after_fetch(
            Mode::Browser,
            detection,
            &Action::Solve,
            &identity,
        );
        if matches!(transition, Transition::Demote(_)) {
            self.move_to(identity, Mode::Http, transition).await?;
        }
        Ok(())
    }

    /// Hands the request to the browser after a challenge met over HTTP.
    ///
    /// The challenge streak starts again: the hand-over is not a failure the rotation ladder
    /// should count against the browser.
    async fn hand_over(
        &mut self,
        state: &mut DomainState,
        detection: &Detection,
        transition: Transition,
    ) -> Result<(), StealthError> {
        if self.mode() == Mode::Browser {
            return Ok(());
        }
        let reason = match transition {
            Transition::Escalate(reason) => reason,
            _ => EscalateReason::Challenge(detection.kind),
        };
        state.consecutive_challenges = 0;
        self.switch_to(Mode::Browser, Transition::Escalate(reason))
            .await
    }

    /// Rotates the egress; with none left to rotate to, the fetch gives up.
    async fn rotate_or_give_up(
        &mut self,
        target: &Target,
        detection: &Detection,
        attempt: u32,
    ) -> Result<(), StealthError> {
        match self.rotate_egress().await {
            Err(StealthError::Scrape(ScrapeError::NoEgressAvailable)) => {
                Err(give_up(target, detection, GiveUpReason::NoEgressLeft, attempt).into())
            }
            other => other,
        }
    }

    /// Leaves the site alone until `until`, and says so.
    fn cool_down(&self, target: &Target, state: &mut DomainState, until: u64) -> StealthError {
        state.start_cooldown(until);
        self.emit(&ScraperEvent::CooldownStarted {
            site: target.site.clone(),
            until,
        });
        ScrapeError::CoolingDown {
            site: target.site.clone(),
            until,
        }
        .into()
    }

    /// The result of a fetch that ended on a real page.
    fn finished(
        &self,
        target: &Target,
        mut fetch: StealthFetch,
        progress: &Progress,
    ) -> StealthFetch {
        if let Some(solved) = &progress.solved {
            self.emit(&ScraperEvent::ChallengeSolved {
                url: target.redacted.clone(),
                kind: solved.kind,
                attempts: progress.attempt,
                elapsed_ms: elapsed_ms(solved.since),
            });
        }
        fetch.attempts = progress.attempt;
        fetch.solved = progress.solved.as_ref().map(|solved| solved.kind);
        fetch
    }

    /// Tells the pool how the egress in use fared.
    fn report(&self, outcome: EgressOutcome) {
        if let (Some(pool), Some(current)) = (&self.egress.pool, &self.egress.current) {
            pool.report(current.key(), outcome, self.now());
        }
    }
}

/// How the egress fared, by what the site answered.
fn outcome_of(kind: ChallengeKind) -> EgressOutcome {
    match kind {
        ChallengeKind::None => EgressOutcome::Success,
        ChallengeKind::Blocked => EgressOutcome::Blocked,
        _ => EgressOutcome::Challenged,
    }
}

/// The error for a policy that stopped: an unpassed challenge is [`ScrapeError::Unsolved`].
fn give_up(
    target: &Target,
    detection: &Detection,
    reason: GiveUpReason,
    attempts: u32,
) -> ScrapeError {
    let passable = matches!(
        detection.kind,
        ChallengeKind::JsChallenge | ChallengeKind::Interactive
    );
    if reason == GiveUpReason::AttemptsExhausted && passable {
        return ScrapeError::Unsolved {
            url: target.redacted.clone(),
            kind: detection.kind,
            attempts,
        };
    }
    ScrapeError::GaveUp {
        url: target.redacted.clone(),
        reason,
    }
}

/// Milliseconds since `started`, saturating.
fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scrape::challenge::detect::Evidence;

    const SECRET_URL: &str = "https://alice:s3cret@www.shop.co.uk/cart?token=hunter2&page=2";

    fn detection(kind: ChallengeKind) -> Detection {
        Detection {
            kind,
            evidence: Evidence::default(),
        }
    }

    fn target() -> Target {
        Target::parse(SECRET_URL).expect("a target")
    }

    #[test]
    fn a_bare_host_gets_https_and_its_site_is_the_registrable_domain() {
        let target = Target::parse("www.shop.co.uk/cart").expect("a target");
        assert_eq!(target.url, "https://www.shop.co.uk/cart");
        assert_eq!(target.site, "shop.co.uk");
    }

    #[test]
    fn an_address_is_its_own_site() {
        let target = Target::parse("http://127.0.0.1:8080/x").expect("a target");
        assert_eq!(target.site, "127.0.0.1");
    }

    #[test]
    fn a_url_that_cannot_be_fetched_is_refused_without_echoing_it() {
        for raw in [
            "",
            "https://",
            "https://alice:s3cret@/cart?token=hunter2",
            "ht tp://alice:s3cret@shop.example/",
        ] {
            let error = Target::parse(raw)
                .err()
                .unwrap_or_else(|| panic!("{raw:?} was fetched"));
            assert!(matches!(error, ScrapeError::Navigation { .. }), "{error:?}");
            let shown = format!("{error} | {error:?}");
            for secret in ["s3cret", "hunter2", "alice"] {
                assert!(!shown.contains(secret), "{secret} in {shown}");
            }
        }
    }

    #[test]
    fn a_passable_challenge_that_ran_out_of_attempts_is_unsolved() {
        for kind in [ChallengeKind::JsChallenge, ChallengeKind::Interactive] {
            let error = give_up(
                &target(),
                &detection(kind),
                GiveUpReason::AttemptsExhausted,
                4,
            );
            assert!(
                matches!(error, ScrapeError::Unsolved { kind: found, attempts: 4, .. } if found == kind),
                "{error:?}"
            );
        }
    }

    #[test]
    fn every_other_way_to_stop_is_a_give_up_with_its_reason() {
        let cases = [
            (ChallengeKind::Blocked, GiveUpReason::Blocked),
            (ChallengeKind::JsChallenge, GiveUpReason::NoEgressLeft),
            (ChallengeKind::Blocked, GiveUpReason::AttemptsExhausted),
        ];
        for (kind, reason) in cases {
            let error = give_up(&target(), &detection(kind), reason, 2);
            assert!(
                matches!(error, ScrapeError::GaveUp { reason: found, .. } if found == reason),
                "{error:?}"
            );
        }
    }

    #[test]
    fn the_egress_fares_by_what_the_site_answered() {
        assert_eq!(outcome_of(ChallengeKind::None), EgressOutcome::Success);
        assert_eq!(outcome_of(ChallengeKind::Blocked), EgressOutcome::Blocked);
        for kind in [
            ChallengeKind::JsChallenge,
            ChallengeKind::Interactive,
            ChallengeKind::RateLimited {
                retry_after_secs: None,
            },
        ] {
            assert_eq!(outcome_of(kind), EgressOutcome::Challenged);
        }
    }

    #[test]
    fn a_fetch_shows_its_url_redacted_and_counts_what_may_hold_a_credential() {
        let fetch = StealthFetch {
            url: SECRET_URL.to_owned(),
            status: 200,
            headers: vec![("set-cookie".to_owned(), "cf_clearance=s3cret".to_owned())],
            body: "<p>hunter2</p>".to_owned(),
            detection: detection(ChallengeKind::None),
            mode: Mode::Http,
            attempts: 1,
            solved: None,
            egress: None,
        };
        let shown = format!("{fetch:?}");
        for secret in ["alice", "s3cret", "hunter2"] {
            assert!(!shown.contains(secret), "{secret} in {shown}");
        }
        assert!(
            shown.contains("page=2") && shown.contains("body_bytes: 14"),
            "{shown}"
        );
    }

    #[test]
    fn the_time_a_challenge_took_saturates_instead_of_wrapping() {
        assert!(elapsed_ms(Instant::now()) < 1000);
    }
}
