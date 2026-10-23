//! The `sb` and `sb_cdp` fixtures for `cargo test` (feature `testing`).

mod deadline;

pub(crate) use deadline::{exceeded_message, Deadline};
