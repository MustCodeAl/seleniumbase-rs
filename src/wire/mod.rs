//! Wire-level fingerprints: TLS and HTTP/2 specs measured from a local browser, and the `BrowserKind` they come from.
//!
//! The main types are re-exported here: the parsers (`ClientHello`, `Ja3`, `Ja4`, `Ja4Raw`,
//! `Peetprint`, `Akamai`), the specs (`TlsSpec`, `H2Spec`, `HeaderSpec`), and the measured table
//! (`Emulation`). The error enums of the TLS and HTTP/2 specs are both called `SpecError` in their
//! modules, so they are re-exported as `TlsSpecError` and `H2SpecError`; `EmulationError` is the
//! third, and `emulation::SpecError` is an alias of it. The lookups of the measured table are the
//! functions of `emulation` (`measured`, `for_kind`, `get`); they are re-exported under the names
//! `measured_emulations`, `emulation_for_kind` and `get_emulation`, because plain `get` says
//! nothing at this level.
//!
//! The wire layer is experimental, like the rest of the stealth layer: its types may change
//! between releases.

#![forbid(unsafe_code)]

pub mod akamai;
pub mod client_hello;
pub mod emulation;
pub mod h2_frames;
pub mod h2_spec;
pub mod header_spec;
pub mod ids;
pub mod ja3;
pub mod ja4;
pub mod tls_spec;

pub use akamai::{Akamai, AkamaiError};
pub use client_hello::{ClientHello, ClientHelloError};
pub use emulation::{
    for_kind as emulation_for_kind, get as get_emulation, measured as measured_emulations,
    Emulation, EmulationError,
};
pub use h2_spec::{H2Spec, SpecError as H2SpecError};
pub use header_spec::{HeaderIdentity, HeaderSpec, HeaderSpecError, RequestShape};
pub use ja3::{Ja3, Ja3Error, Peetprint, PeetprintError};
pub use ja4::{Ja4, Ja4Error, Ja4Raw};
pub use tls_spec::{KnownGap, SpecError as TlsSpecError, TlsSpec};

/// A browser whose wire behaviour the measured tables describe.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserKind {
    /// Google Chrome, the first browser measured (on macOS).
    Chrome,
}
