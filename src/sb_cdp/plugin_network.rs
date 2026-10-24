//! The network side of the plugin hooks: one router per tab that owns `Fetch`.
//!
//! Chrome pauses a request for whoever called `Fetch.enable` on the tab, and a
//! paused request must be answered exactly once. Three things want that
//! right: the library (a proxy that asks for a password), the program
//! ([`Page::intercept`](super::Page::intercept)) and the plugins
//! ([`PluginSet::interest`]). A plugin never gets `Fetch.*` (the guard refuses
//! it), so when the plugins declare any network interest the browser starts a
//! [`NetRouter`] for each tab it prepares, and the router is the only thing
//! that answers the tab's paused requests.
//!
//! # How a paused request flows
//!
//! At the request stage, in this order:
//!
//! 1. The program's own rules (an [`Interception`](super::Interception) on the
//!    tab) are tried first, first match wins. A rule that **fulfils** or
//!    **blocks** is final: the request never reaches the network, so the
//!    plugins are not asked ("user fulfil wins").
//! 2. Otherwise the plugins decide through [`PluginSet::on_request`]: their
//!    blocking rules and their `on_request` hooks, where any block wins
//!    ("deny overrides", also over a program rule that would only change
//!    headers). A block becomes `Fetch.failRequest(BlockedByClient)` and the
//!    event `cdp.plugin.request.failed`, whose `reason` starts with the name
//!    of the plugin. A decision that takes longer than the router's budget
//!    fails the request too: nothing stays paused.
//! 3. Otherwise the request goes on with the headers the plugins' rules add
//!    ([`PluginSet::extra_headers`]) under the headers of a program rule that
//!    changes them (the program's `set_header` and `remove_header` win).
//!
//! At the response stage, which Chrome pauses only for the URLs an `observe`
//! filter names, [`PluginSet::on_response`] sees the status, the type and, up
//! to the filter's `max_body`, the body; the response then goes on. A request
//! the plugins or the program's rules blocked is reported with
//! [`ResponseOutcome::Blocked`]; one the program fulfilled is not reported.
//!
//! Proxy passwords keep working: the router marks its tab as intercepted, so
//! the browser's proxy responder leaves the paused requests to it and goes on
//! answering `Fetch.authRequired`.
//!
//! # Failure
//!
//! The router's task is supervised. If it dies the supervisor sends
//! `Fetch.disable`, so no request stays paused, and gives the tab back to the
//! proxy responder. If the router falls behind the browser's events (it may
//! then have missed a paused request) it turns `Fetch` off and on again,
//! which releases every request that waits; for that moment those requests
//! are not seen by the plugins.

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use base64::Engine as _;
use seleniumbase_rs_plugin::{
    ContextId, NetworkInterest, Outcome as ResponseOutcome, PageId, PluginSet, RequestDecision,
    RequestFilter, RequestView, ResourceKind, ResponseView,
};
use serde_json::{json, Value};
use tokio::task::JoinHandle;

use super::browser::Browser;
use super::client::{CdpEvent, Client, Events};
use super::intercept::{self, Outcome, Request, ResourceType, Rule, Seen};
use super::proxy_auth::ProxyAuth;
use super::sync::locked;
use crate::error::SeleniumBaseError;

/// The longest a request stays paused while the plugins decide about it. It
/// holds even when the plugin set has no timer, so a hook that never returns
/// cannot leave a request hanging.
const DECISION_BUDGET: Duration = Duration::from_secs(10);

/// Response bodies longer than this are never read, whatever a filter asks.
const BODY_CEILING: usize = 16 * 1024 * 1024;

/// Starts the router of a tab, if the plugins declare any network interest.
///
/// The router is kept by the browser until the tab is forgotten.
///
/// # Errors
///
/// [`SeleniumBaseError::CdpDriver`] if the browser refuses `Fetch.enable`.
pub(super) async fn start_router(
    browser: &Browser,
    target: &str,
    session: &str,
) -> Result<(), SeleniumBaseError> {
    let inner = browser.inner();
    let interest = inner.options.plugins.interest();
    if interest.is_empty() {
        return Ok(());
    }
    let tab = Tab {
        page: PageId::new(target),
        context: browser.tab_context(target).map(ContextId::new),
        session: session.into(),
    };
    let router = NetRouter::start(
        Arc::clone(&inner.client),
        Arc::clone(&inner.auth),
        inner.options.plugins.clone(),
        interest,
        tab,
    )
    .await?;
    locked(&inner.routers).insert(session.to_owned(), router);
    Ok(())
}

/// Which tab a router serves.
#[derive(Debug)]
struct Tab {
    page: PageId,
    context: Option<ContextId>,
    session: Arc<str>,
}

/// The program's own rules on a tab, while an interception runs.
#[derive(Debug, Clone)]
struct User {
    rules: Arc<Vec<Rule>>,
    log: Arc<Mutex<Vec<Seen>>>,
    generation: u64,
}

/// Owns `Fetch` on one tab; see the [module docs](self).
#[derive(Debug)]
pub(super) struct NetRouter {
    client: Arc<Client>,
    auth: Arc<ProxyAuth>,
    set: PluginSet,
    interest: NetworkInterest,
    tab: Tab,
    user: Mutex<Option<User>>,
    generation: AtomicU64,
    supervisor: Mutex<Option<JoinHandle<()>>>,
}

impl NetRouter {
    async fn start(
        client: Arc<Client>,
        auth: Arc<ProxyAuth>,
        set: PluginSet,
        interest: NetworkInterest,
        tab: Tab,
    ) -> Result<Arc<Self>, SeleniumBaseError> {
        // Listening before enabling, so no paused request is missed.
        let events = client.events().for_session(&tab.session);
        let router = Arc::new(Self {
            client,
            auth,
            set,
            interest,
            tab,
            user: Mutex::new(None),
            generation: AtomicU64::new(0),
            supervisor: Mutex::new(None),
        });
        router.auth.set_intercepted(&router.tab.session, true);
        if let Err(error) = router.apply().await {
            router.auth.set_intercepted(&router.tab.session, false);
            return Err(error);
        }
        let weak = Arc::downgrade(&router);
        let (client, auth, session) = (
            Arc::clone(&router.client),
            Arc::clone(&router.auth),
            Arc::clone(&router.tab.session),
        );
        let supervisor = supervise(pump(weak, events), move |panicked| async move {
            tracing::event!(
                name: "cdp.plugin.router.ended",
                tracing::Level::WARN,
                panicked,
                "the network router of a tab ended; Fetch is turned off for it",
            );
            give_up(&client, &auth, &session).await;
        });
        *locked(&router.supervisor) = Some(supervisor);
        Ok(router)
    }

    /// Sends `Fetch.enable` with the patterns the tab needs now: the union of
    /// what the plugins declared, everything while the program intercepts or a
    /// proxy asks for a password.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::CdpDriver`] if the browser refuses.
    pub(super) async fn apply(&self) -> Result<(), SeleniumBaseError> {
        let handles_auth = self.auth.handles_auth(&self.tab.session);
        let everything = handles_auth || locked(&self.user).is_some();
        self.client
            .send(
                "Fetch.enable",
                json!({
                    "patterns": patterns(&self.interest, everything),
                    "handleAuthRequests": handles_auth,
                }),
                Some(&self.tab.session),
            )
            .await
            .map(drop)
    }

    /// Puts the program's rules in front of the plugins; returns the log the
    /// interception reads and the number that detaches exactly these rules.
    pub(super) async fn attach_user(
        &self,
        rules: Vec<Rule>,
    ) -> Result<(Arc<Mutex<Vec<Seen>>>, u64), SeleniumBaseError> {
        let generation = self.generation.fetch_add(1, Ordering::AcqRel) + 1;
        let log = Arc::new(Mutex::new(Vec::new()));
        *locked(&self.user) = Some(User {
            rules: Arc::new(rules),
            log: Arc::clone(&log),
            generation,
        });
        if let Err(error) = self.apply().await {
            *locked(&self.user) = None;
            return Err(error);
        }
        Ok((log, generation))
    }

    /// Takes the rules of `generation` away (the rules of a later
    /// interception stay) and narrows the patterns back.
    pub(super) async fn detach_user(&self, generation: u64) -> Result<(), SeleniumBaseError> {
        if !self.forget_user(generation) {
            return Ok(());
        }
        self.apply().await
    }

    /// Drops the rules of `generation`; `false` when other rules have taken
    /// their place.
    fn forget_user(&self, generation: u64) -> bool {
        let mut user = locked(&self.user);
        if user.as_ref().map(|user| user.generation) != Some(generation) {
            return false;
        }
        *user = None;
        true
    }

    /// Stops the router's tasks without touching the tab, for a tab that is
    /// gone.
    pub(super) fn shutdown(&self) {
        if let Some(supervisor) = locked(&self.supervisor).take() {
            supervisor.abort();
        }
    }

    /// The request as a plugin sees it.
    fn view(&self, request: &Request) -> RequestView {
        let kind = kind_of(request.resource_type);
        let mut view = RequestView::new(&request.url, &request.method, kind)
            .navigation(kind == ResourceKind::Document)
            .page(self.tab.page.clone());
        if let Some(context) = &self.tab.context {
            view = view.context(context.clone());
        }
        // Raw: the plugin set hides the sensitive values from the plugins
        // that may not read them.
        view.headers.clone_from(&request.headers);
        view
    }

    /// Handles one `Fetch.requestPaused`.
    async fn paused(&self, params: &Value) {
        let Some(request_id) = params["requestId"].as_str() else {
            return;
        };
        let response_stage = params.get("responseStatusCode").is_some()
            || params.get("responseErrorReason").is_some();
        if response_stage {
            self.response_stage(request_id, params).await;
        } else {
            self.request_stage(request_id, params).await;
        }
    }

    async fn request_stage(&self, request_id: &str, params: &Value) {
        let request = intercept::parse_request(&params["request"], &params["resourceType"]);
        let user = locked(&self.user).clone();
        let rule = user
            .as_ref()
            .and_then(|user| user.rules.iter().find(|rule| rule.matches(&request)));
        let view = self.view(&request);
        let mut extra = BTreeMap::new();

        let outcome = if let Some(rule) = rule.filter(|rule| rule.is_final()) {
            if rule.outcome() == Outcome::Blocked {
                self.report_blocked(&view, "user rule").await;
            }
            rule.outcome()
        } else if let Some(reason) = self.blocked_by_plugins(&view).await {
            tracing::event!(
                name: "cdp.plugin.request.failed",
                tracing::Level::INFO,
                url = %request.url,
                reason = %reason,
                "a plugin blocked {{url}}: {{reason}}",
            );
            let by = reason
                .split(':')
                .next()
                .unwrap_or(&reason)
                .trim()
                .to_owned();
            self.fail(request_id).await;
            self.report_blocked(&view, &by).await;
            record(user.as_ref(), request, Outcome::Blocked);
            return;
        } else {
            extra = self.set.extra_headers(&view);
            if rule.is_some() || !extra.is_empty() {
                Outcome::Modified
            } else {
                Outcome::Continued
            }
        };
        let (method, answer) = intercept::answer(request_id, &request, rule, &extra);
        self.send(method, answer).await;
        record(user.as_ref(), request, outcome);
    }

    /// Why the plugins block the request, if they do. A decision that does not
    /// come in time blocks it.
    async fn blocked_by_plugins(&self, view: &RequestView) -> Option<String> {
        match tokio::time::timeout(DECISION_BUDGET, self.set.on_request(view)).await {
            Ok(RequestDecision::Continue) => None,
            Ok(RequestDecision::Block(reason)) => Some(reason),
            Ok(_) => Some("router: a decision it does not know".to_owned()),
            Err(_) => Some(format!(
                "router: the plugins did not decide within {DECISION_BUDGET:?}"
            )),
        }
    }

    async fn fail(&self, request_id: &str) {
        self.send(
            "Fetch.failRequest",
            json!({ "requestId": request_id, "errorReason": "BlockedByClient" }),
        )
        .await;
    }

    /// Tells the observers about a request that never reached the network.
    async fn report_blocked(&self, view: &RequestView, by: &str) {
        if self.interest.observe.is_empty() {
            return;
        }
        let response =
            ResponseView::new(view, 0, "").outcome(ResponseOutcome::Blocked { by: by.to_owned() });
        self.deliver(&response).await;
    }

    async fn deliver(&self, response: &ResponseView<'_>) {
        let failures = tokio::time::timeout(DECISION_BUDGET, self.set.on_response(response))
            .await
            .unwrap_or_default();
        for error in failures {
            tracing::event!(
                name: "cdp.plugin.response.failed",
                tracing::Level::WARN,
                error = %error,
                "a plugin failed on a response: {{error}}",
            );
        }
    }

    async fn response_stage(&self, request_id: &str, params: &Value) {
        let request = intercept::parse_request(&params["request"], &params["resourceType"]);
        let view = self.view(&request);
        let wanted: Vec<&RequestFilter> = self
            .interest
            .observe
            .iter()
            .filter(|filter| filter.matches(&view))
            .collect();
        if !wanted.is_empty() {
            let failed = params.get("responseErrorReason").is_some();
            let status = params["responseStatusCode"]
                .as_u64()
                .and_then(|status| u16::try_from(status).ok())
                .unwrap_or(0);
            let mime = mime_of(&params["responseHeaders"]);
            let cap = wanted.iter().filter_map(|filter| filter.max_body).max();
            // A redirect has no body to read, and neither has a failure.
            let body = match cap {
                Some(cap) if cap > 0 && !failed && !(300..400).contains(&status) => {
                    self.read_body(request_id, params, cap).await
                }
                _ => None,
            };
            let mut response = ResponseView::new(&view, status, &mime);
            if let Some((bytes, truncated)) = &body {
                response = response.body(bytes).truncated(*truncated);
            }
            if failed {
                response = response.outcome(ResponseOutcome::Failed);
            }
            self.deliver(&response).await;
        }
        self.release_response(request_id).await;
    }

    /// Lets a paused response go on unchanged. `Fetch.continueResponse` is
    /// the command for it; a browser that does not know it is asked with
    /// `Fetch.continueRequest`, which also resumes a response, so nothing
    /// stays paused.
    async fn release_response(&self, request_id: &str) {
        let released = self
            .client
            .send(
                "Fetch.continueResponse",
                json!({ "requestId": request_id }),
                Some(&self.tab.session),
            )
            .await;
        if released.is_err() {
            self.send("Fetch.continueRequest", json!({ "requestId": request_id }))
                .await;
        }
    }

    /// The body of the paused response, cut to `cap`, and whether it was cut.
    async fn read_body(
        &self,
        request_id: &str,
        params: &Value,
        cap: usize,
    ) -> Option<(Vec<u8>, bool)> {
        if announced_length(&params["responseHeaders"]).is_some_and(|length| length > BODY_CEILING)
        {
            return None;
        }
        let reply = self
            .client
            .send(
                "Fetch.getResponseBody",
                json!({ "requestId": request_id }),
                Some(&self.tab.session),
            )
            .await
            .ok()?;
        let text = reply["body"].as_str()?;
        let mut bytes = if reply["base64Encoded"].as_bool() == Some(true) {
            base64::engine::general_purpose::STANDARD
                .decode(text)
                .ok()?
        } else {
            text.as_bytes().to_vec()
        };
        let truncated = bytes.len() > cap;
        bytes.truncate(cap);
        Some((bytes, truncated))
    }

    async fn send(&self, method: &'static str, params: Value) {
        // A request that already finished has nothing left to answer.
        if let Err(error) = self
            .client
            .send(method, params, Some(&self.tab.session))
            .await
        {
            tracing::event!(
                name: "cdp.plugin.router.answer_failed",
                tracing::Level::DEBUG,
                method,
                error = %error,
                "{{method}} for a paused request failed: {{error}}",
            );
        }
    }

    /// After events were lost a request may wait for an answer that nobody
    /// will give: turn `Fetch` off, which lets every waiting request go, and
    /// on again.
    async fn recover(&self) {
        tracing::event!(
            name: "cdp.plugin.router.lagged",
            tracing::Level::WARN,
            "the network router fell behind the browser; releasing the requests it may have missed",
        );
        let _ = self
            .client
            .send("Fetch.disable", json!({}), Some(&self.tab.session))
            .await;
        if let Err(error) = self.apply().await {
            tracing::event!(
                name: "cdp.plugin.router.reenable_failed",
                tracing::Level::WARN,
                error = %error,
                "the network router could not turn Fetch on again: {{error}}",
            );
        }
    }
}

impl Drop for NetRouter {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Logs a request for the program's interception, if one runs.
fn record(user: Option<&User>, request: Request, outcome: Outcome) {
    if let Some(user) = user {
        let mut seen = locked(&user.log);
        if seen.len() < intercept::LOG_LIMIT {
            seen.push(Seen { request, outcome });
        }
    }
}

/// Gives each `Fetch.requestPaused` of the tab to the router, each in a task of
/// its own so a slow plugin holds up that request only.
async fn pump(router: Weak<NetRouter>, mut events: Events) {
    let mut missed = events.missed();
    while let Some(CdpEvent { method, params, .. }) = events.next().await {
        let Some(router) = router.upgrade() else {
            return;
        };
        if events.missed() > missed {
            missed = events.missed();
            router.recover().await;
        }
        if method == "Fetch.requestPaused" {
            tokio::spawn(async move { router.paused(&params).await });
        }
    }
}

/// Hands the tab back when the router's task ends other than by being
/// stopped: `Fetch` off, and the proxy responder answering again.
async fn give_up(client: &Client, auth: &ProxyAuth, session: &str) {
    auth.set_intercepted(session, false);
    let _ = client.send("Fetch.disable", json!({}), Some(session)).await;
    if auth.handles_auth(session) {
        let _ = client
            .send(
                "Fetch.enable",
                json!({ "handleAuthRequests": true, "patterns": [{ "urlPattern": "*" }] }),
                Some(session),
            )
            .await;
    }
}

/// Aborts the task it holds when dropped.
struct AbortOnDrop(JoinHandle<()>);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Runs `work` in a task of its own and calls `ended(panicked)` when that
/// task ends without being cancelled. Cancelling the returned handle cancels
/// `work` too, and `ended` does not run.
fn supervise<Fut, End, EndFut>(work: Fut, ended: End) -> JoinHandle<()>
where
    Fut: Future<Output = ()> + Send + 'static,
    End: FnOnce(bool) -> EndFut + Send + 'static,
    EndFut: Future<Output = ()> + Send + 'static,
{
    tokio::spawn(async move {
        let mut worker = AbortOnDrop(tokio::spawn(work));
        match (&mut worker.0).await {
            Ok(()) => ended(false).await,
            Err(error) if error.is_panic() => ended(true).await,
            Err(_) => {}
        }
    })
}

fn kind_of(kind: ResourceType) -> ResourceKind {
    match kind {
        ResourceType::Document => ResourceKind::Document,
        ResourceType::Stylesheet => ResourceKind::Stylesheet,
        ResourceType::Image => ResourceKind::Image,
        ResourceType::Media => ResourceKind::Media,
        ResourceType::Font => ResourceKind::Font,
        ResourceType::Script => ResourceKind::Script,
        ResourceType::Xhr => ResourceKind::Xhr,
        ResourceType::Fetch => ResourceKind::Fetch,
        ResourceType::WebSocket => ResourceKind::WebSocket,
        ResourceType::Other => ResourceKind::Other,
    }
}

/// The `Fetch` name of a kind of resource, when it has exactly one.
fn resource_name(kind: ResourceKind) -> Option<&'static str> {
    match kind {
        ResourceKind::Document => Some("Document"),
        ResourceKind::Stylesheet => Some("Stylesheet"),
        ResourceKind::Image => Some("Image"),
        ResourceKind::Media => Some("Media"),
        ResourceKind::Font => Some("Font"),
        ResourceKind::Script => Some("Script"),
        ResourceKind::Xhr => Some("XHR"),
        ResourceKind::Fetch => Some("Fetch"),
        ResourceKind::WebSocket => Some("WebSocket"),
        // `Other` stands for several protocol types, and a kind added later
        // is not known here: pause them all, the filters decide.
        _ => None,
    }
}

/// One `Fetch.enable` pattern: URL glob, resource type, response stage.
type Pattern = (String, Option<&'static str>, bool);

/// The patterns that make Chrome pause what the plugins asked for, and
/// everything while `everything` is set: the union, so Chrome stops only the
/// matching requests. The request stage is for rules and intercept filters,
/// the response stage for observe filters. Methods cannot be named in a
/// pattern; the filters check them when a request arrives.
fn patterns(interest: &NetworkInterest, everything: bool) -> Vec<Value> {
    let mut wanted: BTreeSet<Pattern> = BTreeSet::new();
    if everything {
        wanted.insert(("*".to_owned(), None, false));
    }
    let request_filters = interest
        .rules
        .iter()
        .map(|rule| &rule.filter)
        .chain(&interest.intercept);
    for filter in request_filters {
        wanted.extend(filter_patterns(filter, false));
    }
    for filter in &interest.observe {
        wanted.extend(filter_patterns(filter, true));
    }
    // A pattern for everything makes the narrower ones of its stage redundant.
    for stage in [false, true] {
        if wanted.contains(&("*".to_owned(), None, stage)) {
            wanted.retain(|(url, kind, at)| *at != stage || (url == "*" && kind.is_none()));
        }
    }
    wanted
        .into_iter()
        .map(|(url, kind, response)| {
            let mut pattern = json!({
                "urlPattern": url,
                "requestStage": if response { "Response" } else { "Request" },
            });
            if let Some(kind) = kind {
                pattern["resourceType"] = json!(kind);
            }
            pattern
        })
        .collect()
}

fn filter_patterns(filter: &RequestFilter, response: bool) -> Vec<Pattern> {
    // The filters' globs know no escape character; Chrome's patterns do.
    let url = filter
        .url
        .as_deref()
        .map_or_else(|| "*".to_owned(), |glob| glob.replace('\\', "\\\\"));
    if filter.kinds.is_empty() {
        return vec![(url, None, response)];
    }
    filter
        .kinds
        .iter()
        .map(|kind| (url.clone(), resource_name(*kind), response))
        .collect()
}

/// The MIME type in the `Content-Type` of the response headers (a list of
/// `{name, value}`), without parameters; empty when there is none.
fn mime_of(headers: &Value) -> String {
    header_value(headers, "content-type")
        .and_then(|value| value.split(';').next())
        .map(|mime| mime.trim().to_ascii_lowercase())
        .unwrap_or_default()
}

/// The `Content-Length` of the response, when it states one.
fn announced_length(headers: &Value) -> Option<usize> {
    header_value(headers, "content-length")?.trim().parse().ok()
}

fn header_value<'a>(headers: &'a Value, name: &str) -> Option<&'a str> {
    headers.as_array()?.iter().find_map(|header| {
        header["name"]
            .as_str()
            .filter(|found| found.eq_ignore_ascii_case(name))
            .and_then(|_| header["value"].as_str())
    })
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, AtomicU8};

    use seleniumbase_rs_plugin::NetRule;

    use super::*;

    /// (glob, stage, resource type) of each pattern.
    fn found(patterns: &[Value]) -> Vec<(String, String, Option<String>)> {
        patterns
            .iter()
            .map(|pattern| {
                (
                    pattern["urlPattern"].as_str().unwrap().to_owned(),
                    pattern["requestStage"].as_str().unwrap().to_owned(),
                    pattern["resourceType"].as_str().map(str::to_owned),
                )
            })
            .collect()
    }

    fn pattern(url: &str, stage: &str, kind: Option<&str>) -> (String, String, Option<String>) {
        (url.to_owned(), stage.to_owned(), kind.map(str::to_owned))
    }

    #[test]
    fn the_patterns_are_the_union_of_what_the_plugins_declared() {
        let interest = NetworkInterest::none()
            .rule(NetRule::block(
                RequestFilter::any().url("https://ads.test/*"),
            ))
            .intercept(RequestFilter::any().url("https://api.test/*"))
            .observe(RequestFilter::any().url("https://api.test/*").max_body(10));

        assert_eq!(
            found(&patterns(&interest, false)),
            [
                pattern("https://ads.test/*", "Request", None),
                pattern("https://api.test/*", "Request", None),
                pattern("https://api.test/*", "Response", None),
            ]
        );
    }

    #[test]
    fn a_filter_for_several_kinds_is_one_pattern_for_each() {
        let interest = NetworkInterest::none()
            .observe(RequestFilter::any().kinds([ResourceKind::Xhr, ResourceKind::Image]));

        assert_eq!(
            found(&patterns(&interest, false)),
            [
                pattern("*", "Response", Some("Image")),
                pattern("*", "Response", Some("XHR")),
            ]
        );
    }

    #[test]
    fn the_kind_other_pauses_every_kind_and_the_filter_decides() {
        let interest = NetworkInterest::none()
            .intercept(RequestFilter::any().kinds([ResourceKind::Other, ResourceKind::Image]));

        assert_eq!(
            found(&patterns(&interest, false)),
            [pattern("*", "Request", None)]
        );
    }

    #[test]
    fn everything_makes_the_narrower_patterns_of_its_stage_redundant() {
        let interest = NetworkInterest::none()
            .intercept(RequestFilter::any().url("https://api.test/*"))
            .observe(RequestFilter::any().url("https://api.test/*"));

        assert_eq!(
            found(&patterns(&interest, true)),
            [
                pattern("*", "Request", None),
                pattern("https://api.test/*", "Response", None),
            ],
            "the response stage is not widened"
        );
    }

    #[test]
    fn a_backslash_in_a_glob_is_not_an_escape() {
        let interest =
            NetworkInterest::none().intercept(RequestFilter::any().url(r"https://a.test/\*"));

        assert_eq!(
            found(&patterns(&interest, false)),
            [pattern(r"https://a.test/\\*", "Request", None)]
        );
    }

    #[test]
    fn the_mime_type_comes_from_the_content_type_without_parameters() {
        let headers = json!([
            { "name": "Server", "value": "x" },
            { "name": "content-TYPE", "value": "Text/HTML; charset=utf-8" },
            { "name": "Content-Length", "value": " 42 " },
        ]);
        assert_eq!(mime_of(&headers), "text/html");
        assert_eq!(announced_length(&headers), Some(42));
        assert_eq!(mime_of(&json!([])), "");
        assert_eq!(mime_of(&Value::Null), "");
        let many = json!([{ "name": "Content-Length", "value": "many" }]);
        assert_eq!(announced_length(&many), None);
    }

    #[tokio::test]
    async fn a_task_that_panics_is_reported_and_one_that_is_stopped_is_not() {
        let state = Arc::new(AtomicU8::new(0));
        let reported = Arc::clone(&state);
        let _ = supervise(
            async { panic!("the router's task fell over") },
            move |panicked| async move {
                reported.store(if panicked { 2 } else { 1 }, Ordering::SeqCst);
            },
        )
        .await;
        assert_eq!(state.load(Ordering::SeqCst), 2);

        let ran = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&ran);
        let supervisor = supervise(std::future::pending::<()>(), move |_| async move {
            flag.store(true, Ordering::SeqCst);
        });
        tokio::task::yield_now().await;
        supervisor.abort();
        let _ = supervisor.await;
        tokio::task::yield_now().await;
        assert!(!ran.load(Ordering::SeqCst), "stopping is not a failure");
    }
}
