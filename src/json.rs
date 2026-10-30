//! JSON values, under this crate's name.
//!
//! The API trades in `serde_json` values: [`BaseCase::add_cookie`](crate::BaseCase::add_cookie)
//! takes one, [`BaseCase::execute_script`](crate::BaseCase::execute_script) returns one, and
//! so do the pure CDP engine's `Page::evaluate` and the CDP commands. Import the type and
//! the [`json!`] macro from here and a program needs no `serde_json` line in its manifest:
//!
//! ```
//! use seleniumbase_rs::json::{json, Value};
//!
//! let cookie: Value = json!({ "name": "session", "value": "abc123" });
//! assert_eq!(cookie["name"], "session");
//! ```
//!
//! Only the parts this crate's own signatures and its documentation use are listed. A program
//! that derives `Serialize` or `Deserialize` for its own types needs `serde` itself, and one
//! that needs a `serde_json` feature (`preserve_order`, `arbitrary_precision`) lists the
//! crate.

pub use serde_json::{
    from_slice, from_str, from_value, json, to_string, to_string_pretty, to_value, Error, Map,
    Number, Value,
};
