//! The fetch loop of the browser scraper: load, detect, decide, act, and load again.

use std::time::Instant;

use super::{BrowserFetch, BrowserScraper};
use crate::scrape::challenge::detect::{ChallengeKind, Detection};
use crate::scrape::challenge::mitigation::{Action, AttemptContext, GiveUpReason, Mode};
use crate::scrape::egress::pool::EgressOutcome;
use crate::scrape::egress::redact::RedactedUrl;
use crate::scrape::error::ScrapeError;
use crate::scrape::jar::store::site_of;
use crate::scrape::ledger::events::ScraperEvent;
use crate::scrape::ledger::state::DomainState;
use crate::scrape::runtime::cdp::{self, NavResponse, PageSnapshot};
use crate::utils::urls::normalize_page_url;

/// Why a URL is refused without echoing it.
const INVALID_URL: &str = "the URL is not valid";

/// Why a URL with no host is refused.
const NO_HOST: &str = "the URL has no host";

/// A page to fetch: the URL the browser is given, the site it belongs to, and its redacted form.
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

/// What to do after an action was carried out.
enum Step {
    /// The page is real; return it.
    Finish,
    /// Load the page again.
    Again,
}

/// What a loop knows of a challenge it solved.
struct Solved {
    kind: ChallengeKind,
    since: Instant,
}

impl BrowserScraper {
    /// Fetches `url` through the browser, solving challenges on the way.
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
    /// - [`ScrapeError::Navigation`] when the browser could not load the page;
    /// - [`ScrapeError::State`], [`ScrapeError::Geo`] and [`ScrapeError::Browser`] when the store,
    ///   a locale or the browser fails.
    #[tracing::instrument(skip_all, fields(url = %RedactedUrl::new(url)))]
    pub async fn fetch(&mut self, url: &str) -> Result<BrowserFetch, ScrapeError> {
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
            });
        }
        self.egress.site = Some(target.site.clone());
        let outcome = self.attempts(&target, &mut state).await;
        let saved = self.wiring.store.put(state).await;
        let fetched = outcome?;
        saved?;
        Ok(fetched)
    }

    /// Reads the page now open, solves the challenge on it if there is one, and waits for the
    /// browser to show the clearance. Returns what the page is afterwards.
    ///
    /// The status and headers used for detection are those of the navigation that led to the page
    /// ([`fetch`](Self::fetch) loads it). A page that is not a challenge is returned as it is.
    ///
    /// A challenge with a widget is attempted with [`Page::solve_captcha`](crate::sb_cdp::Page),
    /// after the pointer has arrived on the page like a person's. A script challenge needs no
    /// click: it is waited for. The wait ends when the vendor's clearance cookie holds a new value
    /// or, for a vendor without one, when the page stops being a challenge, and is bounded by the
    /// solve timeout of the builder.
    ///
    /// # Errors
    ///
    /// Returns [`ScrapeError::Browser`] when the browser cannot be read or refuses the click.
    #[tracing::instrument(skip_all)]
    pub async fn solve_challenge(&mut self) -> Result<Detection, ScrapeError> {
        let before = self.read_current().await?;
        if before.detection.kind == ChallengeKind::None {
            return Ok(before.detection);
        }
        let vendor = before.detection.evidence.vendor;
        let stale = cdp::clearance_value(&self.page, vendor).await?;
        let has_widget = before.detection.evidence.widget.is_some();
        if has_widget || before.detection.kind == ChallengeKind::Interactive {
            cdp::human_warm_up(&self.page, self.persona.seed).await;
            self.page.solve_captcha().await?;
        }
        cdp::wait_for_clearance(
            &self.page,
            vendor,
            stale.as_deref(),
            self.wiring.solve_timeout,
        )
        .await?;
        Ok(self.read_current().await?.detection)
    }

    /// The page now open, judged with the answer that led to it.
    async fn read_current(&self) -> Result<PageSnapshot, ScrapeError> {
        let response = match &self.last {
            Some(response) => response.clone(),
            None => NavResponse {
                url: self.page.url().await?,
                status: 0,
                headers: Vec::new(),
            },
        };
        cdp::detect_page(&self.page, &response).await
    }

    /// Loads, decides and acts until the page is real or the policy stops.
    async fn attempts(
        &mut self,
        target: &Target,
        state: &mut DomainState,
    ) -> Result<BrowserFetch, ScrapeError> {
        let mut solved: Option<Solved> = None;
        let mut attempt = 0_u32;
        loop {
            attempt = attempt.saturating_add(1);
            let (response, snapshot) = self.load_once(target, state).await?;
            let action = self.decide(target, state, &snapshot.detection, attempt);
            if action == Action::Solve && solved.is_none() {
                solved = Some(Solved {
                    kind: snapshot.detection.kind,
                    since: Instant::now(),
                });
            }
            match self.act(action, target, state, &snapshot, attempt).await? {
                Step::Finish => {
                    return Ok(self.finished(target, response, snapshot, attempt, solved));
                }
                Step::Again => {}
            }
        }
    }

    /// One load: navigates, reads the page, and records what happened.
    async fn load_once(
        &mut self,
        target: &Target,
        state: &mut DomainState,
    ) -> Result<(NavResponse, PageSnapshot), ScrapeError> {
        let started = Instant::now();
        let response = match cdp::goto_with_response(&self.page, &target.url).await {
            Ok(response) => response,
            Err(error) => {
                self.report(EgressOutcome::ConnectError);
                return Err(error);
            }
        };
        let snapshot = cdp::detect_page(&self.page, &response).await?;
        let detection = &snapshot.detection;
        state.record(detection, Mode::Browser, self.now());
        state.last_egress = self.current_key();
        self.emit(&ScraperEvent::Request {
            url: target.redacted.clone(),
            status: response.status,
            mode: Mode::Browser,
            elapsed_ms: elapsed_ms(started),
        });
        if detection.kind != ChallengeKind::None {
            self.emit(&ScraperEvent::ChallengeDetected {
                url: target.redacted.clone(),
                kind: detection.kind,
                vendor: detection.evidence.vendor,
                mode: Mode::Browser,
            });
        }
        self.report(outcome_of(detection.kind));
        self.last = Some(response.clone());
        Ok((response, snapshot))
    }

    /// Tells the pool how the egress in use fared.
    fn report(&self, outcome: EgressOutcome) {
        if let (Some(pool), Some(current)) = (&self.egress.pool, &self.egress.current) {
            pool.report(current.egress.key(), outcome, self.now());
        }
    }

    /// Asks the mitigation policy what to do, and reports the answer.
    fn decide(
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
        let action = self.wiring.policy.decide(detection, &context);
        self.emit(&ScraperEvent::ActionChosen {
            site: target.site.clone(),
            action,
        });
        action
    }

    /// Carries out `action`.
    async fn act(
        &mut self,
        action: Action,
        target: &Target,
        state: &mut DomainState,
        snapshot: &PageSnapshot,
        attempt: u32,
    ) -> Result<Step, ScrapeError> {
        match action {
            Action::Proceed => return Ok(Step::Finish),
            Action::Solve => {
                self.solve_challenge().await?;
            }
            Action::Retry { after } => tokio::time::sleep(after).await,
            Action::RotateEgress => {
                self.rotate_or_give_up(target, &snapshot.detection, attempt)
                    .await?;
            }
            Action::RotateProfile => self.rotate_profile().await?,
            Action::Cooldown { until } => return Err(self.cool_down(target, state, until)),
            Action::GiveUp(reason) => {
                return Err(give_up(target, &snapshot.detection, reason, attempt));
            }
            // An action this loop does not know ends it: nothing is retried blindly.
            _ => {
                return Err(give_up(
                    target,
                    &snapshot.detection,
                    GiveUpReason::AttemptsExhausted,
                    attempt,
                ));
            }
        }
        Ok(Step::Again)
    }

    /// Rotates the egress; with none left to rotate to, the fetch gives up.
    async fn rotate_or_give_up(
        &mut self,
        target: &Target,
        detection: &Detection,
        attempt: u32,
    ) -> Result<(), ScrapeError> {
        match self.rotate_egress().await {
            Err(ScrapeError::NoEgressAvailable) => Err(give_up(
                target,
                detection,
                GiveUpReason::NoEgressLeft,
                attempt,
            )),
            other => other,
        }
    }

    /// Leaves the site alone until `until`, and says so.
    fn cool_down(&self, target: &Target, state: &mut DomainState, until: u64) -> ScrapeError {
        state.start_cooldown(until);
        self.emit(&ScraperEvent::CooldownStarted {
            site: target.site.clone(),
            until,
        });
        ScrapeError::CoolingDown {
            site: target.site.clone(),
            until,
        }
    }

    /// The result of a fetch that ended on a real page.
    fn finished(
        &self,
        target: &Target,
        response: NavResponse,
        snapshot: PageSnapshot,
        attempts: u32,
        solved: Option<Solved>,
    ) -> BrowserFetch {
        if let Some(solved) = &solved {
            self.emit(&ScraperEvent::ChallengeSolved {
                url: target.redacted.clone(),
                kind: solved.kind,
                attempts,
                elapsed_ms: elapsed_ms(solved.since),
            });
        }
        BrowserFetch {
            url: response.url,
            status: response.status,
            headers: response.headers,
            body: snapshot.body,
            detection: snapshot.detection,
            attempts,
            solved: solved.map(|solved| solved.kind),
            egress: self.current_key(),
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
