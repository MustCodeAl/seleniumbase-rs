//! Method names in ten languages, for tests written in a language other than
//! English.
//!
//! [`master_dict`] is the vocabulary: one row per `BaseCase` method, with its
//! word in every [`Language`]. [`translator::translate_action`] looks a name up
//! by language code or name; each per-language module (`chinese`, `dutch`, ...)
//! offers the same lookup for one fixed language. A name the dictionary does not
//! know is returned unchanged. [`scenario`] translates the action names of a
//! JSON scenario file, and `sbase translate` does so from the command line.
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::utils::translate::{translator::translate_action, Language};
//!
//! assert_eq!(translate_action("click", "es"), "clic");
//!
//! let language: Language = "Japanese".parse()?;
//! assert_eq!(language.code(), "ja");
//! # Ok::<(), seleniumbase_rs::utils::translate::ParseLanguageError>(())
//! ```

pub mod chinese;
pub mod dutch;
pub mod english;
pub mod french;
pub mod italian;
pub mod japanese;
pub mod korean;
pub mod language;
pub mod master_dict;
pub mod portuguese;
pub mod russian;
pub mod scenario;
pub mod spanish;
pub mod translator;

pub use language::{Language, ParseLanguageError};
