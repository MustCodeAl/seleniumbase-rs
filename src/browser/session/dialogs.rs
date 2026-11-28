//! JavaScript dialogs (`alert`, `confirm`, `prompt`) on a direct CDP attachment.
//!
//! WebDriver keeps the open dialog in the browser, and a command that opened
//! one returns. Over DevTools the page stalls inside the command until the
//! dialog is handled. A [`DialogWatch`] listens for the dialog events of one
//! tab, so the session knows which dialog is open, and a command that opens
//! one is let go ([`DialogWatch::unless_dialog`]), as WebDriver lets it go.
//! [`PureDialogs`] answers the open dialog (`Page.handleJavaScriptDialog`).

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::error::SeleniumBaseError;
use crate::sb_cdp::Page;

/// The dialog a tab has open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OpenDialog {
    /// `alert`, `confirm`, `prompt` or `beforeunload`.
    pub(crate) kind: String,
    /// What the dialog says.
    pub(crate) message: String,
    /// What a prompt holds before anything is typed.
    pub(crate) default_prompt: String,
    /// What was typed into a prompt since it opened.
    pub(crate) typed: Option<String>,
}

impl OpenDialog {
    fn from_event(params: &Value) -> Self {
        Self {
            kind: params["type"].as_str().unwrap_or("alert").to_owned(),
            message: params["message"].as_str().unwrap_or_default().to_owned(),
            default_prompt: params["defaultPrompt"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            typed: None,
        }
    }
}

/// Follows the dialogs of one tab for as long as it lives.
pub(crate) struct DialogWatch {
    open: watch::Sender<Option<OpenDialog>>,
    listener: Mutex<Option<JoinHandle<()>>>,
}

impl Drop for DialogWatch {
    fn drop(&mut self) {
        let listener = crate::common::poison::locked(&self.listener).take();
        if let Some(listener) = listener {
            listener.abort();
        }
    }
}

impl DialogWatch {
    /// A watch that follows no tab: nothing ever opens.
    pub(crate) fn idle() -> Arc<Self> {
        Arc::new(Self {
            open: watch::channel(None).0,
            listener: Mutex::new(None),
        })
    }

    /// Starts following the dialogs of `page`, from now on. Without a Tokio
    /// runtime to run the listener on, nothing is followed.
    pub(crate) fn start(page: &Page) -> Arc<Self> {
        let this = Self::idle();
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return this;
        };
        // Subscribed before the task starts, so no event slips by.
        let mut events = page.events();
        let follower = Arc::downgrade(&this);
        let listener = runtime.spawn(async move {
            while let Some(event) = events.next().await {
                let Some(watch) = follower.upgrade() else {
                    break;
                };
                match event.method.as_str() {
                    "Page.javascriptDialogOpening" => {
                        watch
                            .open
                            .send_replace(Some(OpenDialog::from_event(&event.params)));
                    }
                    "Page.javascriptDialogClosed" => {
                        watch.open.send_replace(None);
                    }
                    _ => {}
                }
            }
        });
        *crate::common::poison::locked(&this.listener) = Some(listener);
        this
    }

    /// The dialog that is open now.
    pub(crate) fn current(&self) -> Option<OpenDialog> {
        self.open.borrow().clone()
    }

    /// Records what was typed into the open prompt.
    ///
    /// # Errors
    ///
    /// Fails like WebDriver's `no such alert` when nothing is open, and when
    /// the dialog is not a prompt.
    pub(crate) fn type_into_prompt(&self, text: &str) -> Result<(), SeleniumBaseError> {
        let mut result = Err(no_such_alert());
        self.open.send_if_modified(|open| match open {
            Some(dialog) if dialog.kind == "prompt" => {
                dialog.typed = Some(text.to_owned());
                result = Ok(());
                true
            }
            Some(dialog) => {
                result = Err(SeleniumBaseError::CdpDriver(format!(
                    "element not interactable: the open {} is not a prompt",
                    dialog.kind
                )));
                false
            }
            None => false,
        });
        result
    }

    /// Forgets the open dialog, as soon as it was answered.
    fn answered(&self) {
        self.open.send_replace(None);
    }

    /// Runs `action`, unless it opens a dialog first: then `None`, as a
    /// WebDriver command returns once the dialog it opened is there.
    ///
    /// A page stalls inside the command that opened a dialog, so awaiting the
    /// command alone would wait for a dialog nobody has answered.
    ///
    /// # Errors
    ///
    /// The action's error, and, without running it, an error like WebDriver's
    /// `unexpected alert open` when a dialog is already open.
    pub(crate) async fn unless_dialog<T>(
        &self,
        action: impl Future<Output = Result<T, SeleniumBaseError>>,
    ) -> Result<Option<T>, SeleniumBaseError> {
        if let Some(open) = self.current() {
            return Err(SeleniumBaseError::CdpDriver(format!(
                "unexpected alert open: {{Alert text : {}}}",
                open.message
            )));
        }
        let mut changes = self.open.subscribe();
        tokio::select! {
            outcome = action => outcome.map(Some),
            _ = changes.wait_for(Option::is_some) => Ok(None),
        }
    }
}

/// WebDriver's answer for an alert command with no alert open.
fn no_such_alert() -> SeleniumBaseError {
    SeleniumBaseError::CdpDriver(
        "no such alert: no JavaScript dialog is open on this page".to_owned(),
    )
}

/// The dialogs of the tab a direct CDP attachment drives, and the tab to answer
/// them on.
#[derive(Clone)]
pub(crate) struct PureDialogs {
    page: Page,
    watch: Arc<DialogWatch>,
}

impl PureDialogs {
    pub(super) fn new(page: Page, watch: Arc<DialogWatch>) -> Self {
        Self { page, watch }
    }

    /// The open dialog.
    ///
    /// # Errors
    ///
    /// A `no such alert` error when none is open.
    pub(crate) fn open(&self) -> Result<OpenDialog, SeleniumBaseError> {
        self.watch.current().ok_or_else(no_such_alert)
    }

    /// What the open dialog says.
    ///
    /// # Errors
    ///
    /// A `no such alert` error when none is open.
    pub(crate) fn text(&self) -> Result<String, SeleniumBaseError> {
        Ok(self.open()?.message)
    }

    /// Types `text` into the open prompt, for the next accept.
    ///
    /// # Errors
    ///
    /// A `no such alert` error when none is open, and an error when the open
    /// dialog is not a prompt.
    pub(crate) fn send_text(&self, text: &str) -> Result<(), SeleniumBaseError> {
        self.watch.type_into_prompt(text)
    }

    /// Accepts the open dialog (OK), with the text typed into a prompt.
    ///
    /// # Errors
    ///
    /// A `no such alert` error when none is open, or the browser's error.
    pub(crate) async fn accept(&self) -> Result<(), SeleniumBaseError> {
        let open = self.open()?;
        let mut params = json!({ "accept": true });
        if open.kind == "prompt" {
            params["promptText"] = json!(open.typed.unwrap_or(open.default_prompt));
        }
        self.page
            .execute("Page.handleJavaScriptDialog", params)
            .await?;
        self.watch.answered();
        Ok(())
    }

    /// Dismisses the open dialog (Cancel).
    ///
    /// # Errors
    ///
    /// A `no such alert` error when none is open, or the browser's error.
    pub(crate) async fn dismiss(&self) -> Result<(), SeleniumBaseError> {
        self.open()?;
        self.page
            .execute("Page.handleJavaScriptDialog", json!({ "accept": false }))
            .await?;
        self.watch.answered();
        Ok(())
    }
}
