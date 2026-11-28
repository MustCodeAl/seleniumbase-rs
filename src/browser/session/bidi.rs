//! WebDriver BiDi event subscriptions and log/network forwarding for [`BrowserSession`].

use futures_util::StreamExt;
use serde_json::Value;
use thirtyfour::bidi::RequestId;
use thirtyfour::bidi::events::{
    AuthRequired, BeforeRequestSent, FetchError, LogEntryAdded, ResponseCompleted, ResponseStarted,
};
use thirtyfour::bidi::modules::network::{InterceptGuard, InterceptPhase};
use tokio::sync::mpsc::{Receiver, channel};

use super::BrowserSession;
use crate::error::SeleniumBaseError;

/// Default capacity for asynchronous BiDi event streaming channels.
pub const DEFAULT_BIDI_BUFFER_SIZE: usize = 256;

/// A network lifecycle event received over WebDriver BiDi.
///
/// Encapsulates the various network lifecycle stages defined by the W3C WebDriver BiDi
/// specification (`network.beforeRequestSent`, `network.responseStarted`,
/// `network.responseCompleted`, `network.fetchError`, and `network.authRequired`).
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum BidiNetworkEvent {
    /// Request is about to be sent (`network.beforeRequestSent`).
    BeforeRequestSent(BeforeRequestSent),
    /// Response headers have arrived (`network.responseStarted`).
    ResponseStarted(ResponseStarted),
    /// Response body has completed loading or request was cancelled (`network.responseCompleted`).
    ResponseCompleted(ResponseCompleted),
    /// Request failed before completing (`network.fetchError`).
    FetchError(FetchError),
    /// HTTP 401 or 407 authentication challenge received (`network.authRequired`).
    AuthRequired(AuthRequired),
}

impl BidiNetworkEvent {
    /// Returns the target URL of the request or response.
    #[must_use]
    pub fn url(&self) -> &str {
        match self {
            Self::BeforeRequestSent(e) => &e.request.url,
            Self::ResponseStarted(e) => &e.response.url,
            Self::ResponseCompleted(e) => &e.response.url,
            Self::FetchError(e) => &e.request.url,
            Self::AuthRequired(e) => &e.request.url,
        }
    }

    /// Returns the HTTP method of the request (`"GET"`, `"POST"`, etc.).
    #[must_use]
    pub fn method(&self) -> Option<&str> {
        match self {
            Self::BeforeRequestSent(e) => Some(&e.request.method),
            Self::ResponseStarted(e) => Some(&e.request.method),
            Self::ResponseCompleted(e) => Some(&e.request.method),
            Self::FetchError(e) => Some(&e.request.method),
            Self::AuthRequired(e) => Some(&e.request.method),
        }
    }

    /// Returns whether this network event indicates a blocked request paused by an intercept.
    #[must_use]
    pub fn is_blocked(&self) -> bool {
        match self {
            Self::BeforeRequestSent(e) => e.is_blocked,
            Self::ResponseStarted(e) => e.is_blocked,
            Self::AuthRequired(_) => true,
            Self::ResponseCompleted(_) | Self::FetchError(_) => false,
        }
    }

    /// Returns the HTTP status code for response events, or `None` for request/error events.
    #[must_use]
    pub fn status(&self) -> Option<u16> {
        match self {
            Self::ResponseStarted(e) => Some(e.response.status),
            Self::ResponseCompleted(e) => Some(e.response.status),
            Self::BeforeRequestSent(_) | Self::FetchError(_) | Self::AuthRequired(_) => None,
        }
    }

    /// Returns the request id associated with this event.
    #[must_use]
    pub fn request_id(&self) -> &RequestId {
        match self {
            Self::BeforeRequestSent(e) => &e.request.id,
            Self::ResponseStarted(e) => &e.request.id,
            Self::ResponseCompleted(e) => &e.request.id,
            Self::FetchError(e) => &e.request.id,
            Self::AuthRequired(e) => &e.request.id,
        }
    }

    /// Returns the driver timestamp in milliseconds since the Unix epoch.
    #[must_use]
    pub fn timestamp(&self) -> u64 {
        match self {
            Self::BeforeRequestSent(e) => e.timestamp,
            Self::ResponseStarted(e) => e.timestamp,
            Self::ResponseCompleted(e) => e.timestamp,
            Self::FetchError(e) => e.timestamp,
            Self::AuthRequired(e) => e.timestamp,
        }
    }
}

impl From<BeforeRequestSent> for BidiNetworkEvent {
    fn from(event: BeforeRequestSent) -> Self {
        Self::BeforeRequestSent(event)
    }
}

impl From<ResponseStarted> for BidiNetworkEvent {
    fn from(event: ResponseStarted) -> Self {
        Self::ResponseStarted(event)
    }
}

impl From<ResponseCompleted> for BidiNetworkEvent {
    fn from(event: ResponseCompleted) -> Self {
        Self::ResponseCompleted(event)
    }
}

impl From<FetchError> for BidiNetworkEvent {
    fn from(event: FetchError) -> Self {
        Self::FetchError(event)
    }
}

impl From<AuthRequired> for BidiNetworkEvent {
    fn from(event: AuthRequired) -> Self {
        Self::AuthRequired(event)
    }
}

impl BrowserSession {
    /// Subscribes to WebDriver BiDi `log.entryAdded` events with default buffer capacity (256)
    /// and forwards all console logs and JavaScript errors to an async mpsc channel.
    ///
    /// Dropping the receiver closes the channel, which automatically terminates the background
    /// listener task and releases the BiDi wire subscription.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::SessionNotStarted`] if no session is active.
    /// Returns [`SeleniumBaseError`] if establishing BiDi or subscribing fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use seleniumbase_rs::BrowserSession;
    /// # async fn test(session: BrowserSession) -> Result<(), seleniumbase_rs::error::SeleniumBaseError> {
    /// let mut logs = session.bidi_listen_logs().await?;
    /// while let Ok(entry) = logs.try_recv() {
    ///     println!("[{}] {:?}", entry.level, entry.text);
    /// }
    /// # Ok(()) }
    /// ```
    pub async fn bidi_listen_logs(&self) -> Result<Receiver<LogEntryAdded>, SeleniumBaseError> {
        self.bidi_listen_logs_with_buffer(DEFAULT_BIDI_BUFFER_SIZE)
            .await
    }

    /// Subscribes to WebDriver BiDi `log.entryAdded` events with a custom channel buffer capacity.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::SessionNotStarted`] if no session is active.
    /// Returns [`SeleniumBaseError`] if establishing BiDi or subscribing fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use seleniumbase_rs::BrowserSession;
    /// # async fn test(session: BrowserSession) -> Result<(), seleniumbase_rs::error::SeleniumBaseError> {
    /// let mut logs = session.bidi_listen_logs_with_buffer(512).await?;
    /// # Ok(()) }
    /// ```
    pub async fn bidi_listen_logs_with_buffer(
        &self,
        buffer: usize,
    ) -> Result<Receiver<LogEntryAdded>, SeleniumBaseError> {
        let bidi = self.bidi().await?;
        let mut stream = bidi.subscribe::<LogEntryAdded>().await?;

        let (tx, rx) = channel(buffer);
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    biased;
                    _ = tx.closed() => break,
                    entry = stream.next() => {
                        match entry {
                            Some(entry) => {
                                if tx.send(entry).await.is_err() {
                                    break;
                                }
                            }
                            None => break,
                        }
                    }
                }
            }
        });

        Ok(rx)
    }

    /// Subscribes to WebDriver BiDi network events with default buffer capacity (256)
    /// and forwards all request and response lifecycle events to an async mpsc channel.
    ///
    /// Subscribes to `network.beforeRequestSent`, `network.responseStarted`,
    /// `network.responseCompleted`, `network.fetchError`, and `network.authRequired`.
    /// Dropping the receiver closes the channel, which automatically terminates the
    /// background forwarder task and unsubscribes from BiDi wire events.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::SessionNotStarted`] if no session is active.
    /// Returns [`SeleniumBaseError`] if establishing BiDi or subscribing fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use seleniumbase_rs::BrowserSession;
    /// # async fn test(session: BrowserSession) -> Result<(), seleniumbase_rs::error::SeleniumBaseError> {
    /// let mut net = session.bidi_listen_network().await?;
    /// while let Ok(event) = net.try_recv() {
    ///     println!("{} {}", event.method().unwrap_or(""), event.url());
    /// }
    /// # Ok(()) }
    /// ```
    pub async fn bidi_listen_network(
        &self,
    ) -> Result<Receiver<BidiNetworkEvent>, SeleniumBaseError> {
        self.bidi_listen_network_with_buffer(DEFAULT_BIDI_BUFFER_SIZE)
            .await
    }

    /// Subscribes to WebDriver BiDi network events with a custom channel buffer capacity.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::SessionNotStarted`] if no session is active.
    /// Returns [`SeleniumBaseError`] if establishing BiDi or subscribing fails.
    pub async fn bidi_listen_network_with_buffer(
        &self,
        buffer: usize,
    ) -> Result<Receiver<BidiNetworkEvent>, SeleniumBaseError> {
        let bidi = self.bidi().await?;

        let mut before_req = bidi.subscribe::<BeforeRequestSent>().await?;
        let mut resp_start = bidi.subscribe::<ResponseStarted>().await?;
        let mut resp_comp = bidi.subscribe::<ResponseCompleted>().await?;
        let mut fetch_err = bidi.subscribe::<FetchError>().await?;
        let mut auth_req = bidi.subscribe::<AuthRequired>().await?;

        let (tx, rx) = channel(buffer);
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    biased;
                    _ = tx.closed() => break,
                    Some(ev) = before_req.next() => {
                        if tx.send(BidiNetworkEvent::BeforeRequestSent(ev)).await.is_err() {
                            break;
                        }
                    }
                    Some(ev) = resp_start.next() => {
                        if tx.send(BidiNetworkEvent::ResponseStarted(ev)).await.is_err() {
                            break;
                        }
                    }
                    Some(ev) = resp_comp.next() => {
                        if tx.send(BidiNetworkEvent::ResponseCompleted(ev)).await.is_err() {
                            break;
                        }
                    }
                    Some(ev) = fetch_err.next() => {
                        if tx.send(BidiNetworkEvent::FetchError(ev)).await.is_err() {
                            break;
                        }
                    }
                    Some(ev) = auth_req.next() => {
                        if tx.send(BidiNetworkEvent::AuthRequired(ev)).await.is_err() {
                            break;
                        }
                    }
                    else => break,
                }
            }
        });

        Ok(rx)
    }

    /// Subscribes exclusively to outgoing request events (`network.beforeRequestSent`).
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::SessionNotStarted`] if no session is active.
    /// Returns [`SeleniumBaseError`] if establishing BiDi or subscribing fails.
    pub async fn bidi_listen_requests(
        &self,
    ) -> Result<Receiver<BeforeRequestSent>, SeleniumBaseError> {
        let bidi = self.bidi().await?;
        let mut stream = bidi.subscribe::<BeforeRequestSent>().await?;

        let (tx, rx) = channel(DEFAULT_BIDI_BUFFER_SIZE);
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    biased;
                    _ = tx.closed() => break,
                    entry = stream.next() => {
                        match entry {
                            Some(entry) => {
                                if tx.send(entry).await.is_err() {
                                    break;
                                }
                            }
                            None => break,
                        }
                    }
                }
            }
        });

        Ok(rx)
    }

    /// Subscribes exclusively to completed response events (`network.responseCompleted`).
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::SessionNotStarted`] if no session is active.
    /// Returns [`SeleniumBaseError`] if establishing BiDi or subscribing fails.
    pub async fn bidi_listen_responses(
        &self,
    ) -> Result<Receiver<ResponseCompleted>, SeleniumBaseError> {
        let bidi = self.bidi().await?;
        let mut stream = bidi.subscribe::<ResponseCompleted>().await?;

        let (tx, rx) = channel(DEFAULT_BIDI_BUFFER_SIZE);
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    biased;
                    _ = tx.closed() => break,
                    entry = stream.next() => {
                        match entry {
                            Some(entry) => {
                                if tx.send(entry).await.is_err() {
                                    break;
                                }
                            }
                            None => break,
                        }
                    }
                }
            }
        });

        Ok(rx)
    }

    /// Adds a WebDriver BiDi network interception rule to pause requests matching
    /// `phases` and optional `url_patterns`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::SessionNotStarted`] if no session is active.
    /// Returns [`SeleniumBaseError`] if registering the intercept fails.
    pub async fn bidi_add_intercept(
        &self,
        phases: Vec<InterceptPhase>,
        url_patterns: Option<Vec<Value>>,
    ) -> Result<InterceptGuard, SeleniumBaseError> {
        let bidi = self.bidi().await?;
        bidi.network()
            .add_intercept(phases, url_patterns)
            .await
            .map_err(Into::into)
    }

    /// Continues an intercepted network request unmodified via BiDi `network.continueRequest`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::SessionNotStarted`] if no session is active.
    /// Returns [`SeleniumBaseError`] if the BiDi command fails.
    pub async fn bidi_continue_request(&self, request: RequestId) -> Result<(), SeleniumBaseError> {
        let bidi = self.bidi().await?;
        bidi.network()
            .continue_request(request)
            .await
            .map_err(Into::into)
    }

    /// Continues an intercepted response unmodified via BiDi `network.continueResponse`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::SessionNotStarted`] if no session is active.
    /// Returns [`SeleniumBaseError`] if the BiDi command fails.
    pub async fn bidi_continue_response(
        &self,
        request: RequestId,
    ) -> Result<(), SeleniumBaseError> {
        let bidi = self.bidi().await?;
        bidi.network()
            .continue_response(request)
            .await
            .map_err(Into::into)
    }

    /// Fails an intercepted network request via BiDi `network.failRequest`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::SessionNotStarted`] if no session is active.
    /// Returns [`SeleniumBaseError`] if the BiDi command fails.
    pub async fn bidi_fail_request(&self, request: RequestId) -> Result<(), SeleniumBaseError> {
        let bidi = self.bidi().await?;
        bidi.network()
            .fail_request(request)
            .await
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use thirtyfour::bidi::modules::log::LogLevel;

    #[test]
    fn bidi_log_entry_deserialization() {
        let raw = json!({
            "type": "console",
            "level": "info",
            "source": { "realm": "r-1" },
            "text": "BiDi log message",
            "timestamp": 1_700_000_000_123_u64,
        });

        let entry: LogEntryAdded = serde_json::from_value(raw).expect("deserializes log entry");
        assert_eq!(entry.entry_type, "console");
        assert!(matches!(entry.level, LogLevel::Info));
        assert_eq!(entry.text.as_deref(), Some("BiDi log message"));
        assert_eq!(entry.timestamp, 1_700_000_000_123);
    }

    #[test]
    fn bidi_network_event_accessors_and_conversions() {
        let raw_req = json!({
            "context": "ctx-1",
            "request": {
                "request": "req-101",
                "url": "https://example.com/api/data",
                "method": "POST",
                "headers": [],
                "cookies": [],
            },
            "timestamp": 1_700_000_000_500_u64,
            "isBlocked": true,
            "intercepts": ["int-1"],
        });
        let req: BeforeRequestSent =
            serde_json::from_value(raw_req).expect("deserializes BeforeRequestSent");
        let ev: BidiNetworkEvent = req.into();
        assert_eq!(ev.url(), "https://example.com/api/data");
        assert_eq!(ev.method(), Some("POST"));
        assert!(ev.is_blocked());
        assert_eq!(ev.status(), None);
        assert_eq!(&ev.request_id().0, "req-101");
        assert_eq!(ev.timestamp(), 1_700_000_000_500);

        let raw_resp = json!({
            "context": "ctx-1",
            "request": {
                "request": "req-102",
                "url": "https://example.com/home",
                "method": "GET",
                "headers": [],
                "cookies": [],
            },
            "response": {
                "url": "https://example.com/home",
                "status": 200,
                "statusText": "OK",
                "headers": [],
            },
            "timestamp": 1_700_000_000_600_u64,
        });
        let resp: ResponseCompleted =
            serde_json::from_value(raw_resp).expect("deserializes ResponseCompleted");
        let ev: BidiNetworkEvent = resp.into();
        assert_eq!(ev.url(), "https://example.com/home");
        assert_eq!(ev.method(), Some("GET"));
        assert!(!ev.is_blocked());
        assert_eq!(ev.status(), Some(200));
        assert_eq!(&ev.request_id().0, "req-102");
        assert_eq!(ev.timestamp(), 1_700_000_000_600);

        let raw_err = json!({
            "context": "ctx-1",
            "request": {
                "request": "req-103",
                "url": "https://example.com/fail",
                "method": "GET",
                "headers": [],
                "cookies": [],
            },
            "timestamp": 1_700_000_000_700_u64,
            "errorText": "net::ERR_CONNECTION_REFUSED",
        });
        let err: FetchError = serde_json::from_value(raw_err).expect("deserializes FetchError");
        let ev: BidiNetworkEvent = err.into();
        assert_eq!(ev.url(), "https://example.com/fail");
        assert_eq!(ev.method(), Some("GET"));
        assert!(!ev.is_blocked());
        assert_eq!(ev.status(), None);
        assert_eq!(&ev.request_id().0, "req-103");
        assert_eq!(ev.timestamp(), 1_700_000_000_700);
    }

    #[tokio::test]
    async fn channel_dispatch_and_receiver_lifecycle() {
        let (tx, mut rx) = channel::<BidiNetworkEvent>(DEFAULT_BIDI_BUFFER_SIZE);

        let raw_resp = json!({
            "context": "ctx-1",
            "request": {
                "request": "req-1",
                "url": "https://example.com/test",
                "method": "GET",
                "headers": [],
                "cookies": [],
            },
            "response": {
                "url": "https://example.com/test",
                "status": 204,
                "statusText": "No Content",
                "headers": [],
            },
            "timestamp": 1_700_000_000_800_u64,
        });
        let resp: ResponseCompleted = serde_json::from_value(raw_resp).unwrap();
        tx.send(resp.into()).await.expect("channel send succeeds");

        let received = rx.recv().await.expect("receives event");
        assert_eq!(received.url(), "https://example.com/test");
        assert_eq!(received.status(), Some(204));

        // Dropping receiver signals closed to sender
        drop(rx);
        tx.closed().await;
        assert!(tx.is_closed());
    }

    #[tokio::test]
    async fn disconnected_session_returns_session_not_started() {
        let session = BrowserSession::disconnected();

        assert!(matches!(
            session.bidi_listen_logs().await,
            Err(SeleniumBaseError::SessionNotStarted)
        ));
        assert!(matches!(
            session.bidi_listen_network().await,
            Err(SeleniumBaseError::SessionNotStarted)
        ));
        assert!(matches!(
            session.bidi_listen_requests().await,
            Err(SeleniumBaseError::SessionNotStarted)
        ));
        assert!(matches!(
            session.bidi_listen_responses().await,
            Err(SeleniumBaseError::SessionNotStarted)
        ));
        assert!(matches!(
            session.bidi_add_intercept(vec![], None).await,
            Err(SeleniumBaseError::SessionNotStarted)
        ));
        assert!(matches!(
            session
                .bidi_continue_request(RequestId::from("r1".to_string()))
                .await,
            Err(SeleniumBaseError::SessionNotStarted)
        ));
        assert!(matches!(
            session
                .bidi_continue_response(RequestId::from("r1".to_string()))
                .await,
            Err(SeleniumBaseError::SessionNotStarted)
        ));
        assert!(matches!(
            session
                .bidi_fail_request(RequestId::from("r1".to_string()))
                .await,
            Err(SeleniumBaseError::SessionNotStarted)
        ));
    }
}
