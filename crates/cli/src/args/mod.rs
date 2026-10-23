//! The arguments of the commands that have more than a few of their own, one module per area.
//!
//! A struct here is a command's `Args` type, which keeps the parser's code for each command in
//! a function of its own (see the note on `PromptArgs` in `sbrs.rs`).

pub(crate) mod tools;
