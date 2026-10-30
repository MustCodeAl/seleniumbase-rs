//! The readable log line of `init_tracing*`, `sbrs` and `sbrs-mcp`.
//!
//! One event is one line:
//!
//! ```text
//! 12:34:56  INFO browser::session: patched chrome binary ready path=/tmp/chrome
//! ```
//!
//! With colour the time, the module path and the field names are dim, the
//! level has its own colour (red `ERROR`, yellow `WARN`, green `INFO`, blue
//! `DEBUG`, magenta `TRACE`), file paths are cyan and tests and steps bold
//! magenta. Without it, which is the case for a pipe, a file, `NO_COLOR` and a
//! dumb terminal, the same line carries no escape sequence.
//!
//! The default ([`Detail::Compact`]) leaves out what makes a terminal noisy:
//! the span context (`new:connect:` in front of every line that a
//! `#[instrument]`ed function logs), the crate name in front of the module
//! path, and the fractions of the second. [`Detail::Full`], which is used when
//! someone asked for a filter (`RUST_LOG`, `SB_LOG_LEVEL`), shows them.
//!
//! Text that came from outside (a page title in a message, a header in a
//! field) is [sanitized](crate::common::color::sanitize), so it cannot send an
//! escape sequence to the terminal.

use std::fmt;

use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_log::NormalizeEvent;
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::time::FormatTime;
use tracing_subscriber::fmt::{FmtContext, FormatEvent, FormatFields, FormattedFields};
use tracing_subscriber::registry::LookupSpan;

use crate::common::color::{palette, sanitize, Style};

/// How much a log line shows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Detail {
    /// The module path without the crate name, no span context, whole seconds.
    Compact,
    /// The whole target, the span context, milliseconds.
    Full,
}

/// The local wall-clock time of day.
#[derive(Clone, Copy, Debug)]
pub(super) struct Clock {
    millis: bool,
}

impl FormatTime for Clock {
    fn format_time(&self, w: &mut Writer<'_>) -> fmt::Result {
        let now = chrono::Local::now();
        if self.millis {
            write!(w, "{}", now.format("%H:%M:%S%.3f"))
        } else {
            write!(w, "{}", now.format("%H:%M:%S"))
        }
    }
}

/// The event formatter: `time LEVEL [spans] target: message key=value ...`.
#[derive(Clone, Debug)]
pub(super) struct LineFormat<T = Clock> {
    timer: T,
    detail: Detail,
}

impl LineFormat {
    /// The formatter for `detail`, stamping with the local time.
    pub(super) fn new(detail: Detail) -> Self {
        Self::with_timer(
            Clock {
                millis: detail == Detail::Full,
            },
            detail,
        )
    }
}

impl<T> LineFormat<T> {
    /// The formatter for `detail`, stamping with `timer`.
    pub(super) fn with_timer(timer: T, detail: Detail) -> Self {
        Self { timer, detail }
    }
}

impl<S, N, T> FormatEvent<S, N> for LineFormat<T>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
    T: FormatTime,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let color = writer.has_ansi_escapes();
        // A record of the `log` crate keeps its own target and level.
        let normalized = event.normalized_metadata();
        let meta = normalized.as_ref().unwrap_or_else(|| event.metadata());

        let mut stamp = String::new();
        self.timer.format_time(&mut Writer::new(&mut stamp))?;
        palette::DIM.write(&mut writer, &stamp, color)?;
        writer.write_char(' ')?;
        let level = meta.level();
        level_style(level).write(&mut writer, format_args!("{:>5}", level.as_str()), color)?;
        writer.write_char(' ')?;

        if self.detail == Detail::Full {
            write_spans(ctx, &mut writer, color)?;
        }
        let target = match self.detail {
            Detail::Compact => compact_target(meta.target()),
            Detail::Full => meta.target(),
        };
        palette::DIM.write(&mut writer, format_args!("{target}:"), color)?;

        let mut collected = Collected::default();
        event.record(&mut collected);
        if let Some(message) = &collected.message {
            writer.write_char(' ')?;
            writer.write_str(&sanitize(message))?;
        }
        for (key, value) in &collected.fields {
            writer.write_char(' ')?;
            palette::DIM.write(&mut writer, format_args!("{key}="), color)?;
            value_style(key).write(&mut writer, sanitize(value), color)?;
        }
        writeln!(writer)
    }
}

/// `connect{url="x"}:` for every span the event is in, outermost first, then a space.
fn write_spans<S, N>(
    ctx: &FmtContext<'_, S, N>,
    writer: &mut Writer<'_>,
    color: bool,
) -> fmt::Result
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    let Some(scope) = ctx.event_scope() else {
        return Ok(());
    };
    let mut any = false;
    for span in scope.from_root() {
        any = true;
        Style::PLAIN
            .bold()
            .write(&mut *writer, span.name(), color)?;
        let extensions = span.extensions();
        if let Some(fields) = extensions.get::<FormattedFields<N>>() {
            if !fields.is_empty() {
                // Formatted by the layer's own field formatter, which escapes
                // what it prints; its ANSI codes are not ours to sanitize.
                write!(writer, "{{{fields}}}")?;
            }
        }
        writer.write_char(':')?;
    }
    if any {
        writer.write_char(' ')?;
    }
    Ok(())
}

/// The target without the crate's own name: `browser::session` for
/// `seleniumbase_rs::browser::session`.
fn compact_target(target: &str) -> &str {
    target.strip_prefix("seleniumbase_rs::").unwrap_or(target)
}

/// The colour of a level's name.
fn level_style(level: &Level) -> Style {
    match *level {
        Level::ERROR => palette::ERROR,
        Level::WARN => palette::WARNING,
        Level::INFO => palette::SUCCESS,
        Level::DEBUG => palette::DETAIL,
        Level::TRACE => palette::TRACE,
    }
}

/// Field names whose values are file or folder paths.
const PATH_KEYS: [&str; 10] = [
    "path",
    "file",
    "dest",
    "source",
    "dir",
    "folder",
    "recording",
    "executable",
    "binary",
    "log",
];

/// Field names whose values are tests or steps.
const STEP_KEYS: [&str; 5] = ["step", "steps", "test", "scenario", "action"];

/// The colour of the value of the field `key`: paths and steps stand out.
fn value_style(key: &str) -> Style {
    if PATH_KEYS.contains(&key)
        || key.ends_with("_path")
        || key.ends_with("_file")
        || key.ends_with("_dir")
    {
        palette::PATH
    } else if STEP_KEYS.contains(&key) {
        palette::STEP
    } else {
        Style::PLAIN
    }
}

/// An event's message and the rest of its fields, as text, in order.
#[derive(Default)]
struct Collected {
    message: Option<String>,
    fields: Vec<(String, String)>,
}

impl Collected {
    fn push(&mut self, field: &Field, value: String) {
        let name = field.name();
        // The `log` crate's own metadata, already shown as target and level.
        if name.starts_with("log.") {
            return;
        }
        if name == "message" {
            self.message = Some(value);
        } else {
            let name = name.strip_prefix("r#").unwrap_or(name);
            self.fields.push((name.to_owned(), value));
        }
    }
}

impl Visit for Collected {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.push(field, format!("{value:?}"));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.push(field, value.to_owned());
        } else {
            self.push(field, bare_or_quoted(value));
        }
    }
}

/// A string field as it is when it is one plain word, else quoted and escaped.
fn bare_or_quoted(value: &str) -> String {
    let plain = !value.is_empty()
        && !value
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || c == '"');
    if plain {
        value.to_owned()
    } else {
        format!("{value:?}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;
    use std::sync::{Arc, Mutex};
    use tracing_subscriber::layer::SubscriberExt;

    /// A writer that keeps what the layer prints.
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

    /// A clock that always reads 12:34:56, so a line can be compared whole.
    struct FixedClock;

    impl FormatTime for FixedClock {
        fn format_time(&self, w: &mut Writer<'_>) -> fmt::Result {
            w.write_str("12:34:56")
        }
    }

    /// What `emit` logs, as the layer prints it.
    fn render(ansi: bool, detail: Detail, emit: impl FnOnce()) -> String {
        let capture = Capture::default();
        let sink = capture.clone();
        let layer = tracing_subscriber::fmt::layer()
            .event_format(LineFormat::with_timer(FixedClock, detail))
            .with_writer(move || sink.clone())
            .with_ansi(ansi);
        let subscriber = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(subscriber, emit);
        capture.text()
    }

    #[test]
    fn a_compact_plain_line_is_time_level_module_message_and_fields() {
        let line = render(false, Detail::Compact, || {
            tracing::info!(
                target: "seleniumbase_rs::browser::session",
                path = "/tmp/chrome",
                "patched chrome binary ready"
            );
        });
        assert_eq!(
            line,
            "12:34:56  INFO browser::session: patched chrome binary ready path=/tmp/chrome\n"
        );
    }

    #[test]
    fn a_coloured_line_dims_the_context_and_highlights_paths_and_steps() {
        let line = render(true, Detail::Compact, || {
            tracing::info!(
                target: "seleniumbase_rs::plugins::screen_shots",
                test = "login",
                path = "/tmp/fail.png",
                "saved a failure screenshot"
            );
        });
        let expected = concat!(
            "\x1b[2m12:34:56\x1b[0m ",
            "\x1b[1;32m INFO\x1b[0m ",
            "\x1b[2mplugins::screen_shots:\x1b[0m",
            " saved a failure screenshot",
            " \x1b[2mtest=\x1b[0m\x1b[1;35mlogin\x1b[0m",
            " \x1b[2mpath=\x1b[0m\x1b[36m/tmp/fail.png\x1b[0m",
            "\n"
        );
        assert_eq!(line, expected);
    }

    #[test]
    fn every_level_has_its_own_colour() {
        let line = render(true, Detail::Compact, || {
            tracing::error!("e");
            tracing::warn!("w");
            tracing::info!("i");
            tracing::debug!("d");
            tracing::trace!("t");
        });
        for (level, code) in [
            ("ERROR", "\x1b[1;31m"),
            (" WARN", "\x1b[1;33m"),
            (" INFO", "\x1b[1;32m"),
            ("DEBUG", "\x1b[1;34m"),
            ("TRACE", "\x1b[35m"),
        ] {
            assert!(
                line.contains(&format!("{code}{level}\x1b[0m")),
                "{level}: {line:?}"
            );
        }
    }

    #[test]
    fn the_compact_line_has_no_span_noise_and_the_full_one_has_it() {
        let emit = || {
            let _connect = tracing::info_span!("connect", url = "ws://x").entered();
            tracing::info!(target: "seleniumbase_rs::browser::session", "connected");
        };
        let compact = render(false, Detail::Compact, emit);
        assert_eq!(compact, "12:34:56  INFO browser::session: connected\n");
        assert!(!compact.contains("connect{"), "{compact}");

        let full = render(false, Detail::Full, emit);
        assert_eq!(
            full,
            "12:34:56  INFO connect{url=\"ws://x\"}: seleniumbase_rs::browser::session: connected\n"
        );
    }

    #[test]
    fn nested_spans_are_listed_outermost_first_in_the_full_line() {
        let line = render(false, Detail::Full, || {
            let _new = tracing::info_span!("new").entered();
            let _connect = tracing::info_span!("connect").entered();
            tracing::info!(target: "t", "go");
        });
        assert_eq!(line, "12:34:56  INFO new:connect: t: go\n");
    }

    #[test]
    fn a_line_without_colour_carries_no_escape_sequence() {
        // What a pipe, a file, NO_COLOR or TERM=dumb get: the layer is built
        // with `ansi = false`, whatever the events hold.
        let line = render(false, Detail::Full, || {
            let _span = tracing::info_span!("work", id = 7).entered();
            tracing::error!(
                path = "/tmp/x",
                step = "click",
                "failed: \x1b[31mred\x1b[0m"
            );
            tracing::warn!(note = "has \x1b[2J in it", "careful");
        });
        assert!(!line.contains('\x1b'), "{line:?}");
        assert!(line.contains("ERROR") && line.contains("WARN"), "{line}");
    }

    #[test]
    fn text_from_outside_cannot_send_an_escape_sequence_to_a_terminal() {
        let line = render(true, Detail::Compact, || {
            tracing::warn!(title = "a\x1b]0;owned\x07b", "page said \x1b[2Jhello");
        });
        // The only escape sequences are ours: every ESC is followed by `[`
        // and ends in `m`.
        assert!(!line.contains("\x1b[2J"), "{line:?}");
        assert!(!line.contains("\x1b]0;"), "{line:?}");
        assert!(!line.contains('\x07'), "{line:?}");
        assert!(line.contains('\u{fffd}'), "{line:?}");
    }

    #[test]
    fn fields_are_bare_when_one_word_and_quoted_otherwise() {
        let line = render(false, Detail::Compact, || {
            tracing::info!(
                target: "seleniumbase_rs::testing",
                test = "login flow",
                id = 7,
                ok = true,
                empty = "",
                "ran"
            );
        });
        assert_eq!(
            line,
            "12:34:56  INFO testing: ran test=\"login flow\" id=7 ok=true empty=\"\"\n"
        );
    }

    #[test]
    fn a_line_with_no_message_is_the_fields_alone() {
        let line = render(false, Detail::Compact, || {
            tracing::info!(target: "seleniumbase_rs::x", answer = 42);
        });
        assert_eq!(line, "12:34:56  INFO x: answer=42\n");
    }

    #[test]
    fn a_target_outside_this_crate_is_shown_whole() {
        let line = render(false, Detail::Compact, || {
            tracing::info!(target: "hyper::proto", "reading");
        });
        assert_eq!(line, "12:34:56  INFO hyper::proto: reading\n");
    }

    #[test]
    fn a_record_of_the_log_crate_keeps_its_own_target_and_hides_the_log_fields() {
        let line = render(false, Detail::Compact, || {
            let record = tracing_log::log::Record::builder()
                .args(format_args!("reading"))
                .target("hyper::proto")
                .level(tracing_log::log::Level::Warn)
                .build();
            tracing_log::log::Log::log(&tracing_log::LogTracer::new(), &record);
        });
        assert_eq!(line, "12:34:56  WARN hyper::proto: reading\n");
    }

    #[test]

    fn paths_and_steps_are_found_by_the_name_of_the_field() {
        for key in [
            "path",
            "dest",
            "chrome_path",
            "user_data_dir",
            "source",
            "file",
        ] {
            assert_eq!(value_style(key), palette::PATH, "{key}");
        }
        for key in ["step", "test", "scenario", "action"] {
            assert_eq!(value_style(key), palette::STEP, "{key}");
        }
        for key in ["url", "version", "peer", "revision"] {
            assert_eq!(value_style(key), Style::PLAIN, "{key}");
        }
    }

    #[test]
    fn the_clock_shows_milliseconds_only_in_the_full_line() {
        let mut compact = String::new();
        Clock { millis: false }
            .format_time(&mut Writer::new(&mut compact))
            .unwrap();
        let mut full = String::new();
        Clock { millis: true }
            .format_time(&mut Writer::new(&mut full))
            .unwrap();
        assert_eq!(compact.len(), "12:34:56".len(), "{compact}");
        assert_eq!(full.len(), "12:34:56.789".len(), "{full}");
        assert_eq!(&full[8..9], ".", "{full}");
    }
}
