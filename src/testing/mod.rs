//! The `sb` and `sb_cdp` fixtures for `cargo test` (feature `testing`).

mod deadline;
mod evidence;
mod lifecycle;

pub use lifecycle::{AttemptEnd, Finalizer, Outcome};

pub(crate) use deadline::{exceeded_message, Deadline};
