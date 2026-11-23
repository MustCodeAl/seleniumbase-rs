// WebDriver BiDi event subscriptions and log/network forwarding for `BaseCase`.
// Included in `base_case.rs`.

use crate::browser::session::BidiNetworkEvent;
use thirtyfour::bidi::events::{BeforeRequestSent, LogEntryAdded, ResponseCompleted};
use thirtyfour::bidi::modules::network::{InterceptGuard, InterceptPhase};
use thirtyfour::bidi::RequestId;
use tokio::sync::mpsc::Receiver;

impl BaseCase {
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
    /// use seleniumbase_rs::BaseCase;
    ///
    /// # async fn demo(mut sb: BaseCase) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    /// let mut logs = sb.bidi_listen_logs().await?;
    /// sb.open("https://example.com").await?;
    /// while let Ok(entry) = logs.try_recv() {
    ///     println!("[{:?}] {:?}", entry.level, entry.text);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn bidi_listen_logs(
        &self,
    ) -> Result<Receiver<LogEntryAdded>, SeleniumBaseError> {
        self.session.bidi_listen_logs().await
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
    /// use seleniumbase_rs::BaseCase;
    ///
    /// # async fn demo(sb: BaseCase) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    /// let mut logs = sb.bidi_listen_logs_with_buffer(512).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn bidi_listen_logs_with_buffer(
        &self,
        buffer: usize,
    ) -> Result<Receiver<LogEntryAdded>, SeleniumBaseError> {
        self.session.bidi_listen_logs_with_buffer(buffer).await
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
    /// use seleniumbase_rs::BaseCase;
    ///
    /// # async fn demo(mut sb: BaseCase) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    /// let mut net = sb.bidi_listen_network().await?;
    /// sb.open("https://example.com").await?;
    /// while let Ok(event) = net.try_recv() {
    ///     println!("{} {}", event.method().unwrap_or(""), event.url());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn bidi_listen_network(
        &self,
    ) -> Result<Receiver<BidiNetworkEvent>, SeleniumBaseError> {
        self.session.bidi_listen_network().await
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
        self.session.bidi_listen_network_with_buffer(buffer).await
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
        self.session.bidi_listen_requests().await
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
        self.session.bidi_listen_responses().await
    }

    /// Adds a WebDriver BiDi network interception rule to pause requests matching
    /// `phases` and optional `url_patterns`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::SessionNotStarted`] if no session is active.
    /// Returns [`SeleniumBaseError`] if registering the intercept fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use seleniumbase_rs::BaseCase;
    /// use thirtyfour::bidi::modules::network::InterceptPhase;
    ///
    /// # async fn demo(sb: BaseCase) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    /// let intercept = sb.bidi_add_intercept(
    ///     vec![InterceptPhase::BeforeRequestSent],
    ///     None,
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn bidi_add_intercept(
        &self,
        phases: Vec<InterceptPhase>,
        url_patterns: Option<Vec<serde_json::Value>>,
    ) -> Result<InterceptGuard, SeleniumBaseError> {
        self.session.bidi_add_intercept(phases, url_patterns).await
    }

    /// Continues an intercepted network request unmodified via BiDi `network.continueRequest`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::SessionNotStarted`] if no session is active.
    /// Returns [`SeleniumBaseError`] if the BiDi command fails.
    pub async fn bidi_continue_request(
        &self,
        request: RequestId,
    ) -> Result<(), SeleniumBaseError> {
        self.session.bidi_continue_request(request).await
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
        self.session.bidi_continue_response(request).await
    }

    /// Fails an intercepted network request via BiDi `network.failRequest`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::SessionNotStarted`] if no session is active.
    /// Returns [`SeleniumBaseError`] if the BiDi command fails.
    pub async fn bidi_fail_request(
        &self,
        request: RequestId,
    ) -> Result<(), SeleniumBaseError> {
        self.session.bidi_fail_request(request).await
    }
}

#[cfg(test)]
mod bidi_base_case_tests {
    use super::*;
    use crate::browser::config::BrowserConfig;

    #[tokio::test]
    async fn base_case_without_session_returns_session_not_started() {
        let sb = BaseCase::without_session(BrowserConfig::default());

        assert!(matches!(
            sb.bidi_listen_logs().await,
            Err(SeleniumBaseError::SessionNotStarted)
        ));
        assert!(matches!(
            sb.bidi_listen_network().await,
            Err(SeleniumBaseError::SessionNotStarted)
        ));
        assert!(matches!(
            sb.bidi_listen_requests().await,
            Err(SeleniumBaseError::SessionNotStarted)
        ));
        assert!(matches!(
            sb.bidi_listen_responses().await,
            Err(SeleniumBaseError::SessionNotStarted)
        ));
        assert!(matches!(
            sb.bidi_add_intercept(vec![], None).await,
            Err(SeleniumBaseError::SessionNotStarted)
        ));
        assert!(matches!(
            sb.bidi_continue_request(RequestId::from("req-1".to_string()))
                .await,
            Err(SeleniumBaseError::SessionNotStarted)
        ));
        assert!(matches!(
            sb.bidi_continue_response(RequestId::from("req-1".to_string()))
                .await,
            Err(SeleniumBaseError::SessionNotStarted)
        ));
        assert!(matches!(
            sb.bidi_fail_request(RequestId::from("req-1".to_string()))
                .await,
            Err(SeleniumBaseError::SessionNotStarted)
        ));
    }
}
