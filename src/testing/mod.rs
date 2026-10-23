//! The `sb` and `sb_cdp` fixtures for `cargo test` (feature `testing`).

mod deadline;
mod evidence;

pub(crate) use deadline::{exceeded_message, Deadline};
