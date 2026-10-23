//! Stealth and anti-detection support: CDP client wrappers, undetected-chrome
//! evasions, chromedriver patching, fingerprint profiles, and injected
//! JavaScript helpers.

pub mod behavior;
pub mod capture;
pub mod cdp;
pub mod dprocess;
pub mod evasions;
pub mod fingerprint;
pub mod humanize;
pub mod js;
pub mod options;
pub mod patcher;
pub mod policy;
pub mod providers;
pub mod randomize;
pub mod reactor;
pub mod settings;
pub mod spoof;
pub mod uc;

pub use capture::{CapturedFingerprint, ProbeInfo};
pub use fingerprint::{
    BatteryProfile, BrandVersion, ClientHints, CoherenceReport, ConnectionProfile, Fingerprint,
    FingerprintComparison, FingerprintError, HumanizeConfig, SettingDiff, SpeechVoice,
    StealthPolicy,
};
pub use patcher::{
    engine_spoofing_args, find_system_chrome, ChromeBinaryPatcher, ChromedriverPatcher, EnginePatch,
};
pub use providers::{
    default_registry, session_registry, EvasionConfig, EvasionContext, EvasionProvider,
    EvasionRegistry, SESSION_OMITTED_PROVIDERS,
};
pub use spoof::Spoof;
