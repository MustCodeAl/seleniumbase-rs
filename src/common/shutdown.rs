//! Graceful shutdown helpers (Twelve-Factor IX: Disposability).
//!
//! Long-running binaries should await [`wait_for_shutdown_signal`] so that
//! SIGINT/SIGTERM triggers a clean exit. The returned future completes on the
//! first termination signal.

use std::time::Duration;

use crate::config::RuntimeConfig;
use crate::error::SeleniumBaseError;

/// Waits for the first SIGINT or SIGTERM signal.
#[cfg(unix)]
pub async fn wait_for_shutdown_signal() -> Result<(), SeleniumBaseError> {
    use tokio::signal::unix::{signal, SignalKind};
    let mut sigint = signal(SignalKind::interrupt()).map_err(|e| {
        SeleniumBaseError::Unsupported(format!("failed to install SIGINT handler: {e}"))
    })?;
    let mut sigterm = signal(SignalKind::terminate()).map_err(|e| {
        SeleniumBaseError::Unsupported(format!("failed to install SIGTERM handler: {e}"))
    })?;

    tokio::select! {
        _ = sigint.recv() => {},
        _ = sigterm.recv() => {},
    }
    Ok(())
}

/// Windows fallback: waits for Ctrl-C.
#[cfg(windows)]
pub async fn wait_for_shutdown_signal() -> Result<(), SeleniumBaseError> {
    tokio::signal::ctrl_c().await.map_err(|e| {
        SeleniumBaseError::Unsupported(format!("failed to install ctrl-c handler: {e}"))
    })
}

/// Waits for a shutdown signal or a timeout taken from `RuntimeConfig`.
pub async fn wait_for_shutdown_or_timeout(config: &RuntimeConfig) -> Result<(), SeleniumBaseError> {
    tokio::select! {
        result = wait_for_shutdown_signal() => result,
        _ = tokio::time::sleep(config.shutdown_timeout) => Ok(()),
    }
}

/// Returns the shutdown timeout from runtime config.
pub fn timeout_from_env() -> Duration {
    RuntimeConfig::from_env()
        .map(|c| c.shutdown_timeout)
        .unwrap_or_else(|_| Duration::from_secs(30))
}
