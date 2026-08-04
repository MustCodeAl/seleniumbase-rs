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

use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

use crate::config::{LogFormat, RuntimeConfig};

fn env_filter() -> EnvFilter {
    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))
}

/// Initialize tracing from the current [`RuntimeConfig`].
///
/// Honors `SB_LOG_LEVEL` and `SB_LOG_FORMAT` so logs are treated as an
/// environment-driven event stream (Twelve-Factor XI).
pub fn init_tracing_from_runtime(config: &RuntimeConfig) {
    let filter = EnvFilter::new(&config.log_level);
    let _ = tracing_log::LogTracer::init();

    #[cfg(feature = "full-tracing")]
    let timing = timing_layer();

    match config.log_format {
        LogFormat::Json => {
            let fmt = json_subscriber::fmt::layer()
                .with_current_span(true)
                .with_span_list(false)
                .with_filter(filter);
            #[cfg(feature = "full-tracing")]
            {
                let _ = tracing_subscriber::registry()
                    .with(fmt)
                    .with(timing)
                    .try_init();
            }
            #[cfg(not(feature = "full-tracing"))]
            {
                let _ = tracing_subscriber::registry().with(fmt).try_init();
            }
        }
        LogFormat::Pretty => {
            let fmt = tracing_subscriber::fmt::layer()
                .with_target(true)
                .with_thread_ids(false)
                .with_filter(filter);
            #[cfg(feature = "full-tracing")]
            {
                let _ = tracing_subscriber::registry()
                    .with(fmt)
                    .with(timing)
                    .try_init();
            }
            #[cfg(not(feature = "full-tracing"))]
            {
                let _ = tracing_subscriber::registry().with(fmt).try_init();
            }
        }
    }
}

/// Install a plain text tracing subscriber and bridge `log` records.
///
/// Reads the `RUST_LOG` environment variable and defaults to `info`.
/// Calling this more than once in the same process is ignored.
pub fn init_tracing() {
    let fmt = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_thread_ids(false)
        .with_filter(env_filter());

    let _ = tracing_log::LogTracer::init();

    #[cfg(feature = "full-tracing")]
    {
        let timing = timing_layer();
        let _ = tracing_subscriber::registry()
            .with(fmt)
            .with(timing)
            .try_init();
    }

    #[cfg(not(feature = "full-tracing"))]
    {
        let _ = tracing_subscriber::registry().with(fmt).try_init();
    }
}

/// Install a JSON tracing subscriber and bridge `log` records.
///
/// Reads the `RUST_LOG` environment variable and defaults to `info`.
/// Calling this more than once in the same process is ignored.
pub fn init_tracing_json() {
    let fmt = json_subscriber::fmt::layer()
        .with_current_span(true)
        .with_span_list(false)
        .with_filter(env_filter());

    let _ = tracing_log::LogTracer::init();

    #[cfg(feature = "full-tracing")]
    {
        let timing = timing_layer();
        let _ = tracing_subscriber::registry()
            .with(fmt)
            .with(timing)
            .try_init();
    }

    #[cfg(not(feature = "full-tracing"))]
    {
        let _ = tracing_subscriber::registry().with(fmt).try_init();
    }
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
/// still inherit the rest of the default subscriber configuration.
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
    std::env::set_var("RUST_LOG", filter);
    init_tracing();
}

/// Initialize JSON tracing with a custom [`EnvFilter`] string.
pub fn init_tracing_json_with_filter(filter: &str) {
    std::env::set_var("RUST_LOG", filter);
    init_tracing_json();
}
