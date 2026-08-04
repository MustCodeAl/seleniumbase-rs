//! Stealth and anti-detection support: CDP client wrappers, undetected-chrome
//! evasions, chromedriver patching, fingerprint profiles, and injected
//! JavaScript helpers.

pub mod cdp;
pub mod dprocess;
pub mod evasions;
pub mod fingerprint;
pub mod humanize;
pub mod js;
pub mod options;
pub mod patcher;
pub mod providers;
pub mod reactor;
pub mod uc;

pub use fingerprint::{
    BatteryProfile, BrandVersion, ClientHints, CoherenceReport, ConnectionProfile, Fingerprint,
    HumanizeConfig, SpeechVoice, StealthFlags,
};
pub use patcher::{
    engine_spoofing_args, find_system_chrome, ChromeBinaryPatcher, ChromedriverPatcher, EnginePatch,
};
pub use providers::{
    default_registry, EvasionConfig, EvasionContext, EvasionProvider, EvasionRegistry,
};
