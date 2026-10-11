//! Tests of the tracing sink: one line per event, the right level, and nothing secret in it.

use std::io;
use std::sync::{Arc, Mutex};

use tracing::Level;
use tracing_subscriber::fmt::MakeWriter;

use super::*;
use crate::scrape::ledger::events::fixtures::{every_event, SECRETS};
use crate::scrape::passport::identity::fixtures::egress;

/// A writer that keeps what the subscriber prints.
#[derive(Clone, Default)]
struct Buffer(Arc<Mutex<Vec<u8>>>);

impl io::Write for Buffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .expect("the buffer lock")
            .extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for Buffer {
    type Writer = Self;

    fn make_writer(&'a self) -> Self {
        self.clone()
    }
}

/// What a subscriber that lets events up to `level` through prints for `event`.
fn log_at(level: Level, event: &ScraperEvent) -> String {
    let buffer = Buffer::default();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(buffer.clone())
        .with_max_level(level)
        .with_ansi(false)
        .without_time()
        .finish();
    tracing::subscriber::with_default(subscriber, || TracingEventSink.emit(event));
    let bytes = buffer.0.lock().expect("the buffer lock").clone();
    String::from_utf8(bytes).expect("the output is text")
}

fn log(event: &ScraperEvent) -> String {
    log_at(Level::TRACE, event)
}

#[test]
fn each_event_becomes_one_line_on_the_target_that_names_it() {
    for event in every_event() {
        let line = log(&event);
        assert_eq!(line.lines().count(), 1, "{line}");
        assert!(line.contains(TARGET), "{line}");
        assert!(line.contains(event.name()), "{line}");
    }
}

#[test]
fn urls_are_logged_redacted() {
    for event in every_event().iter().take(3) {
        let line = log(event);
        assert!(line.contains("example.com/path"), "{line}");
        assert!(line.contains("token=***"), "{line}");
        for secret in SECRETS {
            assert!(!line.contains(secret), "{secret} leaked into {line}");
        }
    }
}

#[test]
fn no_event_leaks_a_secret_at_any_level() {
    for event in every_event() {
        for level in [
            Level::ERROR,
            Level::WARN,
            Level::INFO,
            Level::DEBUG,
            Level::TRACE,
        ] {
            let line = log_at(level, &event);
            for secret in SECRETS {
                assert!(!line.contains(secret), "{secret} leaked into {line}");
            }
        }
    }
}

#[test]
fn levels_follow_how_much_an_event_matters() {
    let events = every_event();
    let expected = [
        (0, "DEBUG"),
        (1, "INFO"),
        (2, "INFO"),
        (3, "DEBUG"),
        (4, "INFO"),
        (5, "INFO"),
        (6, "INFO"),
        (7, "INFO"),
        (8, "WARN"),
        (9, "INFO"),
        (10, "INFO"),
    ];
    for (index, level) in expected {
        let line = log(&events[index]);
        assert!(
            line.contains(level),
            "{} should be {level}: {line}",
            events[index].name()
        );
    }
}

#[test]
fn a_filtered_level_prints_nothing() {
    let events = every_event();
    assert_eq!(log_at(Level::INFO, &events[0]), "", "a request is debug");
    assert_eq!(log_at(Level::WARN, &events[1]), "", "a challenge is info");
    assert_eq!(
        log_at(Level::ERROR, &events[8]),
        "",
        "a cooldown is a warning"
    );
    assert_ne!(log_at(Level::INFO, &events[1]), "");
}

#[test]
fn an_egress_change_names_the_proxies_and_the_direct_connection() {
    let rotated = ScraperEvent::EgressRotated {
        from: Some(egress("p1.example")),
        to: None,
    };
    let line = log(&rotated);
    assert!(line.contains("http://p1.example:8080"), "{line}");
    assert!(line.contains("direct"), "{line}");
}

#[test]
fn the_values_of_an_event_are_fields() {
    let line = log(&every_event()[0]);
    for field in ["status=200", "elapsed_ms=31", "mode=Http"] {
        assert!(line.contains(field), "{field} missing from {line}");
    }
    let changed = log(&every_event()[4]);
    assert!(changed.contains("Browser"), "{changed}");
    assert!(changed.contains("Challenge"), "{changed}");
    let cooldown = log(&every_event()[8]);
    assert!(
        cooldown.contains("site=example.com") && cooldown.contains("until=1800000900"),
        "{cooldown}"
    );
}

#[test]
fn without_a_subscriber_the_sink_is_silent() {
    for event in every_event() {
        TracingEventSink.emit(&event);
    }
}
