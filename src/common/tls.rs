//! The TLS crypto provider for the HTTP clients this crate builds.
//!
//! reqwest and thirtyfour are built without a crypto provider of their own, so
//! the `ring` one serves every HTTPS connection and no C library (`aws-lc-sys`)
//! has to be compiled. rustls needs a process-wide provider before the first
//! client is built; this crate calls [`install_tls_provider`] before each of
//! its clients and WebDriver sessions.

/// Installs `ring` as the process-wide rustls crypto provider, unless the
/// program installed one already. Cheap after the first call.
///
/// Call it before building your own `reqwest` client in a program that uses
/// this crate: the crate builds reqwest without a provider, and a client built
/// before any provider is set panics.
///
/// ```
/// seleniumbase_rs::install_tls_provider();
/// assert!(rustls::crypto::CryptoProvider::get_default().is_some());
/// ```
pub fn install_tls_provider() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        if rustls::crypto::CryptoProvider::get_default().is_none() {
            // Losing a race with another installer leaves that provider in place.
            let _ = rustls::crypto::ring::default_provider().install_default();
        }
    });
}
