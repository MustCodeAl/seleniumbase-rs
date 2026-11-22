//! Context handed to every provider when generating its script.

use super::config::EvasionConfig;
use crate::evasion::fingerprint::Fingerprint;

/// Context handed to every provider when generating its script.
#[derive(Clone, Debug)]
pub struct EvasionContext<'a> {
    /// The session fingerprint being spoofed.
    pub fingerprint: &'a Fingerprint,
    /// Deterministic noise seed (see [`Fingerprint::seed`]).
    pub seed: u64,
    /// Runtime assembly configuration.
    pub config: EvasionConfig,
}

impl<'a> EvasionContext<'a> {
    /// Builds a context from a fingerprint, deriving the seed from it.
    pub fn new(fingerprint: &'a Fingerprint) -> Self {
        Self {
            seed: fingerprint.seed(),
            fingerprint,
            config: EvasionConfig::default(),
        }
    }

    /// Builds a context with an explicit config.
    pub fn with_config(fingerprint: &'a Fingerprint, config: EvasionConfig) -> Self {
        Self {
            seed: fingerprint.seed(),
            fingerprint,
            config,
        }
    }

    /// Escapes a string for safe interpolation inside single-quoted JS.
    ///
    /// Backslashes, single quotes, line breaks and the Unicode line and
    /// paragraph separators are escaped, so the text cannot end the literal or
    /// the line it is on.
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::evasion::providers::EvasionContext;
    ///
    /// assert_eq!(EvasionContext::escape("it's"), "it\\'s");
    /// ```
    pub fn escape(value: &str) -> String {
        value
            .replace('\\', "\\\\")
            .replace('\'', "\\'")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\u{2028}', "\\u2028")
            .replace('\u{2029}', "\\u2029")
    }

    /// Formats a number as a JavaScript numeric expression.
    ///
    /// Rust prints an infinite `f64` as `inf`, which JavaScript reads as an
    /// undefined variable. This prints `Infinity`, `-Infinity` and `NaN`, which
    /// it does understand, and finite values as they are.
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::evasion::providers::EvasionContext;
    ///
    /// assert_eq!(EvasionContext::number(1.5), "1.5");
    /// assert_eq!(EvasionContext::number(f64::INFINITY), "Infinity");
    /// assert_eq!(EvasionContext::number(f64::NEG_INFINITY), "-Infinity");
    /// assert_eq!(EvasionContext::number(f64::NAN), "NaN");
    /// ```
    pub fn number(value: f64) -> String {
        if value.is_nan() {
            "NaN".to_owned()
        } else if value.is_infinite() {
            if value > 0.0 { "Infinity" } else { "-Infinity" }.to_owned()
        } else {
            value.to_string()
        }
    }
}
