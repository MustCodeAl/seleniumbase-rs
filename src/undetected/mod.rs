//! Undetected ChromeDriver helpers: binary patching, CDC scrubbing, detached
//! process spawning, and CDP event reactor.

pub mod dprocess;
pub mod patcher;
pub mod reactor;
pub mod uc;

pub use dprocess::start_detached;
pub use patcher::{
    patch_chromedriver, BinaryFormat, BinaryMarkerReport, BinaryPatchRule, ChromedriverPatcher,
    EnginePatch, MarkerCounts, PatchPreset,
};
pub use reactor::{CdpEvent, CdpReactor};
pub use uc::{
    apply_uc_stealth, clear_cdc_properties, override_geolocation, override_timezone,
    override_user_agent, CLEAR_CDC_SCRIPT, UC_STEALTH_SCRIPT,
};
