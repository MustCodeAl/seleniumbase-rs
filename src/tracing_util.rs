//! Tracing initialization helpers.
//!
//! These helpers wire together the tracing ecosystem used by `seleniumbase-rs`:
//!
//! * [`tracing-subscriber`] for human-readable or JSON logs.
//! * [`tracing-log`] to capture legacy `log` records as tracing events.
//! * JSON lines, from `tracing-subscriber`'s own JSON formatter.
//! * [`tracing-timing`] (when the `full-tracing` feature is enabled) for
//!   histogram timing of spans/events.
//!
//! Logs go to standard error, never to standard output. Standard output is for
//! what a program produces: the answer of an `sbrs` command, or the JSON-RPC
//! messages of `sbrs-mcp`, which a log line in the stream would break.
//!
//! # What a log line looks like
//!
//! ```text
//! 12:34:56  INFO browser::session: patched chrome binary ready path=/tmp/chrome
//! ```
//!
//! The readable format colours the level, dims the time, the module path and
//! the field names, and highlights file paths and steps. Colour is used only
//! when standard error is a terminal, `NO_COLOR` is not set and `TERM` is not
//! `dumb` (see [`common::color`](crate::common::color)); a pipe, a file and
//! the JSON format (`SB_LOG_FORMAT=json`) never carry an escape sequence.
//!
//! By default a line shows the module path without the crate name, no span
//! context (the `new:connect:` that `#[instrument]` puts in front of every
//! line logged inside a function) and whole seconds. When a filter was chosen
//! on purpose, through `RUST_LOG` or `SB_LOG_LEVEL`, the line shows the whole
//! target, the span context and milliseconds instead.
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

mod pretty;

use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

use crate::common::color::use_color;
use crate::config::{LogFormat, RuntimeConfig};
use pretty::{Detail, LineFormat};

/// The filter `directives` describes, if they are not blank and parse.
fn parse_directives(directives: Option<&str>) -> Option<EnvFilter> {
    let directives = directives.map(str::trim).filter(|text| !text.is_empty())?;
    EnvFilter::try_new(directives).ok()
}

/// The filter for the first of `primary` and `rust_log` that is set and
/// parses, else `info`. A blank value counts as unset, because an empty filter
/// would silence everything but errors.
fn filter_for(primary: Option<&str>, rust_log: Option<&str>) -> EnvFilter {
    parse_directives(primary)
        .or_else(|| parse_directives(rust_log))
        .unwrap_or_else(|| EnvFilter::new("info"))
}

/// How much of a line to show: everything when `primary` or `rust_log` is a
/// filter that someone set (and that parses), else the compact line.
fn detail_for(primary: Option<&str>, rust_log: Option<&str>) -> Detail {
    if parse_directives(primary)
        .or_else(|| parse_directives(rust_log))
        .is_some()
    {
        Detail::Full
    } else {
        Detail::Compact
    }
}

/// The filter and the detail for `primary`, then `RUST_LOG`, then the defaults.
///
/// Built from the values, so the process environment is left alone: changing
/// it from a running program is not safe while other threads read it.
fn setup_from(primary: Option<&str>, rust_log: Option<&str>) -> (EnvFilter, Detail) {
    (filter_for(primary, rust_log), detail_for(primary, rust_log))
}

/// [`setup_from`] with `RUST_LOG` read from the process environment.
fn setup_from_env(primary: Option<&str>) -> (EnvFilter, Detail) {
    setup_from(primary, std::env::var("RUST_LOG").ok().as_deref())
}

/// Whether log lines may carry colour: standard error is a terminal and the
/// user has not asked for none (<https://no-color.org>).
fn colour_allowed() -> bool {
    use_color(&std::io::stderr())
}

/// The layer of the readable format, writing to `writer`.
fn pretty_layer<S, W>(
    writer: W,
    ansi: bool,
    detail: Detail,
) -> Box<dyn Layer<S> + Send + Sync + 'static>
where
    S: tracing::Subscriber + for<'span> LookupSpan<'span>,
    W: for<'writer> MakeWriter<'writer> + Send + Sync + 'static,
{
    tracing_subscriber::fmt::layer()
        .event_format(LineFormat::new(detail))
        .with_writer(writer)
        .with_ansi(ansi)
        .boxed()
}

/// The layer of JSON lines, writing to `writer`. JSON carries no colour.
fn json_layer<S, W>(writer: W) -> Box<dyn Layer<S> + Send + Sync + 'static>
where
    S: tracing::Subscriber + for<'span> LookupSpan<'span>,
    W: for<'writer> MakeWriter<'writer> + Send + Sync + 'static,
{
    tracing_subscriber::fmt::layer()
        .json()
        .with_writer(writer)
        .with_current_span(true)
        .with_span_list(false)
        .boxed()
}

/// Installs the global subscriber, ignoring a second call.
fn install((filter, detail): (EnvFilter, Detail), format: &LogFormat) {
    let _ = tracing_log::LogTracer::init();
    let registry = tracing_subscriber::registry();
    match format {
        LogFormat::Json => {
            let layer = json_layer(std::io::stderr).with_filter(filter);
            finish(registry.with(layer));
        }
        LogFormat::Pretty => {
            let layer = pretty_layer(std::io::stderr, colour_allowed(), detail).with_filter(filter);
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
/// environment-driven event stream (Twelve-Factor XI). The filter is
/// `SB_LOG_LEVEL`, else `RUST_LOG`, else `info`; a value that does not parse
/// falls through to the next. A filter chosen this way also turns on the full
/// line (whole target, span context, milliseconds).
pub fn init_tracing_from_runtime(config: &RuntimeConfig) {
    install(
        setup_from_env(config.log_level.as_deref()),
        &config.log_format,
    );
}

/// Install a plain text tracing subscriber and bridge `log` records.
///
/// Reads the `RUST_LOG` environment variable and defaults to `info`.
/// Calling this more than once in the same process is ignored.
pub fn init_tracing() {
    install(setup_from_env(None), &LogFormat::Pretty);
}

/// Install a JSON tracing subscriber and bridge `log` records.
///
/// Reads the `RUST_LOG` environment variable and defaults to `info`.
/// Calling this more than once in the same process is ignored.
pub fn init_tracing_json() {
    install(setup_from_env(None), &LogFormat::Json);
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
    install(setup_from_env(Some(filter)), &LogFormat::Pretty);
}

/// Initialize JSON tracing with a custom [`EnvFilter`] string.
///
/// See [`init_tracing_with_filter`] for how the filter is read.
pub fn init_tracing_json_with_filter(filter: &str) {
    install(setup_from_env(Some(filter)), &LogFormat::Json);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;
    use std::sync::{Arc, Mutex};

    /// A writer that keeps what a layer prints.
    #[derive(Clone, Default)]
    struct Capture(Arc<Mutex<Vec<u8>>>);

    impl io::Write for Capture {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl Capture {
        fn text(&self) -> String {
            String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
        }
    }

    /// What `emit` logs through `layer`, as it is printed.
    fn logged(
        layer: Box<dyn Layer<tracing_subscriber::Registry> + Send + Sync>,
        emit: impl FnOnce(),
    ) {
        let subscriber = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(subscriber, emit);
    }

    #[test]
    fn a_filter_string_is_used_without_touching_the_environment() {
        let before = std::env::var_os("RUST_LOG");
        let (filter, _) = setup_from_env(Some("seleniumbase_rs=debug,info"));
        assert_eq!(std::env::var_os("RUST_LOG"), before);
        let shown = filter.to_string();
        assert!(shown.contains("seleniumbase_rs=debug"), "{shown}");
    }

    fn shown(filter: &EnvFilter) -> String {
        filter.to_string()
    }

    #[test]
    fn the_filter_comes_from_sb_log_level_then_rust_log_then_info() {
        let both = shown(&filter_for(Some("debug"), Some("warn")));
        assert!(both.contains("debug") && !both.contains("warn"), "{both}");
        let rust_log = shown(&filter_for(None, Some("warn")));
        assert!(rust_log.contains("warn"), "{rust_log}");
        let neither = shown(&filter_for(None, None));
        assert!(neither.contains("info"), "{neither}");
    }

    #[test]
    fn a_blank_or_invalid_value_falls_through_to_the_next() {
        for primary in ["", "   ", "=== not a filter ==="] {
            let filter = shown(&filter_for(Some(primary), Some("warn")));
            assert!(filter.contains("warn"), "{primary:?}: {filter}");
        }
        let blank_rust_log = shown(&filter_for(Some(""), Some(" ")));
        assert!(blank_rust_log.contains("info"), "{blank_rust_log}");
    }

    #[test]
    fn an_unparsable_filter_falls_back_instead_of_failing() {
        // Nothing to assert beyond not panicking and getting some filter back.
        let _ = setup_from_env(Some("=== not a filter ==="));
    }

    #[test]
    fn the_full_line_is_shown_only_when_a_filter_was_chosen() {
        assert_eq!(detail_for(None, None), Detail::Compact);
        assert_eq!(
            detail_for(Some("debug"), None),
            Detail::Full,
            "SB_LOG_LEVEL"
        );
        assert_eq!(
            detail_for(None, Some("seleniumbase_rs=trace")),
            Detail::Full,
            "RUST_LOG"
        );
        for blank_or_bad in ["", "  ", "=== not a filter ==="] {
            assert_eq!(
                detail_for(Some(blank_or_bad), Some(blank_or_bad)),
                Detail::Compact,
                "{blank_or_bad:?} is not a choice"
            );
        }
    }

    #[test]
    fn the_readable_layer_is_plain_when_colour_is_off() {
        let capture = Capture::default();
        let sink = capture.clone();
        logged(
            pretty_layer(move || sink.clone(), false, Detail::Compact),
            || {
                let _span = tracing::info_span!("connect", url = "ws://x").entered();
                tracing::warn!(path = "/tmp/x", "slow");
            },
        );
        let text = capture.text();
        assert!(!text.contains('\x1b'), "{text:?}");
        assert!(
            text.contains("WARN") && text.contains("slow path=/tmp/x"),
            "{text}"
        );
        assert!(
            !text.contains("connect"),
            "no span noise by default: {text}"
        );
    }

    #[test]
    fn the_readable_layer_is_coloured_when_colour_is_on() {
        let capture = Capture::default();
        let sink = capture.clone();
        logged(
            pretty_layer(move || sink.clone(), true, Detail::Compact),
            || tracing::warn!("slow"),
        );
        let text = capture.text();
        assert!(text.contains("\x1b[1;33m WARN\x1b[0m"), "{text:?}");
        assert!(text.contains("\x1b[2m"), "the time is dim: {text:?}");
    }

    #[test]
    fn json_lines_carry_no_colour() {
        let capture = Capture::default();
        let sink = capture.clone();
        logged(json_layer(move || sink.clone()), || {
            tracing::error!(path = "/tmp/x", "failed");
        });
        let text = capture.text();
        assert!(!text.contains('\x1b'), "{text:?}");
        let line: serde_json::Value = serde_json::from_str(text.trim()).unwrap();
        assert_eq!(line["level"], "ERROR");
        assert_eq!(line["fields"]["message"], "failed");
        assert_eq!(line["fields"]["path"], "/tmp/x");
    }
}
