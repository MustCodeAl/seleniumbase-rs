//! Tracing initialization helpers.
//!
//! These helpers wire together the tracing ecosystem used by `seleniumbase-rs`:
//!
//! * [`tracing-subscriber`] for human-readable or JSON logs.
//! * [`tracing-log`] to capture legacy `log` records as tracing events.
//! * [`json-subscriber`] for structured JSON output.
//! * [`tracing-timing`] (when the `full-tracing` feature is enabled) for
//!   histogram timing of spans/events.
//!
//! Logs go to standard error, never to standard output. Standard output is for
//! what a program produces: the answer of an `sbrs` command, or the JSON-RPC
//! messages of `sbrs-mcp`, which a log line in the stream would break.
//! Colour is used only when standard error is a terminal and `NO_COLOR` is not
//! set.
//!
//! # Example
//!
//! ```no_run
//! use seleniumbase_rs::tracing_util;
//!
//! fn main() {
//!     tracing_util::init_tracing();
//!     // or, for JSON logs:
//!     // tracing_util::init_tracing_json();
//! }
//! ```

use std::io::IsTerminal;

use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

use crate::config::{LogFormat, RuntimeConfig};

/// The filter named by `RUST_LOG`, or `info` if it is unset or invalid.
fn env_filter() -> EnvFilter {
    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))
}

/// The filter `directives` describes, or the `RUST_LOG` one if they are invalid.
///
/// Built directly, so the process environment is left alone: changing it from
/// a running program is not safe while other threads read it.
fn filter_from(directives: &str) -> EnvFilter {
    EnvFilter::try_new(directives).unwrap_or_else(|_| env_filter())
}

/// Whether log lines may carry colour: standard error is a terminal and the
/// user has not asked for none (<https://no-color.org>).
fn colour_allowed() -> bool {
    std::io::stderr().is_terminal()
        && std::env::var_os("NO_COLOR").is_none_or(|value| value.is_empty())
}

/// Installs the global subscriber, ignoring a second call.
fn install(filter: EnvFilter, format: &LogFormat) {
    let _ = tracing_log::LogTracer::init();
    let registry = tracing_subscriber::registry();
    match format {
        LogFormat::Json => {
            let layer = tracing_subscriber::fmt::layer()
                .json()
                .with_writer(std::io::stderr)
                .with_current_span(true)
                .with_span_list(false)
                .with_filter(filter);
            finish(registry.with(layer));
        }
        LogFormat::Pretty => {
            let layer = tracing_subscriber::fmt::layer()
                .with_writer(std::io::stderr)
                .with_ansi(colour_allowed())
                .with_target(true)
                .with_thread_ids(false)
                .with_filter(filter);
            finish(registry.with(layer));
        }
    }
}

/// Adds the timing layer when it is built in, and sets the subscriber.
fn finish<S>(subscriber: S)
where
    S: tracing::Subscriber
        + for<'span> tracing_subscriber::registry::LookupSpan<'span>
        + Send
        + Sync
        + 'static,
{
    #[cfg(feature = "full-tracing")]
    let subscriber = subscriber.with(timing_layer());
    // tokio-console attaches on 127.0.0.1:6669; the layer filters on its own.
    #[cfg(feature = "tokio-console")]
    let subscriber = subscriber.with(console_subscriber::spawn());
    let _ = subscriber.try_init();
}

/// Initialize tracing from the current [`RuntimeConfig`].
///
/// Honors `SB_LOG_LEVEL` and `SB_LOG_FORMAT` so logs are treated as an
/// environment-driven event stream (Twelve-Factor XI).
pub fn init_tracing_from_runtime(config: &RuntimeConfig) {
    install(EnvFilter::new(&config.log_level), &config.log_format);
}

/// Install a plain text tracing subscriber and bridge `log` records.
///
/// Reads the `RUST_LOG` environment variable and defaults to `info`.
/// Calling this more than once in the same process is ignored.
pub fn init_tracing() {
    install(env_filter(), &LogFormat::Pretty);
}

/// Install a JSON tracing subscriber and bridge `log` records.
///
/// Reads the `RUST_LOG` environment variable and defaults to `info`.
/// Calling this more than once in the same process is ignored.
pub fn init_tracing_json() {
    install(env_filter(), &LogFormat::Json);
}

#[cfg(feature = "full-tracing")]
fn timing_layer() -> tracing_timing::TimingLayer {
    tracing_timing::Builder::default().layer(|| {
        tracing_timing::Histogram::new_with_max(1_000_000_000, 2)
            .expect("failed to create timing histogram")
    })
}

/// Initialize tracing with a custom [`EnvFilter`] string.
///
/// This is useful for binaries that want to accept a `--log-level` flag and
/// still inherit the rest of the default subscriber configuration. A filter
/// that does not parse falls back to `RUST_LOG`, then to `info`. The process
/// environment is not changed.
///
/// # Example
///
/// ```no_run
/// use seleniumbase_rs::tracing_util;
///
/// fn main() {
///     tracing_util::init_tracing_with_filter("seleniumbase_rs=debug,info");
/// }
/// ```
pub fn init_tracing_with_filter(filter: &str) {
    install(filter_from(filter), &LogFormat::Pretty);
}

/// Initialize JSON tracing with a custom [`EnvFilter`] string.
///
/// See [`init_tracing_with_filter`] for how the filter is read.
pub fn init_tracing_json_with_filter(filter: &str) {
    install(filter_from(filter), &LogFormat::Json);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_filter_string_is_used_without_touching_the_environment() {
        let before = std::env::var_os("RUST_LOG");
        let filter = filter_from("seleniumbase_rs=debug,info");
        assert_eq!(std::env::var_os("RUST_LOG"), before);
        let shown = filter.to_string();
        assert!(shown.contains("seleniumbase_rs=debug"), "{shown}");
    }

    #[test]
    fn an_unparsable_filter_falls_back_instead_of_failing() {
        // Nothing to assert beyond not panicking and getting some filter back.
        let _ = filter_from("=== not a filter ===");
    }
}
