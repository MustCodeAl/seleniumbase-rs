//! Entering the frames a page script cannot read into.
//!
//! The page helper searches inside a frame by reading the frame's document,
//! which a script may do for a frame of the page's own origin. It cannot for a
//! `data:` frame, a sandboxed one, a frame of another origin, or a frame of
//! another site, which Chrome runs in a process of its own (an out-of-process
//! frame). The protocol reaches all of them, and in one of two ways:
//!
//! * A frame in the tab's own process has an execution context of its own, an
//!   isolated world made with `Page.createIsolatedWorld`, in which a script
//!   sees the frame's own document.
//! * An out-of-process frame is a target with a session of its own, attached
//!   with `Target.attachToTarget` (a frame's id is its target's id). A script
//!   sent to that session runs in the frame's main world.
//!
//! A [`Scope`] names where a script runs, and which frame element in the
//! scope around it holds the frame, so the frame's place in the tab's viewport
//! can be worked out for the mouse.

use std::sync::Arc;

use serde_json::{json, Value};

use super::page::{is_navigation_race, Page};
use super::types::Point;
use crate::error::SeleniumBaseError;

/// The name of the isolated world made in a frame of the tab's own process.
const WORLD: &str = "sbcdp";

/// Scrolls the frame element it is called on into view, and waits for the
/// browser to draw the result (two animation frames, at most a quarter of a
/// second), because a click is placed by what the browser last drew.
const REVEAL: &str = "function () { \
    this.scrollIntoView({ block: 'center', inline: 'center', behavior: 'instant' }); \
    return new Promise((resolve) => { \
        setTimeout(resolve, 250); \
        requestAnimationFrame(() => requestAnimationFrame(resolve)); \
    }); \
}";

/// Where a script runs: a protocol session, the execution context in it (the
/// main world of the session's page or frame when there is none), and, for a
/// frame, the element that holds the frame.
#[derive(Debug, Clone)]
pub(super) struct Scope {
    session: Arc<str>,
    context: Option<i64>,
    holder: Option<Arc<Holder>>,
}

/// The frame element that holds the frame a [`Scope`] is the content of.
#[derive(Debug)]
struct Holder {
    /// Where the frame element is.
    outer: Scope,
    /// The remote object of the frame element, a handle valid in `outer`.
    object: String,
    /// The frame's protocol id.
    frame: String,
}

impl Scope {
    /// The scope of a tab's own main frame, in `session`.
    pub(super) fn main(session: Arc<str>) -> Self {
        Self {
            session,
            context: None,
            holder: None,
        }
    }

    /// The protocol session the scope's commands go to.
    pub(super) fn session(&self) -> &str {
        &self.session
    }

    /// `params` of a `Runtime.evaluate` with the scope's execution context
    /// added, if it has one.
    pub(super) fn in_context(&self, mut params: Value) -> Value {
        if let (Some(context), Some(map)) = (self.context, params.as_object_mut()) {
            map.insert("contextId".to_owned(), json!(context));
        }
        params
    }

    /// The frame element that holds this scope's frame, with the scope that
    /// element is in.
    fn holder(&self) -> Option<&Holder> {
        self.holder.as_deref()
    }
}

/// Whether the page helper gave up on a chain because a frame on its way is
/// closed to a script, and the frame has to be entered through the protocol.
pub(super) fn is_frames_request(error: &SeleniumBaseError) -> bool {
    matches!(error, SeleniumBaseError::CdpDriver(message) if message.contains("sbcdp:frames"))
}

/// Whether `error` says a frame, its session or its context went away while it
/// was being used, which looking again may get past.
pub(super) fn is_frame_gone(error: &SeleniumBaseError) -> bool {
    matches!(error, SeleniumBaseError::CdpDriver(message)
        if is_navigation_race(message)
            || message.contains("Session with given id not found")
            || message.contains("No frame for given id")
            || message.contains("No target with given id")
            || message.contains("Could not find object")
            || message.contains("Cannot find object"))
}

impl Page {
    /// The scope of the tab's own main frame.
    pub(super) fn main_scope(&self) -> Scope {
        Scope::main(self.session_handle())
    }

    /// Which steps of the chain `chain` (JSON) the helper in `scope` could not
    /// search inside a frame for: each is the index of the step from which the
    /// search goes on inside that frame, in the order the frames were met.
    pub(super) async fn blocked_steps(
        &self,
        scope: &Scope,
        label: &str,
        chain: &str,
    ) -> Result<Vec<usize>, SeleniumBaseError> {
        let steps = self
            .eval_in(scope, label, &format!("__sbcdp.blockedSteps({chain})"))
            .await?;
        Ok(steps
            .as_array()
            .map(|steps| {
                steps
                    .iter()
                    .filter_map(|step| step.as_u64().and_then(|n| usize::try_from(n).ok()))
                    .collect()
            })
            .unwrap_or_default())
    }

    /// The remote object of the `index`-th frame element the helper in `scope`
    /// could not search inside, for the chain `chain` (JSON).
    pub(super) async fn frame_element(
        &self,
        scope: &Scope,
        chain: &str,
        index: usize,
    ) -> Result<Option<String>, SeleniumBaseError> {
        let response = self
            .send_in(
                scope,
                "Runtime.evaluate",
                scope.in_context(json!({
                    "expression": format!("__sbcdp.blockedFrame({chain}, {index})"),
                    "returnByValue": false,
                })),
            )
            .await?;
        if response.get("exceptionDetails").is_some() {
            return Ok(None);
        }
        Ok(response["result"]["objectId"].as_str().map(str::to_owned))
    }

    /// The scope that is the content of the frame element `object`, which is
    /// in `outer`, or `None` if there is no frame to enter: the element holds
    /// none, or it went away.
    pub(super) async fn enter_frame(
        &self,
        outer: &Scope,
        object: String,
    ) -> Result<Option<Scope>, SeleniumBaseError> {
        let described = self
            .send_in(outer, "DOM.describeNode", json!({ "objectId": object }))
            .await?;
        let Some(frame) = described["node"]["frameId"].as_str() else {
            return Ok(None);
        };
        // A frame of the process `outer` is in has a context to make; any other
        // frame is refused ("No frame for given id") and has a target of its own.
        let made = self
            .send_in(
                outer,
                "Page.createIsolatedWorld",
                json!({ "frameId": frame, "worldName": WORLD }),
            )
            .await;
        let (session, context) = match made {
            Ok(world) => match world["executionContextId"].as_i64() {
                Some(context) => (Arc::clone(&outer.session), Some(context)),
                None => return Ok(None),
            },
            Err(SeleniumBaseError::CdpDriver(_)) => {
                match self.browser().frame_session(frame).await? {
                    Some(session) => (session, None),
                    None => return Ok(None),
                }
            }
            Err(other) => return Err(other),
        };
        Ok(Some(Scope {
            session,
            context,
            holder: Some(Arc::new(Holder {
                outer: outer.clone(),
                object,
                frame: frame.to_owned(),
            })),
        }))
    }

    /// How far the top-left corner of `scope`'s content is from the top-left
    /// corner of the tab's viewport: the sum, over the frames between, of where
    /// each frame's content starts in the frame around it. The helper adds the
    /// frames it can read out to; this adds the ones it cannot.
    pub(super) async fn scope_origin(&self, scope: &Scope) -> Result<Point, SeleniumBaseError> {
        let mut origin = Point { x: 0.0, y: 0.0 };
        let mut at = scope;
        while let Some(holder) = at.holder() {
            let response = self
                .send_in(
                    &holder.outer,
                    "Runtime.callFunctionOn",
                    json!({
                        "objectId": holder.object,
                        "functionDeclaration": "function () { return __sbcdp.frameOrigin(this); }",
                        "returnByValue": true,
                    }),
                )
                .await?;
            if let Some(details) = response.get("exceptionDetails") {
                let text = details["exception"]["description"]
                    .as_str()
                    .or_else(|| details["text"].as_str())
                    .unwrap_or("script error");
                return Err(SeleniumBaseError::cdp_driver(text));
            }
            origin.x += response["result"]["value"]["x"]
                .as_f64()
                .unwrap_or_default();
            origin.y += response["result"]["value"]["y"]
                .as_f64()
                .unwrap_or_default();
            at = &holder.outer;
        }
        Ok(origin)
    }

    /// Scrolls each frame element on the way to `scope` into view, outermost
    /// first, so that a point inside the frame can be clicked. Scrolling an
    /// element inside an out-of-process frame does not bring the frame itself
    /// into view in the page around it.
    pub(super) async fn reveal_scope(&self, scope: &Scope) -> Result<(), SeleniumBaseError> {
        let mut holders = Vec::new();
        let mut at = scope;
        while let Some(holder) = at.holder() {
            holders.push(holder);
            at = &holder.outer;
        }
        for holder in holders.into_iter().rev() {
            self.send_in(
                &holder.outer,
                "Runtime.callFunctionOn",
                json!({
                    "objectId": holder.object,
                    "functionDeclaration": REVEAL,
                    "awaitPromise": true,
                }),
            )
            .await?;
        }
        Ok(())
    }

    /// Lets go of the handle of the frame element that holds `scope`'s frame.
    pub(super) async fn release_holder(&self, scope: &Scope) {
        if let Some(holder) = scope.holder() {
            self.release_object(&holder.outer, &holder.object).await;
        }
    }

    /// Lets go of the handles of every frame element on the way to `scope`.
    pub(super) async fn release_scope(&self, scope: &Scope) {
        let mut at = scope;
        while let Some(holder) = at.holder() {
            self.release_object(&holder.outer, &holder.object).await;
            at = &holder.outer;
        }
    }

    /// Forgets the sessions of the out-of-process frames on the way to
    /// `scope`, so the next use attaches again.
    pub(super) fn forget_scope(&self, scope: &Scope) {
        let mut at = scope;
        while let Some(holder) = at.holder() {
            if at.context.is_none() {
                self.browser().forget_frame_session(&holder.frame);
            }
            at = &holder.outer;
        }
    }

    /// Releases one remote object. A handle that is already gone is no error.
    async fn release_object(&self, scope: &Scope, object: &str) {
        let _ = self
            .send_in(
                scope,
                "Runtime.releaseObject",
                json!({ "objectId": object }),
            )
            .await;
    }
}
