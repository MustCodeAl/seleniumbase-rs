//! A progress bar for one download, drawn on stdout only when it is a terminal.
//!
//! Driver and browser archives use this while their bytes arrive. A pipe, a
//! redirect, or an explicit quiet flag draws nothing: [`ProgressDrawTarget::hidden`]
//! never writes. indicatif 0.18 (docs.rs, `ProgressBar::with_draw_target`,
//! `ProgressStyle::with_template`, `ProgressBar::enable_steady_tick`).

use std::io::{self, IsTerminal};
use std::time::Duration;

use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};

/// How often a spinner with no known length redraws.
const SPINNER_INTERVAL: Duration = Duration::from_millis(100);

/// Byte bar. The keys are indicatif's built-in placeholders.
const BYTE_TEMPLATE: &str = "{msg} [{bar:40}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})";

/// Spinner for a response that does not say how long it is.
const SPINNER_TEMPLATE: &str = "{spinner} {msg} {bytes}";

/// Reports the bytes of one download.
///
/// Nothing is drawn when `show` is false. [`DownloadProgress::on_stdout`] sets
/// that from whether stdout is a terminal, so a pipe stays quiet.
pub(crate) struct DownloadProgress {
    bar: ProgressBar,
    message: String,
    show: bool,
}

impl DownloadProgress {
    /// A bar on stdout when stdout is a terminal, otherwise nothing.
    #[must_use]
    pub(crate) fn on_stdout(message: impl Into<String>) -> Self {
        Self::new(message, io::stdout().is_terminal(), ProgressDrawTarget::stdout())
    }

    /// A bar that draws nothing. Listing fetches use this.
    #[must_use]
    pub(crate) fn quiet() -> Self {
        Self::new(String::new(), false, ProgressDrawTarget::hidden())
    }

    /// `show` draws on `target`. When it is false the target is not used and
    /// nothing is drawn, which is how a pipe and the tests stay quiet.
    #[must_use]
    pub(crate) fn new(
        message: impl Into<String>,
        show: bool,
        target: ProgressDrawTarget,
    ) -> Self {
        let target = if show {
            target
        } else {
            ProgressDrawTarget::hidden()
        };
        // No length yet: the response may not say how big it is. The message is
        // applied in `start`, after the style, so the first draw is the real one.
        let bar = ProgressBar::with_draw_target(None, target);
        Self {
            bar,
            message: message.into(),
            show,
        }
    }

    /// A byte bar when `content_length` is known, otherwise a spinner.
    pub(crate) fn start(&self, content_length: Option<u64>) {
        if !self.show {
            return;
        }
        match content_length {
            Some(length) => {
                self.bar.set_length(length);
                self.bar.set_style(byte_style());
            }
            None => {
                self.bar.set_style(spinner_style());
                self.bar.enable_steady_tick(SPINNER_INTERVAL);
            }
        }
        self.bar.set_message(self.message.clone());
    }

    /// Counts `bytes` more of the body.
    pub(crate) fn advance(&self, bytes: u64) {
        if self.show && bytes > 0 {
            self.bar.inc(bytes);
        }
    }

    /// Clears the bar. Dropping an unfinished bar clears it too.
    pub(crate) fn finish(&self) {
        self.bar.finish_and_clear();
    }
}

/// The archive's file name, for the progress bar.
///
/// The query string is dropped. A name that still carries user info (`@` or
/// `:`) is not used, so a password in the address is not printed.
#[must_use]
pub(crate) fn archive_name(url: &str) -> &str {
    let path = match url.split_once('?') {
        Some((path, _)) => path,
        None => url,
    };
    path.rsplit('/')
        .find(|part| !part.is_empty() && !part.contains('@') && !part.contains(':'))
        .unwrap_or("download")
}

fn byte_style() -> ProgressStyle {
    style_or_default(BYTE_TEMPLATE, ProgressStyle::default_bar).progress_chars("##-")
}

fn spinner_style() -> ProgressStyle {
    style_or_default(SPINNER_TEMPLATE, ProgressStyle::default_spinner)
}

/// `with_template` rejects a bad template. These are fixed strings; a rejection
/// falls back to indicatif's default rather than panicking.
fn style_or_default(template: &str, fallback: fn() -> ProgressStyle) -> ProgressStyle {
    ProgressStyle::with_template(template).unwrap_or_else(|_| fallback())
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use indicatif::{ProgressDrawTarget, TermLike};

    use super::{archive_name, DownloadProgress};

    /// Records whatever a bar would print. A quiet bar must leave it empty.
    #[derive(Debug)]
    struct RecordingTerm {
        written: Arc<Mutex<Vec<u8>>>,
    }

    impl TermLike for RecordingTerm {
        fn width(&self) -> u16 {
            80
        }

        fn move_cursor_up(&self, _n: usize) -> std::io::Result<()> {
            Ok(())
        }

        fn move_cursor_down(&self, _n: usize) -> std::io::Result<()> {
            Ok(())
        }

        fn move_cursor_right(&self, _n: usize) -> std::io::Result<()> {
            Ok(())
        }

        fn move_cursor_left(&self, _n: usize) -> std::io::Result<()> {
            Ok(())
        }

        fn write_line(&self, line: &str) -> std::io::Result<()> {
            self.write_str(line)?;
            self.write_str("\n")
        }

        fn write_str(&self, text: &str) -> std::io::Result<()> {
            self.written
                .lock()
                .expect("progress log")
                .extend(text.as_bytes());
            Ok(())
        }

        fn clear_line(&self) -> std::io::Result<()> {
            Ok(())
        }

        fn flush(&self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn recording_term(written: &Arc<Mutex<Vec<u8>>>) -> ProgressDrawTarget {
        let term = RecordingTerm {
            written: Arc::clone(written),
        };
        // No refresh limit, so a shown bar draws on the call instead of later.
        ProgressDrawTarget::term_like(Box::new(term))
    }

    #[test]
    fn a_quiet_download_writes_nothing() {
        let written = Arc::new(Mutex::new(Vec::new()));
        let progress = DownloadProgress::new("chromedriver", false, recording_term(&written));
        progress.start(Some(32));
        progress.advance(16);
        progress.finish();
        assert!(written.lock().expect("progress log").is_empty(), "a byte bar was drawn");

        let written = Arc::new(Mutex::new(Vec::new()));
        let progress = DownloadProgress::new("chrome", false, recording_term(&written));
        progress.start(None);
        progress.advance(8);
        progress.finish();
        assert!(written.lock().expect("progress log").is_empty(), "a spinner was drawn");
    }

    #[test]
    fn a_shown_download_names_the_archive() {
        let written = Arc::new(Mutex::new(Vec::new()));
        let progress = DownloadProgress::new("chromedriver", true, recording_term(&written));
        progress.start(Some(32));
        progress.advance(16);
        progress.finish();
        let text = String::from_utf8(written.lock().expect("progress log").clone())
            .expect("bar text is utf-8");
        assert!(text.contains("chromedriver"), "the bar did not name the download: {text:?}");
    }

    #[test]
    fn the_bar_names_the_archive_and_not_a_password() {
        assert_eq!(
            archive_name("https://user:secret@example.test/edgedriver_mac64.zip?x=1"),
            "edgedriver_mac64.zip",
        );
        assert_eq!(
            archive_name("https://example.test/chromedriver-mac-arm64.zip"),
            "chromedriver-mac-arm64.zip",
        );
        assert_eq!(archive_name("https://user:secret@example.test"), "download");
    }
}
