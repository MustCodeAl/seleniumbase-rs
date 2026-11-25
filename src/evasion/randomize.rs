//! Generates a random but coherent browser identity from a seed.
//!
//! A detector does not only look at each signal; it checks that the signals
//! agree with each other. A macOS user agent over a Direct3D renderer, a Tokyo
//! time zone under an `en-US` locale, or a phone-sized screen on a machine with
//! sixteen cores each give a fake identity away. Randomising one field at a time
//! produces exactly those mismatches, so `generate` does not. It draws a whole
//! machine (a graphics adapter or device model from a table of real ones, with
//! the core count, memory and screens such a machine ships with) and a whole
//! place (a locale, its time zone and a spot in that time zone), then derives
//! every field of the [`Fingerprint`](crate::Fingerprint) from them.
//!
//! The public entry point is [`Fingerprint::randomized`](crate::Fingerprint::randomized); this module holds the
//! tables and the assembly. Everything here is pure and uses the crate's
//! seedable [`Rng`], so a seed always gives the same identity.

use super::fingerprint::{
    BatteryProfile, BrandVersion, BrowserType, ClientHints, ConnectionProfile, OsType, SpeechVoice,
};
use super::humanize::Rng;
use super::settings::{
    ColorGamut, ColorScheme, MediaDevices, MediaFeatures, WebGl, WebRtcAddresses,
};

/// The Chromium major versions a generated identity may claim: the recent
/// stable releases. Refresh with each Chromium release, together with
/// [`KNOWN_FULL_VERSIONS`], which must hold a row for every major listed here.
///
/// A repeated major weights the draw: on 2026-11-08 Chrome for Testing's Stable was 157.0.8103.38 (Beta 158 is never drawn), so 157 gets six draws in twelve, 156 three, 155 two, and 154 one, as real visitors spread over recent releases.
/// Earlier (2026-10-15) Google's Chrome version history
/// (`versionhistory.googleapis.com`, stable channel, `mac` and `win64`) served
/// 155 to everyone and 156.0.8078.35 to an early 0.5 %; 154 is the release
/// before them.
const CHROMIUM_MAJORS: [u32; 12] = [154, 155, 155, 156, 156, 156, 157, 157, 157, 157, 157, 157];

/// The current stable Chromium major (Chrome for Testing last-known-good,
/// 2026-11-08). The next bump is this constant, the private `CHROMIUM_MAJORS` table and a new
/// row in [`KNOWN_FULL_VERSIONS`]; a test fails while the table is older.
pub const CURRENT_STABLE_MAJOR: u32 = 157;

/// The Chromium major the fixed presets ([`Fingerprint::windows_desktop`](crate::Fingerprint::windows_desktop) and
/// the others) claim: the newest of the majors a generated identity may claim.
pub const PRESET_CHROMIUM_MAJOR: u32 = CHROMIUM_MAJORS[CHROMIUM_MAJORS.len() - 1];

/// Where a row of [`KNOWN_FULL_VERSIONS`] comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FullVersionSource {
    /// Read from `navigator.userAgentData.getHighEntropyValues` of the branded
    /// Google Chrome installed on the Mac the table was written on, by the
    /// ignored test `tests/client_hints_chrome.rs`.
    MeasuredOnMac,
    /// A build Google's Chrome version history lists on the stable channel for
    /// `mac` and `win64` (`versionhistory.googleapis.com`); not run here.
    GooglePublished,
}

/// A Chromium release whose full version is known, so the high-entropy Client
/// Hints (`uaFullVersion`, `fullVersionList`) can name a real build instead of
/// the `{major}.0.0.0` of the frozen user agent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KnownFullVersion {
    /// The major version, such as `155`.
    pub major: u32,
    /// The full version Chrome reports, such as `155.0.8059.40`.
    pub full_version: &'static str,
    /// Whether the version was measured here or taken from Google's releases.
    pub source: FullVersionSource,
}

/// One known full version per Chromium major, newest last. Chrome builds
/// differ a little between platforms and over a release's life (Google served
/// 155.0.8059.85 from 2026-10-15); one real build per major is far closer to
/// what a real browser reports than `{major}.0.0.0`.
pub const KNOWN_FULL_VERSIONS: [KnownFullVersion; 4] = [
    // Google's version history, stable channel, read 2026-10-15.
    KnownFullVersion {
        major: 154,
        full_version: "154.0.8037.100",
        source: FullVersionSource::GooglePublished,
    },
    // Chrome 155.0.8059.40 on macOS 27.0.1 (build 26A434), measured on
    // 2026-10-15 with `cargo test --test client_hints_chrome -- --ignored`.
    KnownFullVersion {
        major: 155,
        full_version: "155.0.8059.40",
        source: FullVersionSource::MeasuredOnMac,
    },
    // Google's version history, stable channel, read 2026-10-15: the 156 build
    // served to the first 0.5 % on `mac` and `win64`. Not measured here.
    KnownFullVersion {
        major: 156,
        full_version: "156.0.8078.35",
        source: FullVersionSource::GooglePublished,
    },
    // Chrome 157.0.8103.39 on macOS, measured on 2026-11-07.
    KnownFullVersion {
        major: 157,
        full_version: "157.0.8103.39",
        source: FullVersionSource::MeasuredOnMac,
    },
];

/// The known full version of Chromium `major`, if [`KNOWN_FULL_VERSIONS`] has one.
#[must_use]
pub fn known_full_version(major: u32) -> Option<&'static KnownFullVersion> {
    KNOWN_FULL_VERSIONS.iter().find(|row| row.major == major)
}

/// Mobile Safari releases a generated iPhone identity may claim, each with the
/// iOS version its user agent names (`CPU iPhone OS 18_5`).
///
/// Up to Safari 18 the two agree. Safari 26 freezes the OS token: WebKit bug
/// 298717 (<https://bugs.webkit.org/show_bug.cgi?id=298717>, 2025-09-11) shows
/// Safari on iOS 26.0 sending `CPU iPhone OS 18_6 like Mac OS X` with
/// `Version/26.0`, and a WebKit engineer answers that the value is
/// "intentionally not 26" and stays frozen. The same reply says it was unclear
/// whether the frozen value is 18.6, 18.6.2 or 18.7, and nothing here could
/// read a later Safari on an iPhone (no Xcode simulator on this Mac), so Safari
/// 26.1 and later, and Safari 27, are not listed.
const MOBILE_SAFARI_VERSIONS: [(&str, &str); 4] = [
    ("18.1", "18_1"),
    ("18.3.1", "18_3_1"),
    ("18.5", "18_5"),
    ("26.0", "18_6"),
];

/// `navigator.vendor` of every Chromium browser.
const CHROMIUM_VENDOR: &str = "Google Inc.";

/// `navigator.vendor` of Safari.
const SAFARI_VENDOR: &str = "Apple Computer, Inc.";

/// A screen as the page sees it: size in CSS pixels and the device pixel ratio.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Screen {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) pixel_ratio: f64,
}

impl Screen {
    const fn new(width: u32, height: u32, pixel_ratio: f64) -> Self {
        Self {
            width,
            height,
            pixel_ratio,
        }
    }
}

/// How capable a PC is, which decides the cores, memory and screens it has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Tier {
    /// Laptops and office desktops with integrated or entry graphics.
    Entry,
    /// Mainstream gaming and workstation desktops.
    Mid,
    /// Enthusiast desktops.
    High,
}

/// Picks an item using its positive weight in one RNG draw (`next_u64`).
fn pick_weighted<'a, T>(rng: &mut Rng, items: &'a [T], weight: impl Fn(&T) -> u32) -> &'a T {
    let total: u64 = items
        .iter()
        .map(|item| u64::from(weight(item).max(1)))
        .sum();
    let target = rng.next_u64() % total;
    let mut acc = 0_u64;
    for item in items {
        acc += u64::from(weight(item).max(1));
        if target < acc {
            return item;
        }
    }
    &items[items.len() - 1]
}

/// What a [`Tier`] of PC reports. Repeating a value weights it.
#[derive(Debug)]
struct TierSpec {
    /// `navigator.hardwareConcurrency`.
    cores: [u32; 6],
    /// `navigator.deviceMemory` (Chromium caps it at 8).
    memory_gb: [f64; 4],
    /// Screens on Windows, where fractional display scaling is common.
    windows_screens: [Screen; 8],
    /// Screens on Linux.
    linux_screens: [Screen; 8],
}

static ENTRY: TierSpec = TierSpec {
    cores: [4, 4, 4, 6, 8, 8],
    memory_gb: [4.0, 4.0, 8.0, 8.0],
    windows_screens: [
        Screen::new(1366, 768, 1.0),
        Screen::new(1920, 1080, 1.0),
        Screen::new(1920, 1080, 1.0),
        Screen::new(1536, 864, 1.25),
        Screen::new(1536, 864, 1.25),
        Screen::new(1920, 1080, 1.25),
        Screen::new(1280, 720, 1.5),
        Screen::new(1600, 900, 1.0),
    ],
    linux_screens: [
        Screen::new(1366, 768, 1.0),
        Screen::new(1920, 1080, 1.0),
        Screen::new(1920, 1080, 1.0),
        Screen::new(1920, 1080, 1.0),
        Screen::new(1600, 900, 1.0),
        Screen::new(1920, 1200, 1.0),
        Screen::new(1280, 800, 1.0),
        Screen::new(1920, 1080, 1.25),
    ],
};

static MID: TierSpec = TierSpec {
    cores: [6, 8, 8, 8, 12, 12],
    memory_gb: [8.0, 8.0, 8.0, 8.0],
    windows_screens: [
        Screen::new(1920, 1080, 1.0),
        Screen::new(1920, 1080, 1.0),
        Screen::new(1920, 1080, 1.0),
        Screen::new(1536, 864, 1.25),
        Screen::new(1536, 864, 1.25),
        Screen::new(2560, 1440, 1.0),
        Screen::new(1600, 900, 1.0),
        Screen::new(2048, 1152, 1.25),
    ],
    linux_screens: [
        Screen::new(1920, 1080, 1.0),
        Screen::new(1920, 1080, 1.0),
        Screen::new(1920, 1080, 1.0),
        Screen::new(1920, 1200, 1.0),
        Screen::new(2560, 1440, 1.0),
        Screen::new(2560, 1440, 1.0),
        Screen::new(1920, 1080, 1.25),
        Screen::new(1680, 1050, 1.0),
    ],
};

static HIGH: TierSpec = TierSpec {
    cores: [12, 12, 16, 16, 16, 20],
    memory_gb: [8.0, 8.0, 8.0, 8.0],
    windows_screens: [
        Screen::new(2560, 1440, 1.0),
        Screen::new(2560, 1440, 1.0),
        Screen::new(1920, 1080, 1.0),
        Screen::new(1920, 1080, 2.0),
        Screen::new(3840, 2160, 1.0),
        Screen::new(3840, 2160, 1.5),
        Screen::new(2560, 1440, 1.25),
        Screen::new(3440, 1440, 1.0),
    ],
    linux_screens: [
        Screen::new(2560, 1440, 1.0),
        Screen::new(2560, 1440, 1.0),
        Screen::new(3840, 2160, 1.0),
        Screen::new(3840, 2160, 2.0),
        Screen::new(1920, 1080, 2.0),
        Screen::new(1920, 1080, 1.0),
        Screen::new(2560, 1600, 1.0),
        Screen::new(3440, 1440, 1.0),
    ],
};

impl Tier {
    fn spec(self) -> &'static TierSpec {
        match self {
            Self::Entry => &ENTRY,
            Self::Mid => &MID,
            Self::High => &HIGH,
        }
    }
}

/// The makers of discrete and integrated PC graphics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum GpuVendor {
    Nvidia,
    Intel,
    Amd,
}

impl GpuVendor {
    /// The PCI vendor id.
    const fn pci_id(self) -> u16 {
        match self {
            Self::Nvidia => 0x10de,
            Self::Intel => 0x8086,
            Self::Amd => 0x1002,
        }
    }

    /// How ANGLE names the maker when it runs over Direct3D 11.
    const fn direct3d_name(self) -> &'static str {
        match self {
            Self::Nvidia => "NVIDIA",
            Self::Intel => "Intel",
            Self::Amd => "AMD",
        }
    }

    /// How ANGLE names the maker when it runs over desktop OpenGL.
    const fn opengl_name(self) -> &'static str {
        match self {
            Self::Nvidia => "NVIDIA Corporation",
            Self::Intel => "Intel",
            Self::Amd => "AMD",
        }
    }
}

/// A PC graphics adapter and the class of machine it sits in.
#[derive(Clone, Copy, Debug)]
pub(super) struct Adapter {
    pub(super) vendor: GpuVendor,
    /// On Windows the adapter name Direct3D reports; on Linux the part of the
    /// renderer string after the maker, as Mesa or the NVIDIA driver words it.
    pub(super) name: &'static str,
    /// The PCI device id.
    pub(super) device_id: u16,
    pub(super) tier: Tier,
    pub(super) slug: &'static str,
    pub(super) weight: u32,
}

impl Adapter {
    const fn new(
        vendor: GpuVendor,
        name: &'static str,
        device_id: u16,
        tier: Tier,
        slug: &'static str,
        weight: u32,
    ) -> Self {
        Self {
            vendor,
            name,
            device_id,
            tier,
            slug,
            weight,
        }
    }
}

/// Windows adapters, named the way Chrome's ANGLE layer reports them.
#[rustfmt::skip]
pub(super) static WINDOWS_ADAPTERS: [Adapter; 16] = [
    Adapter::new(GpuVendor::Nvidia, "NVIDIA GeForce GTX 1650", 0x1f82, Tier::Entry, "gtx-1650", 3),
    Adapter::new(GpuVendor::Nvidia, "NVIDIA GeForce GTX 1660 SUPER", 0x21c4, Tier::Mid, "gtx-1660-super", 2),
    Adapter::new(GpuVendor::Nvidia, "NVIDIA GeForce RTX 2060", 0x1f08, Tier::Mid, "rtx-2060", 2),
    Adapter::new(GpuVendor::Nvidia, "NVIDIA GeForce RTX 3050", 0x2507, Tier::Mid, "rtx-3050", 3),
    Adapter::new(GpuVendor::Nvidia, "NVIDIA GeForce RTX 3060", 0x2503, Tier::Mid, "rtx-3060", 5),
    Adapter::new(GpuVendor::Nvidia, "NVIDIA GeForce RTX 3070", 0x2484, Tier::High, "rtx-3070", 2),
    Adapter::new(GpuVendor::Nvidia, "NVIDIA GeForce RTX 4060", 0x28a0, Tier::Mid, "rtx-4060", 5),
    Adapter::new(GpuVendor::Nvidia, "NVIDIA GeForce RTX 4070", 0x2786, Tier::High, "rtx-4070", 2),
    Adapter::new(GpuVendor::Intel, "Intel(R) UHD Graphics 620", 0x5917, Tier::Entry, "uhd-620", 4),
    Adapter::new(GpuVendor::Intel, "Intel(R) UHD Graphics 630", 0x3e9b, Tier::Entry, "uhd-630", 4),
    Adapter::new(GpuVendor::Intel, "Intel(R) Iris(R) Xe Graphics", 0x46a6, Tier::Entry, "iris-xe", 6),
    Adapter::new(GpuVendor::Intel, "Intel(R) UHD Graphics 770", 0x4680, Tier::Mid, "uhd-770", 3),
    Adapter::new(GpuVendor::Amd, "AMD Radeon(TM) Graphics", 0x1638, Tier::Entry, "radeon-graphics", 4),
    Adapter::new(GpuVendor::Amd, "AMD Radeon RX 580 Series", 0x67df, Tier::Mid, "rx-580", 2),
    Adapter::new(GpuVendor::Amd, "AMD Radeon RX 6600", 0x73ff, Tier::Mid, "rx-6600", 3),
    Adapter::new(GpuVendor::Amd, "AMD Radeon RX 6700 XT", 0x73df, Tier::High, "rx-6700-xt", 2),
];

/// Linux adapters, worded the way Mesa and the NVIDIA driver word them.
pub(super) static LINUX_ADAPTERS: [Adapter; 10] = [
    Adapter::new(
        GpuVendor::Intel,
        "Mesa Intel(R) UHD Graphics 630 (CFL GT2), OpenGL 4.6",
        0x3e9b,
        Tier::Entry,
        "uhd-630",
        4,
    ),
    Adapter::new(
        GpuVendor::Intel,
        "Mesa Intel(R) UHD Graphics 620 (KBL GT2), OpenGL 4.6",
        0x5917,
        Tier::Entry,
        "uhd-620",
        3,
    ),
    Adapter::new(
        GpuVendor::Intel,
        "Mesa Intel(R) Iris(R) Xe Graphics (TGL GT2), OpenGL 4.6",
        0x9a49,
        Tier::Entry,
        "iris-xe",
        6,
    ),
    Adapter::new(
        GpuVendor::Intel,
        "Mesa Intel(R) UHD Graphics 770 (ADL-S GT1), OpenGL 4.6",
        0x4680,
        Tier::Mid,
        "uhd-770",
        3,
    ),
    Adapter::new(
        GpuVendor::Amd,
        "AMD Radeon RX 6600 (radeonsi, navi23, LLVM 17.0.6, DRM 3.57, 6.8.0-45-generic), OpenGL 4.6",
        0x73ff,
        Tier::Mid,
        "rx-6600",
        3,
    ),
    Adapter::new(
        GpuVendor::Amd,
        "AMD Radeon RX 580 Series (radeonsi, polaris10, LLVM 15.0.7, DRM 3.49, 6.1.0-18-amd64), OpenGL 4.6",
        0x67df,
        Tier::Mid,
        "rx-580",
        2,
    ),
    Adapter::new(
        GpuVendor::Amd,
        "AMD Radeon RX 7800 XT (radeonsi, navi32, LLVM 17.0.6, DRM 3.57, 6.8.0-45-generic), OpenGL 4.6",
        0x747e,
        Tier::High,
        "rx-7800-xt",
        2,
    ),
    Adapter::new(
        GpuVendor::Nvidia,
        "NVIDIA GeForce GTX 1660 SUPER/PCIe/SSE2, OpenGL 4.5.0 NVIDIA 535.183.01",
        0x21c4,
        Tier::Mid,
        "gtx-1660-super",
        2,
    ),
    Adapter::new(
        GpuVendor::Nvidia,
        "NVIDIA GeForce RTX 3060/PCIe/SSE2, OpenGL 4.5.0 NVIDIA 550.107.02",
        0x2503,
        Tier::Mid,
        "rtx-3060",
        4,
    ),
    Adapter::new(
        GpuVendor::Nvidia,
        "NVIDIA GeForce RTX 4070/PCIe/SSE2, OpenGL 4.5.0 NVIDIA 550.107.02",
        0x2786,
        Tier::High,
        "rtx-4070",
        2,
    ),
];

/// An Apple Silicon Mac: the chip, its logical core count, and the screen that
/// ships with the model that carries it.
#[derive(Clone, Copy, Debug)]
pub(super) struct MacModel {
    pub(super) name: &'static str,
    pub(super) chip: &'static str,
    pub(super) cores: u32,
    pub(super) screen: Screen,
    pub(super) weight: u32,
}

impl MacModel {
    const fn new(
        name: &'static str,
        chip: &'static str,
        cores: u32,
        screen: Screen,
        weight: u32,
    ) -> Self {
        Self {
            name,
            chip,
            cores,
            screen,
            weight,
        }
    }
}

pub(super) static MAC_MODELS: [MacModel; 17] = [
    MacModel::new(
        "macbook-13-m1",
        "Apple M1",
        8,
        Screen::new(1440, 900, 2.0),
        4,
    ),
    MacModel::new(
        "macbook-pro-14-m1-pro",
        "Apple M1 Pro",
        10,
        Screen::new(1512, 982, 2.0),
        2,
    ),
    MacModel::new(
        "macbook-pro-16-m1-max",
        "Apple M1 Max",
        10,
        Screen::new(1728, 1117, 2.0),
        1,
    ),
    MacModel::new(
        "macbook-air-13-m2",
        "Apple M2",
        8,
        Screen::new(1470, 956, 2.0),
        5,
    ),
    MacModel::new(
        "mac-mini-m2-1080p",
        "Apple M2",
        8,
        Screen::new(1920, 1080, 1.0),
        2,
    ),
    MacModel::new(
        "macbook-pro-14-m2-pro",
        "Apple M2 Pro",
        12,
        Screen::new(1512, 982, 2.0),
        3,
    ),
    MacModel::new(
        "macbook-pro-16-m2-max",
        "Apple M2 Max",
        12,
        Screen::new(1728, 1117, 2.0),
        1,
    ),
    MacModel::new(
        "macbook-air-13-m3",
        "Apple M3",
        8,
        Screen::new(1470, 956, 2.0),
        5,
    ),
    MacModel::new(
        "macbook-air-15-m3",
        "Apple M3",
        8,
        Screen::new(1710, 1107, 2.0),
        3,
    ),
    MacModel::new("imac-24-m3", "Apple M3", 8, Screen::new(2240, 1260, 2.0), 2),
    MacModel::new(
        "macbook-pro-14-m3-pro",
        "Apple M3 Pro",
        12,
        Screen::new(1512, 982, 2.0),
        3,
    ),
    MacModel::new(
        "macbook-pro-16-m3-max",
        "Apple M3 Max",
        16,
        Screen::new(1728, 1117, 2.0),
        1,
    ),
    MacModel::new(
        "macbook-air-13-m4",
        "Apple M4",
        10,
        Screen::new(1470, 956, 2.0),
        5,
    ),
    MacModel::new(
        "mac-mini-m4",
        "Apple M4",
        10,
        Screen::new(2560, 1440, 1.0),
        2,
    ),
    MacModel::new(
        "macbook-pro-14-m4",
        "Apple M4",
        10,
        Screen::new(1512, 982, 2.0),
        4,
    ),
    MacModel::new(
        "macbook-pro-16-m4-pro",
        "Apple M4 Pro",
        14,
        Screen::new(1728, 1117, 2.0),
        3,
    ),
    MacModel::new(
        "macbook-pro-16-m4-max",
        "Apple M4 Max",
        16,
        Screen::new(1728, 1117, 2.0),
        1,
    ),
];

/// macOS releases a generated Mac may run, as Chrome's `platformVersion` names
/// them: major, minor and patch, so macOS 27.0 is `27.0.0`. All are recent
/// enough for every chip in [`MAC_MODELS`].
///
/// The releases are the current ones of each line on Apple's security releases
/// page (<https://support.apple.com/en-us/100100>, read 2026-10-15): macOS
/// Golden Gate 27 and 27.0.1, Tahoe 26.7.1 and Sequoia 15.8.1. The format is the
/// one Chrome 155 reported on this Mac under macOS 27.0.1: `27.0.1`
/// (`tests/client_hints_chrome.rs`).
const MACOS_VERSIONS: [&str; 6] = ["15.8.1", "26.7.1", "26.7.1", "27.0.0", "27.0.1", "27.0.1"];

/// Windows platform versions as Client Hints report them (Windows 10, then
/// successive Windows 11 releases).
///
/// Microsoft documents the value as the major version of
/// `Windows.Foundation.UniversalApiContract` followed by `.0.0`, with `1.0.0`
/// to `10.0.0` for Windows 10 and `13.0.0` and above for Windows 11
/// (<https://learn.microsoft.com/microsoft-edge/web-platform/how-to-detect-win11>).
/// Repeating recent Windows 11 entries weights draws toward current hosts.
const WINDOWS_VERSIONS: [&str; 9] = [
    "10.0.0", "10.0.0", "13.0.0", "14.0.0", "15.0.0", "15.0.0", "19.0.0", "19.0.0", "19.0.0",
];

/// Linux kernel versions as Client Hints report them.
const LINUX_VERSIONS: [&str; 8] = [
    "5.15.0", "6.1.0", "6.5.0", "6.6.0", "6.8.0", "6.8.0", "6.11.0", "6.12.0",
];

/// A phone GPU as WebGL reports it: `UNMASKED_VENDOR_WEBGL` and
/// `UNMASKED_RENDERER_WEBGL`.
type PhoneGpu = (&'static str, &'static str);

const ADRENO_750: PhoneGpu = ("Qualcomm", "Adreno (TM) 750");
const ADRENO_740: PhoneGpu = ("Qualcomm", "Adreno (TM) 740");
const MALI_G715: PhoneGpu = ("ARM", "Mali-G715");
const MALI_G710: PhoneGpu = ("ARM", "Mali-G710");
const MALI_G78: PhoneGpu = ("ARM", "Mali-G78");

/// An Android phone, with the graphics chip it is built around.
#[derive(Clone, Copy, Debug)]
pub(super) struct AndroidDevice {
    pub(super) name: &'static str,
    /// The model name that Client Hints report.
    pub(super) model: &'static str,
    /// The Android release that Client Hints report as the platform version.
    pub(super) android: &'static str,
    pub(super) screen: Screen,
    pub(super) gpu: PhoneGpu,
    pub(super) weight: u32,
}

impl AndroidDevice {
    const fn new(
        name: &'static str,
        model: &'static str,
        android: &'static str,
        screen: Screen,
        gpu: PhoneGpu,
        weight: u32,
    ) -> Self {
        Self {
            name,
            model,
            android,
            screen,
            gpu,
            weight,
        }
    }
}

#[rustfmt::skip]
pub(super) static ANDROID_DEVICES: [AndroidDevice; 10] = [
    AndroidDevice::new("pixel-9", "Pixel 9", "15.0.0", Screen::new(412, 923, 2.625), MALI_G715, 4),
    AndroidDevice::new("pixel-8", "Pixel 8", "14.0.0", Screen::new(412, 915, 2.625), MALI_G715, 4),
    AndroidDevice::new("pixel-7", "Pixel 7", "14.0.0", Screen::new(412, 915, 2.625), MALI_G710, 2),
    AndroidDevice::new("pixel-7-pro", "Pixel 7 Pro", "14.0.0", Screen::new(412, 892, 3.5), MALI_G710, 2),
    AndroidDevice::new("galaxy-s24", "SM-S921B", "15.0.0", Screen::new(360, 780, 3.0), ADRENO_750, 4),
    AndroidDevice::new("galaxy-s24-ultra", "SM-S928B", "15.0.0", Screen::new(412, 892, 3.5), ADRENO_750, 3),
    AndroidDevice::new("galaxy-s23", "SM-S911B", "14.0.0", Screen::new(360, 780, 3.0), ADRENO_740, 3),
    AndroidDevice::new("galaxy-s23-ultra", "SM-S918B", "14.0.0", Screen::new(412, 883, 3.5), ADRENO_740, 2),
    AndroidDevice::new("oneplus-11", "CPH2449", "13.0.0", Screen::new(412, 919, 3.5), ADRENO_740, 2),
    AndroidDevice::new("galaxy-s21", "SM-G991B", "13.0.0", Screen::new(360, 800, 3.0), MALI_G78, 1),
];

/// An iPhone model with its screen.
#[derive(Clone, Copy, Debug)]
pub(super) struct IphoneModel {
    pub(super) name: &'static str,
    pub(super) screen: Screen,
    pub(super) weight: u32,
}

impl IphoneModel {
    const fn new(name: &'static str, screen: Screen, weight: u32) -> Self {
        Self {
            name,
            screen,
            weight,
        }
    }
}

/// iPhone models with their screen in CSS pixels.
pub(super) static IPHONE_SCREENS: [IphoneModel; 7] = [
    IphoneModel::new("iphone-se-3", Screen::new(375, 667, 2.0), 1),
    IphoneModel::new("iphone-13-mini", Screen::new(375, 812, 3.0), 1),
    IphoneModel::new("iphone-14", Screen::new(390, 844, 3.0), 3),
    IphoneModel::new("iphone-15", Screen::new(393, 852, 3.0), 4),
    IphoneModel::new("iphone-16-pro", Screen::new(402, 874, 3.0), 4),
    IphoneModel::new("iphone-15-pro-max", Screen::new(430, 932, 3.0), 3),
    IphoneModel::new("iphone-16-pro-max", Screen::new(440, 956, 3.0), 3),
];

/// A place a generated identity may be in: a locale, its country, the time zone it is spoken
/// in, and a city in that zone. The table is [`LOCALES`]; the geo code of `seleniumbase-rs-scrape`
/// reads it too.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Locale {
    /// The BCP 47 tag that `navigator.language` reports, such as `de-DE`.
    pub tag: &'static str,
    /// The ISO 3166-1 alpha-2 country code, upper case.
    pub country: &'static str,
    /// `navigator.languages`, most preferred first. The first entry is the tag.
    pub languages: &'static [&'static str],
    /// The IANA time zone.
    pub timezone: &'static str,
    /// Latitude of the city centre, in degrees.
    pub latitude: f64,
    /// Longitude of the city centre, in degrees.
    pub longitude: f64,
}

/// How far a reported position may sit from the city centre, in degrees
/// (about four kilometres).
const POSITION_JITTER_DEGREES: f64 = 0.04;

/// The least a reported position may claim to be off by, in metres.
const MIN_ACCURACY_METRES: f64 = 20.0;

/// The most a reported position may claim to be off by, in metres.
const MAX_ACCURACY_METRES: f64 = 150.0;

/// The `q` value, in tenths, of the second language in an `Accept-Language` header. Each later
/// language drops by one tenth, down to a floor of one.
const SECOND_LANGUAGE_Q_TENTHS: usize = 9;

/// How many languages of a raw list [`Locale::accept_language_list`] keeps. The tenth carries
/// the lowest `q` Chrome writes, one tenth.
const MAX_ACCEPT_LANGUAGES: usize = 10;

/// The `q` of the language at `index` (counting from zero) of an `Accept-Language` list, in
/// tenths: [`SECOND_LANGUAGE_Q_TENTHS`] for the second, one less for each later one, never
/// below one.
fn quality_tenths(index: usize) -> usize {
    (SECOND_LANGUAGE_Q_TENTHS + 1).saturating_sub(index).max(1)
}

/// A spot near a city, as a browser would report it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Position {
    pub(crate) latitude: f64,
    pub(crate) longitude: f64,
    pub(crate) accuracy: f64,
}

impl Locale {
    const fn new(
        tag: &'static str,
        country: &'static str,
        languages: &'static [&'static str],
        timezone: &'static str,
        latitude: f64,
        longitude: f64,
    ) -> Self {
        Self {
            tag,
            country,
            languages,
            timezone,
            latitude,
            longitude,
        }
    }

    /// `Accept-Language`, most preferred first, in the shape Chrome sends it: the first language
    /// bare, then each later one with a `q` that falls by a tenth from `0.9`.
    #[must_use]
    pub fn accept_language(&self) -> String {
        if self.languages.is_empty() {
            return self.tag.to_owned();
        }
        let mut header = String::new();
        for (index, language) in self.languages.iter().enumerate() {
            if index > 0 {
                let tenths = quality_tenths(index);
                header.push_str(&format!(",{language};q=0.{tenths}"));
            } else {
                header.push_str(language);
            }
        }
        header
    }

    /// `Accept-Language` for a raw list of languages, such as the list a page reports in
    /// `navigator.languages`, in the shape Chrome sends it: the first language bare, then each
    /// later one with a `q` that falls by a tenth from `0.9` down to `0.1`.
    ///
    /// This is the list form of [`accept_language`](Self::accept_language), which writes the
    /// curated list of a locale. A raw list is cleaned first: each entry is trimmed, an empty
    /// entry and a repeat are dropped, and only the first ten are kept (the tenth carries
    /// `q=0.1`, the lowest value Chrome writes). Returns `None` when no language is left.
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::evasion::randomize::Locale;
    ///
    /// assert_eq!(
    ///     Locale::accept_language_list(&["de-DE", " de ", "de", "en-US"]).as_deref(),
    ///     Some("de-DE,de;q=0.9,en-US;q=0.8")
    /// );
    /// assert_eq!(Locale::accept_language_list(&[" ", ""]), None);
    /// ```
    #[must_use]
    pub fn accept_language_list<S: AsRef<str>>(languages: &[S]) -> Option<String> {
        let mut kept: Vec<&str> = Vec::new();
        for language in languages {
            let language = language.as_ref().trim();
            if !language.is_empty() && !kept.contains(&language) {
                kept.push(language);
            }
        }
        let header = kept
            .iter()
            .take(MAX_ACCEPT_LANGUAGES)
            .enumerate()
            .map(|(index, language)| {
                if index == 0 {
                    (*language).to_owned()
                } else {
                    format!("{language};q=0.{}", quality_tenths(index))
                }
            })
            .collect::<Vec<_>>()
            .join(",");
        (!header.is_empty()).then_some(header)
    }

    /// A spot within a few kilometres of the city, with the accuracy a browser would claim. It
    /// makes exactly three draws from `rng`, in this order: latitude, longitude, accuracy.
    pub(crate) fn position(&self, rng: &mut Rng) -> Position {
        let latitude = round_degrees(
            self.latitude + rng.range(-POSITION_JITTER_DEGREES, POSITION_JITTER_DEGREES),
        );
        let longitude = round_degrees(
            self.longitude + rng.range(-POSITION_JITTER_DEGREES, POSITION_JITTER_DEGREES),
        );
        let accuracy = rng.range(MIN_ACCURACY_METRES, MAX_ACCURACY_METRES).round();
        Position {
            latitude,
            longitude,
            accuracy,
        }
    }
}

/// How many leading entries of [`LOCALES`] a generated identity may be placed in. Changing it
/// changes every seeded identity, so it only grows by editing the table on purpose.
const RANDOM_REGION_COUNT: usize = 17;

/// The places an identity may be in, and the countries a proxy may exit from. The first
/// [`RANDOM_REGION_COUNT`] entries are the places [`generate`] draws from; the US appears once
/// per time zone there, so that it is the most common, as it is on the web. The entries after
/// them only serve a lookup by country or tag. A country with several entries (the US) is
/// found at its first.
const LOCALE_TABLE: &[Locale] = &[
    Locale::new(
        "en-US",
        "US",
        &["en-US", "en"],
        "America/New_York",
        40.7128,
        -74.0060,
    ),
    Locale::new(
        "en-US",
        "US",
        &["en-US", "en"],
        "America/Chicago",
        41.8781,
        -87.6298,
    ),
    Locale::new(
        "en-US",
        "US",
        &["en-US", "en"],
        "America/Denver",
        39.7392,
        -104.9903,
    ),
    Locale::new(
        "en-US",
        "US",
        &["en-US", "en"],
        "America/Los_Angeles",
        34.0522,
        -118.2437,
    ),
    Locale::new(
        "en-GB",
        "GB",
        &["en-GB", "en"],
        "Europe/London",
        51.5074,
        -0.1278,
    ),
    Locale::new(
        "en-CA",
        "CA",
        &["en-CA", "en"],
        "America/Toronto",
        43.6532,
        -79.3832,
    ),
    Locale::new(
        "en-AU",
        "AU",
        &["en-AU", "en"],
        "Australia/Sydney",
        -33.8688,
        151.2093,
    ),
    Locale::new(
        "en-IN",
        "IN",
        &["en-IN", "en"],
        "Asia/Kolkata",
        19.0760,
        72.8777,
    ),
    Locale::new(
        "de-DE",
        "DE",
        &["de-DE", "de", "en-US", "en"],
        "Europe/Berlin",
        52.5200,
        13.4050,
    ),
    Locale::new(
        "fr-FR",
        "FR",
        &["fr-FR", "fr", "en-US", "en"],
        "Europe/Paris",
        48.8566,
        2.3522,
    ),
    Locale::new(
        "es-ES",
        "ES",
        &["es-ES", "es", "en-US", "en"],
        "Europe/Madrid",
        40.4168,
        -3.7038,
    ),
    Locale::new(
        "it-IT",
        "IT",
        &["it-IT", "it", "en-US", "en"],
        "Europe/Rome",
        41.9028,
        12.4964,
    ),
    Locale::new(
        "nl-NL",
        "NL",
        &["nl-NL", "nl", "en-US", "en"],
        "Europe/Amsterdam",
        52.3676,
        4.9041,
    ),
    Locale::new(
        "pt-BR",
        "BR",
        &["pt-BR", "pt", "en-US", "en"],
        "America/Sao_Paulo",
        -23.5505,
        -46.6333,
    ),
    Locale::new(
        "ja-JP",
        "JP",
        &["ja-JP", "ja", "en-US", "en"],
        "Asia/Tokyo",
        35.6762,
        139.6503,
    ),
    Locale::new(
        "pl-PL",
        "PL",
        &["pl-PL", "pl", "en-US", "en"],
        "Europe/Warsaw",
        52.2297,
        21.0122,
    ),
    Locale::new(
        "sv-SE",
        "SE",
        &["sv-SE", "sv", "en-US", "en"],
        "Europe/Stockholm",
        59.3293,
        18.0686,
    ),
    // From here on the entries are not drawn for a generated identity.
    Locale::new(
        "es-MX",
        "MX",
        &["es-MX", "es", "en-US", "en"],
        "America/Mexico_City",
        19.4326,
        -99.1332,
    ),
    Locale::new(
        "es-AR",
        "AR",
        &["es-AR", "es", "en-US", "en"],
        "America/Argentina/Buenos_Aires",
        -34.6037,
        -58.3816,
    ),
    Locale::new(
        "es-CO",
        "CO",
        &["es-CO", "es", "en-US", "en"],
        "America/Bogota",
        4.7110,
        -74.0721,
    ),
    Locale::new(
        "es-CL",
        "CL",
        &["es-CL", "es", "en-US", "en"],
        "America/Santiago",
        -33.4489,
        -70.6693,
    ),
    Locale::new(
        "pt-PT",
        "PT",
        &["pt-PT", "pt", "en-US", "en"],
        "Europe/Lisbon",
        38.7223,
        -9.1393,
    ),
    Locale::new(
        "ko-KR",
        "KR",
        &["ko-KR", "ko", "en-US", "en"],
        "Asia/Seoul",
        37.5665,
        126.9780,
    ),
    Locale::new(
        "zh-TW",
        "TW",
        &["zh-TW", "zh", "en-US", "en"],
        "Asia/Taipei",
        25.0330,
        121.5654,
    ),
    Locale::new(
        "tr-TR",
        "TR",
        &["tr-TR", "tr", "en-US", "en"],
        "Europe/Istanbul",
        41.0082,
        28.9784,
    ),
    Locale::new(
        "ru-RU",
        "RU",
        &["ru-RU", "ru", "en-US", "en"],
        "Europe/Moscow",
        55.7558,
        37.6173,
    ),
    Locale::new(
        "uk-UA",
        "UA",
        &["uk-UA", "uk", "en-US", "en"],
        "Europe/Kyiv",
        50.4501,
        30.5234,
    ),
    Locale::new(
        "cs-CZ",
        "CZ",
        &["cs-CZ", "cs", "en-US", "en"],
        "Europe/Prague",
        50.0755,
        14.4378,
    ),
    Locale::new(
        "da-DK",
        "DK",
        &["da-DK", "da", "en-US", "en"],
        "Europe/Copenhagen",
        55.6761,
        12.5683,
    ),
    Locale::new(
        "fi-FI",
        "FI",
        &["fi-FI", "fi", "en-US", "en"],
        "Europe/Helsinki",
        60.1699,
        24.9384,
    ),
    Locale::new(
        "nb-NO",
        "NO",
        &["nb-NO", "nb", "en-US", "en"],
        "Europe/Oslo",
        59.9139,
        10.7522,
    ),
    Locale::new(
        "de-AT",
        "AT",
        &["de-AT", "de", "en-US", "en"],
        "Europe/Vienna",
        48.2082,
        16.3738,
    ),
    Locale::new(
        "de-CH",
        "CH",
        &["de-CH", "de", "en-US", "en"],
        "Europe/Zurich",
        47.3769,
        8.5417,
    ),
    Locale::new(
        "nl-BE",
        "BE",
        &["nl-BE", "nl", "en-US", "en"],
        "Europe/Brussels",
        50.8503,
        4.3517,
    ),
    Locale::new(
        "en-IE",
        "IE",
        &["en-IE", "en"],
        "Europe/Dublin",
        53.3498,
        -6.2603,
    ),
    Locale::new(
        "en-NZ",
        "NZ",
        &["en-NZ", "en"],
        "Pacific/Auckland",
        -36.8485,
        174.7633,
    ),
    Locale::new(
        "en-SG",
        "SG",
        &["en-SG", "en"],
        "Asia/Singapore",
        1.3521,
        103.8198,
    ),
    Locale::new(
        "en-ZA",
        "ZA",
        &["en-ZA", "en"],
        "Africa/Johannesburg",
        -26.2041,
        28.0473,
    ),
    Locale::new(
        "en-PH",
        "PH",
        &["en-PH", "en"],
        "Asia/Manila",
        14.5995,
        120.9842,
    ),
    Locale::new(
        "id-ID",
        "ID",
        &["id-ID", "id", "en-US", "en"],
        "Asia/Jakarta",
        -6.2088,
        106.8456,
    ),
    Locale::new(
        "th-TH",
        "TH",
        &["th-TH", "th", "en-US", "en"],
        "Asia/Bangkok",
        13.7563,
        100.5018,
    ),
    Locale::new(
        "vi-VN",
        "VN",
        &["vi-VN", "vi", "en-US", "en"],
        "Asia/Ho_Chi_Minh",
        10.8231,
        106.6297,
    ),
    Locale::new(
        "ar-AE",
        "AE",
        &["ar-AE", "ar", "en-US", "en"],
        "Asia/Dubai",
        25.2048,
        55.2708,
    ),
    Locale::new(
        "he-IL",
        "IL",
        &["he-IL", "he", "en-US", "en"],
        "Asia/Jerusalem",
        31.7683,
        35.2137,
    ),
    Locale::new(
        "el-GR",
        "GR",
        &["el-GR", "el", "en-US", "en"],
        "Europe/Athens",
        37.9838,
        23.7275,
    ),
    Locale::new(
        "hu-HU",
        "HU",
        &["hu-HU", "hu", "en-US", "en"],
        "Europe/Budapest",
        47.4979,
        19.0402,
    ),
    Locale::new(
        "ro-RO",
        "RO",
        &["ro-RO", "ro", "en-US", "en"],
        "Europe/Bucharest",
        44.4268,
        26.1025,
    ),
];

/// The table, for lookups by country and tag and for the geo resolver.
pub static LOCALES: &[Locale] = LOCALE_TABLE;

/// The places [`generate`] draws from: the first [`RANDOM_REGION_COUNT`] entries of [`LOCALES`].
static REGIONS: [Locale; RANDOM_REGION_COUNT] = first_regions();

/// Copies the leading entries of the table, so that the drawn places and the table cannot drift.
const fn first_regions() -> [Locale; RANDOM_REGION_COUNT] {
    let mut regions = [LOCALE_TABLE[0]; RANDOM_REGION_COUNT];
    let mut index = 0;
    while index < RANDOM_REGION_COUNT {
        regions[index] = LOCALE_TABLE[index];
        index += 1;
    }
    regions
}

/// The WebGL extensions of a Windows D3D11 Chrome instance.
pub(crate) fn windows_d3d11_extensions() -> Vec<String> {
    vec![
        "ANGLE_instanced_arrays".to_owned(),
        "EXT_blend_minmax".to_owned(),
        "EXT_clip_control".to_owned(),
        "EXT_color_buffer_half_float".to_owned(),
        "EXT_depth_clamp".to_owned(),
        "EXT_disjoint_timer_query".to_owned(),
        "EXT_float_blend".to_owned(),
        "EXT_frag_depth".to_owned(),
        "EXT_polygon_offset_clamp".to_owned(),
        "EXT_shader_texture_lod".to_owned(),
        "EXT_texture_compression_bptc".to_owned(),
        "EXT_texture_compression_rgtc".to_owned(),
        "EXT_texture_filter_anisotropic".to_owned(),
        "EXT_texture_mirror_clamp_to_edge".to_owned(),
        "EXT_sRGB".to_owned(),
        "KHR_parallel_shader_compile".to_owned(),
        "OES_draw_buffers_indexed".to_owned(),
        "OES_element_index_uint".to_owned(),
        "OES_fbo_render_mipmap".to_owned(),
        "OES_standard_derivatives".to_owned(),
        "OES_texture_float".to_owned(),
        "OES_texture_float_linear".to_owned(),
        "OES_texture_half_float".to_owned(),
        "OES_texture_half_float_linear".to_owned(),
        "OES_vertex_array_object".to_owned(),
        "WEBGL_color_buffer_float".to_owned(),
        "WEBGL_compressed_texture_s3tc".to_owned(),
        "WEBGL_compressed_texture_s3tc_srgb".to_owned(),
        "WEBGL_debug_renderer_info".to_owned(),
        "WEBGL_debug_shaders".to_owned(),
        "WEBGL_depth_texture".to_owned(),
        "WEBGL_draw_buffers".to_owned(),
        "WEBGL_lose_context".to_owned(),
        "WEBGL_multi_draw".to_owned(),
        "WEBKIT_WEBGL_compressed_texture_s3tc".to_owned(),
    ]
}

/// The WebGL extensions of a macOS Metal Chrome instance.
pub(crate) fn macos_metal_extensions() -> Vec<String> {
    vec![
        "ANGLE_instanced_arrays".to_owned(),
        "EXT_blend_minmax".to_owned(),
        "EXT_clip_control".to_owned(),
        "EXT_color_buffer_half_float".to_owned(),
        "EXT_depth_clamp".to_owned(),
        "EXT_float_blend".to_owned(),
        "EXT_frag_depth".to_owned(),
        "EXT_polygon_offset_clamp".to_owned(),
        "EXT_shader_texture_lod".to_owned(),
        "EXT_texture_compression_bptc".to_owned(),
        "EXT_texture_compression_rgtc".to_owned(),
        "EXT_texture_filter_anisotropic".to_owned(),
        "EXT_texture_mirror_clamp_to_edge".to_owned(),
        "EXT_sRGB".to_owned(),
        "KHR_parallel_shader_compile".to_owned(),
        "OES_element_index_uint".to_owned(),
        "OES_fbo_render_mipmap".to_owned(),
        "OES_standard_derivatives".to_owned(),
        "OES_texture_float".to_owned(),
        "OES_texture_float_linear".to_owned(),
        "OES_texture_half_float".to_owned(),
        "OES_texture_half_float_linear".to_owned(),
        "OES_vertex_array_object".to_owned(),
        "WEBGL_color_buffer_float".to_owned(),
        "WEBGL_debug_renderer_info".to_owned(),
        "WEBGL_debug_shaders".to_owned(),
        "WEBGL_depth_texture".to_owned(),
        "WEBGL_draw_buffers".to_owned(),
        "WEBGL_lose_context".to_owned(),
        "WEBGL_multi_draw".to_owned(),
    ]
}

/// The default audio sample rate in Hz for the operating system persona.
#[must_use]
pub(crate) const fn audio_sample_rate_for(os: OsType) -> u32 {
    match os {
        OsType::Macos | OsType::Ios => 48000,
        OsType::Windows => 48000,
        OsType::Linux | OsType::Android => 44100,
    }
}

/// The WebGL strings of a PC adapter behind ANGLE: `maker` is how ANGLE names the maker.
fn pc_graphics(adapter: &Adapter, maker: &str, renderer: String) -> WebGl {
    WebGl::new(format!("Google Inc. ({maker})"), renderer)
        .ids(
            format!("0x{:04x}", adapter.vendor.pci_id()),
            format!("0x{:04x}", adapter.device_id),
        )
        .extensions(windows_d3d11_extensions())
        .max_texture_size(16384)
}

/// The WebGL strings of a phone or tablet, which reports its chip directly.
fn direct_graphics((vendor, renderer): PhoneGpu) -> WebGl {
    WebGl::new(vendor, renderer)
}

/// Everything that is decided by the operating system and the machine.
#[derive(Debug)]
pub(super) struct Profile {
    pub(super) browser_type: BrowserType,
    /// The Chromium major the user agent claims, when it is Chromium.
    pub(super) core_version: Option<u32>,
    pub(super) user_agent: String,
    pub(super) vendor: &'static str,
    pub(super) client_hints: Option<ClientHints>,
    pub(super) cores: u32,
    pub(super) memory_gb: Option<f64>,
    pub(super) touch_points: u32,
    pub(super) screen: Screen,
    pub(super) color_depth: u32,
    pub(super) graphics: WebGl,
}

/// The brand of a named build.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Brand {
    Chrome,
    Edge,
    Safari,
}

/// A named browser build.
#[derive(Debug)]
pub(super) struct NamedBuild {
    pub(super) name: String,
    pub(super) brand: Brand,
    pub(super) major: u32,
    pub(super) os: OsType,
    /// Safari only: (version, the iOS token of its user agent), a row of MOBILE_SAFARI_VERSIONS.
    pub(super) safari: Option<(&'static str, &'static str)>,
}

/// A row of one of the machine tables.
#[derive(Clone, Copy, Debug)]
pub(super) enum MachineRow {
    Pc(OsType, &'static Adapter),
    Mac(&'static MacModel),
    Android(&'static AndroidDevice),
    Iphone(&'static IphoneModel),
}

impl MachineRow {
    pub(super) fn webgl(&self) -> WebGl {
        match *self {
            Self::Pc(OsType::Windows, adapter) => {
                let maker = adapter.vendor.direct3d_name();
                let renderer = format!(
                    "ANGLE ({maker}, {} (0x{:08X}) Direct3D11 vs_5_0 ps_5_0, D3D11)",
                    adapter.name, adapter.device_id
                );
                pc_graphics(adapter, maker, renderer)
            }
            Self::Pc(OsType::Linux, adapter) => linux_graphics(adapter),
            Self::Pc(_, adapter) => {
                let maker = adapter.vendor.opengl_name();
                let renderer = format!("ANGLE ({maker}, {})", adapter.name);
                pc_graphics(adapter, maker, renderer)
            }
            Self::Mac(model) => apple_graphics(model.chip),
            Self::Android(device) => direct_graphics(device.gpu),
            Self::Iphone(_) => direct_graphics(("Apple Inc.", "Apple GPU")),
        }
    }
}

#[derive(Debug)]
pub(super) struct NamedMachine {
    pub(super) name: String,
    pub(super) row: MachineRow,
}

pub(super) static BUILDS: std::sync::LazyLock<Vec<NamedBuild>> = std::sync::LazyLock::new(|| {
    let mut builds = Vec::with_capacity(24);
    let majors: Vec<u32> = KNOWN_FULL_VERSIONS.iter().map(|row| row.major).collect();
    for &major in &majors {
        for &os in &[
            OsType::Windows,
            OsType::Macos,
            OsType::Linux,
            OsType::Android,
        ] {
            let os_str = match os {
                OsType::Windows => "windows",
                OsType::Macos => "macos",
                OsType::Linux => "linux",
                OsType::Android => "android",
                OsType::Ios => "ios",
            };
            builds.push(NamedBuild {
                name: format!("chrome-{major}-{os_str}"),
                brand: Brand::Chrome,
                major,
                os,
                safari: None,
            });
        }
    }
    for &major in &majors {
        builds.push(NamedBuild {
            name: format!("edge-{major}-windows"),
            brand: Brand::Edge,
            major,
            os: OsType::Windows,
            safari: None,
        });
    }
    for &(ver, token) in &MOBILE_SAFARI_VERSIONS {
        let slug = ver.replace('.', "-");
        let major: u32 = ver
            .split('.')
            .next()
            .and_then(|p| p.parse().ok())
            .unwrap_or(18);
        builds.push(NamedBuild {
            name: format!("safari-{slug}-ios"),
            brand: Brand::Safari,
            major,
            os: OsType::Ios,
            safari: Some((ver, token)),
        });
    }
    builds
});

pub(super) static MACHINES: std::sync::LazyLock<Vec<NamedMachine>> =
    std::sync::LazyLock::new(|| {
        let mut machines = Vec::with_capacity(52);
        for adapter in &WINDOWS_ADAPTERS {
            machines.push(NamedMachine {
                name: format!("windows-{}", adapter.slug),
                row: MachineRow::Pc(OsType::Windows, adapter),
            });
        }
        for adapter in &LINUX_ADAPTERS {
            machines.push(NamedMachine {
                name: format!("linux-{}", adapter.slug),
                row: MachineRow::Pc(OsType::Linux, adapter),
            });
        }
        for model in &MAC_MODELS {
            machines.push(NamedMachine {
                name: model.name.to_owned(),
                row: MachineRow::Mac(model),
            });
        }
        for device in &ANDROID_DEVICES {
            machines.push(NamedMachine {
                name: device.name.to_owned(),
                row: MachineRow::Android(device),
            });
        }
        for iphone in &IPHONE_SCREENS {
            machines.push(NamedMachine {
                name: iphone.name.to_owned(),
                row: MachineRow::Iphone(iphone),
            });
        }
        machines
    });

pub(super) static PLACES: std::sync::LazyLock<Vec<(String, &'static Locale)>> =
    std::sync::LazyLock::new(|| {
        LOCALES
            .iter()
            .map(|loc| {
                let tag = loc.tag.to_ascii_lowercase();
                let city = loc
                    .timezone
                    .rsplit('/')
                    .next()
                    .unwrap_or(loc.timezone)
                    .to_ascii_lowercase()
                    .replace('_', "-");
                (format!("{tag}-{city}"), loc)
            })
            .collect()
    });

/// What a draw must take as given instead of picking it. `Default` pins nothing.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Pins<'a> {
    pub(super) major: Option<u32>,
    pub(super) place: Option<&'a Locale>,
    pub(super) build: Option<&'a NamedBuild>,
    pub(super) machine: Option<MachineRow>,
}

/// What one seeded draw decides: the machine, the place and a spot in it.
///
/// A fingerprint builder takes every setting that was asked for at random from
/// one `Draw`, so the settings agree with each other. The extras (fonts, media
/// devices, ...) each have a stream of their own, so asking for one of them
/// never changes another.
#[derive(Debug)]
pub(super) struct Draw {
    pub(super) os: OsType,
    pub(super) profile: Profile,
    pub(super) region: Locale,
    pub(super) position: Position,
    seed: u64,
}

/// Draws a machine for `os` and a place, fully determined by `seed`.
///
/// With `forced_major` the Chromium major is that value instead of a draw; the
/// draw still happens, so the rest of the identity is the same as for the
/// unforced seed. With `place` the region is that locale instead of a draw, in
/// the same way: the draw still happens.
pub(super) fn draw(os: OsType, seed: u64, pins: &Pins<'_>) -> Draw {
    let mut rng = Rng::new(seed);
    let drawn = *rng.pick(&REGIONS);
    let region = pins.place.copied().unwrap_or(drawn);
    let profile = match os {
        OsType::Windows => windows(&mut rng, pins),
        OsType::Macos => macos(&mut rng, pins),
        OsType::Linux => linux(&mut rng, pins),
        OsType::Android => android(&mut rng, pins),
        OsType::Ios => ios(&mut rng, pins),
    };
    let position = region.position(&mut rng);
    Draw {
        os,
        profile,
        region,
        position,
        seed,
    }
}

/// The locale whose tag is `tag`, ignoring case.
#[must_use]
pub(super) fn locale_by_tag(tag: &str) -> Option<&'static Locale> {
    LOCALES.iter().find(|l| l.tag.eq_ignore_ascii_case(tag))
}

/// The fonts a system ships with. The first eight are on every machine of the
/// system; a draw drops some of the rest.
fn system_fonts(os: OsType) -> &'static [&'static str] {
    match os {
        OsType::Windows => &[
            "Arial",
            "Calibri",
            "Consolas",
            "Courier New",
            "Segoe UI",
            "Tahoma",
            "Times New Roman",
            "Verdana",
            "Arial Black",
            "Cambria",
            "Comic Sans MS",
            "Georgia",
            "Impact",
            "Lucida Console",
            "Microsoft Sans Serif",
            "Palatino Linotype",
            "Trebuchet MS",
            "Webdings",
            "Wingdings",
        ],
        OsType::Macos => &[
            "Arial",
            "Courier New",
            "Georgia",
            "Helvetica",
            "Helvetica Neue",
            "Menlo",
            "Times New Roman",
            "Verdana",
            "Avenir",
            "Avenir Next",
            "Gill Sans",
            "Impact",
            "Lucida Grande",
            "Monaco",
            "Optima",
            "Palatino",
            "Trebuchet MS",
            "Zapfino",
        ],
        OsType::Linux => &[
            "DejaVu Sans",
            "DejaVu Sans Mono",
            "DejaVu Serif",
            "Liberation Mono",
            "Liberation Sans",
            "Liberation Serif",
            "Noto Sans",
            "Noto Serif",
            "Cantarell",
            "Noto Color Emoji",
            "Ubuntu",
            "Ubuntu Mono",
        ],
        OsType::Android => &[
            "Roboto",
            "Noto Sans",
            "Noto Serif",
            "Droid Sans Mono",
            "Noto Color Emoji",
            "Cutive Mono",
            "Dancing Script",
            "Coming Soon",
        ],
        OsType::Ios => &[
            "Arial",
            "Courier New",
            "Georgia",
            "Helvetica",
            "Helvetica Neue",
            "Menlo",
            "Times New Roman",
            "Verdana",
            "Avenir",
            "Gill Sans",
            "Optima",
            "Palatino",
            "American Typewriter",
            "Marker Felt",
        ],
    }
}

/// The display and preferences a preset of `os` claims: a P3 display on a
/// Mac and an iPhone, sRGB elsewhere, and the light scheme.
#[must_use]
pub(super) const fn media_features_for(os: OsType) -> MediaFeatures {
    MediaFeatures::new(match os {
        OsType::Macos | OsType::Ios => ColorGamut::P3,
        OsType::Windows | OsType::Linux | OsType::Android => ColorGamut::Srgb,
    })
}

/// The quota Chrome gives one origin on a volume of `gigabytes` (decimal) less
/// the `reserved` bytes the system keeps: 60 % of it.
#[must_use]
pub(super) const fn quota_for_disk(gigabytes: u64, reserved: u64) -> u64 {
    (gigabytes * 1_000_000_000 - reserved) / 5 * 3
}

/// The disks of a desktop, in decimal gigabytes.
const DESKTOP_DISKS: [u64; 4] = [256, 512, 1000, 2000];

/// The disks of a phone, in decimal gigabytes.
const PHONE_DISKS: [u64; 3] = [128, 256, 512];

/// What the system keeps of a disk before the browser's share: 20 GiB in a
/// preset, between 8 and 40 GiB in a draw.
const GIBIBYTE: u64 = 1024 * 1024 * 1024;

/// The quota a preset of `os` claims: a 512 GB desktop disk or a 128 GB phone,
/// each less the 20 GiB the system keeps.
#[must_use]
pub(super) const fn storage_quota_for(os: OsType) -> u64 {
    match os {
        OsType::Android | OsType::Ios => quota_for_disk(128, 20 * GIBIBYTE),
        OsType::Windows | OsType::Macos | OsType::Linux => quota_for_disk(512, 20 * GIBIBYTE),
    }
}

/// The voices `speechSynthesis.getVoices()` lists on `os`.
#[must_use]
pub(super) fn speech_voices_for(os: OsType) -> Vec<SpeechVoice> {
    let voice = |name: &str, lang: &str, default: bool, local: bool| SpeechVoice {
        name: name.to_owned(),
        lang: lang.to_owned(),
        default,
        local_service: local,
        voice_uri: name.to_owned(),
    };
    match os {
        OsType::Windows => SpeechVoice::default_set(),
        OsType::Macos => vec![
            voice("Alex", "en-US", true, true),
            voice("Samantha", "en-US", false, true),
            voice("Victoria", "en-US", false, true),
            voice("Google US English", "en-US", false, false),
        ],
        OsType::Linux => vec![
            voice("Google US English", "en-US", true, false),
            voice("Google UK English Female", "en-GB", false, false),
            voice("Google UK English Male", "en-GB", false, false),
        ],
        OsType::Android => vec![
            voice("Google US English", "en-US", true, true),
            voice("Google UK English Female", "en-GB", false, true),
        ],
        OsType::Ios => vec![
            voice("Samantha", "en-US", true, true),
            voice("Daniel", "en-GB", false, true),
            voice("Karen", "en-AU", false, true),
        ],
    }
}

impl Draw {
    /// A stream of random numbers that belongs to one extra setting.
    fn stream(&self, salt: u64) -> Rng {
        Rng::new(self.seed ^ salt.wrapping_mul(0xD6E8_FEB8_6659_FD93))
    }

    /// The fonts of the system, with some of the less common ones left out.
    pub(super) fn fonts(&self) -> Vec<String> {
        let mut rng = self.stream(1);
        system_fonts(self.os)
            .iter()
            .enumerate()
            .filter(|(index, _)| *index < 8 || rng.chance(0.85))
            .map(|(_, font)| (*font).to_owned())
            .collect()
    }

    /// The microphones, speakers and cameras of a device of this kind.
    pub(super) fn media_devices(&self) -> MediaDevices {
        let mut rng = self.stream(2);
        if self.os.is_mobile() {
            return MediaDevices::counts(1, 1, 2);
        }
        let more = |rng: &mut Rng, odds: f64| u32::from(rng.chance(odds));
        MediaDevices::counts(
            1 + more(&mut rng, 0.3),
            1 + more(&mut rng, 0.4),
            more(&mut rng, 0.7),
        )
    }

    /// A public address of a consumer network and a private local one.
    pub(super) fn webrtc(&self) -> WebRtcAddresses {
        let mut rng = self.stream(3);
        let first = *rng.pick(&[24_u32, 67, 71, 73, 76, 98, 108, 174]);
        // `range` is half-open, and an octet below 256 is a whole number.
        let mut octet = |low: f64, high: f64| rng.range(low, high).floor() as u32;
        let public = format!(
            "{first}.{}.{}.{}",
            octet(1.0, 255.0),
            octet(1.0, 255.0),
            octet(1.0, 254.0)
        );
        let local = format!("192.168.{}.{}", octet(0.0, 2.0), octet(2.0, 250.0));
        WebRtcAddresses::new(public, local)
    }

    /// A battery that is mostly charged, or `None` where the browser has no
    /// Battery Status API.
    pub(super) fn battery(&self) -> Option<BatteryProfile> {
        if self.profile.browser_type == BrowserType::MobileSafari {
            return None;
        }
        let mut rng = self.stream(4);
        let charging = rng.chance(0.6);
        let level = if charging && rng.chance(0.4) {
            1.0
        } else {
            (rng.range(0.25, 1.0) * 100.0).round() / 100.0
        };
        let (charging_time, discharging_time) = if charging {
            (((1.0 - level) * 7200.0).round(), f64::INFINITY)
        } else {
            (f64::INFINITY, (level * 28_800.0).round())
        };
        Some(BatteryProfile {
            charging,
            level,
            charging_time,
            discharging_time,
        })
    }

    /// A 4G connection, or `None` where the browser has no Network Information
    /// API.
    pub(super) fn connection(&self) -> Option<ConnectionProfile> {
        if self.profile.browser_type == BrowserType::MobileSafari {
            return None;
        }
        let mut rng = self.stream(5);
        Some(ConnectionProfile {
            effective_type: "4g".to_owned(),
            rtt: *rng.pick(&[50, 100, 150]),
            downlink: *rng.pick(&[1.5, 5.0, 10.0]),
            save_data: false,
        })
    }

    /// The voices of the system.
    pub(super) fn speech_voices(&self) -> Vec<SpeechVoice> {
        speech_voices_for(self.os)
    }

    /// Whether Web Bluetooth finds an adapter, or `None` where the browser has
    /// no Web Bluetooth.
    pub(super) fn bluetooth(&self) -> Option<bool> {
        if self.profile.browser_type == BrowserType::MobileSafari {
            return None;
        }
        let mut rng = self.stream(6);
        Some(match self.os {
            OsType::Android | OsType::Macos => true,
            OsType::Windows => rng.chance(0.5),
            OsType::Linux => rng.chance(0.3),
            OsType::Ios => false,
        })
    }

    /// A disk of the machine's kind, less between 8 and 40 GiB the system keeps
    /// (in whole MiB), as the quota of one origin. Stream 8.
    pub(super) fn storage_quota(&self) -> u64 {
        let mut rng = self.stream(8);
        let gigabytes = if self.os.is_mobile() {
            *rng.pick(&PHONE_DISKS)
        } else {
            *rng.pick(&DESKTOP_DISKS)
        };

        // `range` is half-open, so this is 8 GiB up to but not including 40 GiB.
        let reserved_mebibytes = rng.range(8.0 * 1024.0, 40.0 * 1024.0).floor() as u64;
        quota_for_disk(gigabytes, reserved_mebibytes * 1024 * 1024)
    }

    /// The display of the machine and its user's preferences: the gamut of
    /// [`media_features_for`], the dark scheme three times in ten, reduced
    /// motion two times in a hundred, nothing else. Stream 7.
    pub(super) fn media_features(&self) -> MediaFeatures {
        let mut rng = self.stream(7);
        MediaFeatures {
            color_scheme: if rng.chance(0.3) {
                ColorScheme::Dark
            } else {
                ColorScheme::Light
            },
            reduced_motion: rng.chance(0.02),
            ..media_features_for(self.os)
        }
    }
}

/// Rounds a coordinate to four decimal places (about eleven metres).
fn round_degrees(degrees: f64) -> f64 {
    (degrees * 10_000.0).round() / 10_000.0
}

/// The Chromium major for a profile: the forced one when given, else a draw. The draw always
/// happens, so that a forced identity keeps every other field of its seed.
fn chromium_major(rng: &mut Rng, forced: Option<u32>) -> u32 {
    let drawn = *rng.pick(&CHROMIUM_MAJORS);
    forced.unwrap_or(drawn)
}

fn windows(rng: &mut Rng, pins: &Pins<'_>) -> Profile {
    let major = chromium_major(rng, pins.major.or_else(|| pins.build.map(|b| b.major)));
    let drawn_adapter = pick_weighted(rng, &WINDOWS_ADAPTERS, |a| a.weight);
    let adapter = match pins.machine {
        Some(MachineRow::Pc(OsType::Windows, a)) => a,
        _ => drawn_adapter,
    };
    let spec = adapter.tier.spec();
    let maker = adapter.vendor.direct3d_name();
    let renderer = format!(
        "ANGLE ({maker}, {} (0x{:08X}) Direct3D11 vs_5_0 ps_5_0, D3D11)",
        adapter.name, adapter.device_id
    );
    let client_hints = ClientHints {
        platform: "Windows".to_owned(),
        platform_version: (*rng.pick(&WINDOWS_VERSIONS)).to_owned(),
        architecture: "x86".to_owned(),
        bitness: "64".to_owned(),
        ..chromium_hints(major)
    };
    let drawn_cores = *rng.pick(&spec.cores);
    let drawn_memory = *rng.pick(&spec.memory_gb);
    let drawn_screen = *rng.pick(&spec.windows_screens);
    let is_pc_pinned = matches!(pins.machine, Some(MachineRow::Pc(OsType::Windows, _)));
    let cores = if is_pc_pinned {
        spec.cores[0]
    } else {
        drawn_cores
    };
    let memory_gb = if is_pc_pinned {
        spec.memory_gb[0]
    } else {
        drawn_memory
    };
    let screen = if is_pc_pinned {
        spec.windows_screens[0]
    } else {
        drawn_screen
    };
    let user_agent = if matches!(pins.build.map(|b| b.brand), Some(Brand::Edge)) {
        edge_user_agent_for_os(OsType::Windows, major)
    } else {
        chromium_user_agent_for_os(OsType::Windows, major)
    };
    Profile {
        browser_type: BrowserType::Chromium,
        core_version: Some(major),
        user_agent,
        vendor: CHROMIUM_VENDOR,
        client_hints: Some(client_hints),
        cores,
        memory_gb: Some(memory_gb),
        touch_points: 0,
        screen,
        color_depth: 24,
        graphics: pc_graphics(adapter, maker, renderer),
    }
}

fn macos(rng: &mut Rng, pins: &Pins<'_>) -> Profile {
    let major = chromium_major(rng, pins.major.or_else(|| pins.build.map(|b| b.major)));
    let drawn_model = pick_weighted(rng, &MAC_MODELS, |m| m.weight);
    let model = match pins.machine {
        Some(MachineRow::Mac(m)) => m,
        _ => drawn_model,
    };
    let client_hints = ClientHints {
        platform: "macOS".to_owned(),
        platform_version: (*rng.pick(&MACOS_VERSIONS)).to_owned(),
        architecture: "arm".to_owned(),
        bitness: "64".to_owned(),
        ..chromium_hints(major)
    };
    Profile {
        browser_type: BrowserType::Chromium,
        core_version: Some(major),
        // Chromium freezes the macOS token at 10_15_7, even on Apple Silicon.
        user_agent: chromium_user_agent_for_os(OsType::Macos, major),
        vendor: CHROMIUM_VENDOR,
        client_hints: Some(client_hints),
        cores: model.cores,
        // Chromium reports at most 8 GB, and every Apple Silicon Mac has that.
        memory_gb: Some(8.0),
        touch_points: 0,
        screen: model.screen,
        // Chromium reports a 30-bit depth on the wide-gamut Retina displays.
        color_depth: if model.screen.pixel_ratio >= 2.0 {
            30
        } else {
            24
        },
        graphics: apple_graphics(model.chip),
    }
}

/// The WebGL strings of an Apple Silicon Mac: ANGLE over Metal.
fn apple_graphics(chip: &str) -> WebGl {
    WebGl::new(
        "Google Inc. (Apple)",
        format!("ANGLE (Apple, ANGLE Metal Renderer: {chip}, Unspecified Version)"),
    )
    .extensions(macos_metal_extensions())
    .max_texture_size(16384)
}

/// The WebGL strings of a Linux machine: ANGLE over desktop OpenGL.
fn linux_graphics(adapter: &Adapter) -> WebGl {
    let maker = adapter.vendor.opengl_name();
    let renderer = format!("ANGLE ({maker}, {})", adapter.name);
    pc_graphics(adapter, maker, renderer)
}

/// The WebGL vendor and renderer of the Apple Silicon `chip`, as Chrome on a Mac
/// reports them. The macOS preset uses it, so it matches what a generated Mac says.
#[must_use]
pub(crate) fn macos_webgl(chip: &str) -> (String, String) {
    let graphics = apple_graphics(chip);
    (
        graphics.vendor.unwrap_or_default(),
        graphics.renderer.unwrap_or_default(),
    )
}

/// The WebGL vendor and renderer of the Linux preset's machine (a desktop with
/// an NVIDIA card), worded as a generated Linux identity words it.
#[must_use]
pub(crate) fn linux_preset_webgl() -> (String, String) {
    let adapter = LINUX_ADAPTERS
        .iter()
        .find(|a| a.name.starts_with("NVIDIA GeForce RTX 4070"))
        .unwrap_or(&LINUX_ADAPTERS[0]);
    let graphics = linux_graphics(adapter);
    (
        graphics.vendor.unwrap_or_default(),
        graphics.renderer.unwrap_or_default(),
    )
}

/// The first place in [`LOCALES`] whose time zone is `timezone`: its locale,
/// country and the coordinates of its city, so a fixed identity can be given a
/// position that agrees with its time zone.
#[must_use]
pub fn locale_of_timezone(timezone: &str) -> Option<&'static Locale> {
    LOCALES.iter().find(|l| l.timezone == timezone)
}

fn linux(rng: &mut Rng, pins: &Pins<'_>) -> Profile {
    let major = chromium_major(rng, pins.major.or_else(|| pins.build.map(|b| b.major)));
    let drawn_adapter = pick_weighted(rng, &LINUX_ADAPTERS, |a| a.weight);
    let adapter = match pins.machine {
        Some(MachineRow::Pc(OsType::Linux, a)) => a,
        _ => drawn_adapter,
    };
    let spec = adapter.tier.spec();
    let client_hints = ClientHints {
        platform: "Linux".to_owned(),
        platform_version: (*rng.pick(&LINUX_VERSIONS)).to_owned(),
        architecture: "x86".to_owned(),
        bitness: "64".to_owned(),
        ..chromium_hints(major)
    };
    let drawn_cores = *rng.pick(&spec.cores);
    let drawn_memory = *rng.pick(&spec.memory_gb);
    let drawn_screen = *rng.pick(&spec.linux_screens);
    let is_pc_pinned = matches!(pins.machine, Some(MachineRow::Pc(OsType::Linux, _)));
    let cores = if is_pc_pinned {
        spec.cores[0]
    } else {
        drawn_cores
    };
    let memory_gb = if is_pc_pinned {
        spec.memory_gb[0]
    } else {
        drawn_memory
    };
    let screen = if is_pc_pinned {
        spec.linux_screens[0]
    } else {
        drawn_screen
    };
    Profile {
        browser_type: BrowserType::Chromium,
        core_version: Some(major),
        user_agent: chromium_user_agent_for_os(OsType::Linux, major),
        vendor: CHROMIUM_VENDOR,
        client_hints: Some(client_hints),
        cores,
        memory_gb: Some(memory_gb),
        touch_points: 0,
        screen,
        color_depth: 24,
        graphics: linux_graphics(adapter),
    }
}

fn android(rng: &mut Rng, pins: &Pins<'_>) -> Profile {
    let major = chromium_major(rng, pins.major.or_else(|| pins.build.map(|b| b.major)));
    let drawn_device = pick_weighted(rng, &ANDROID_DEVICES, |d| d.weight);
    let device = match pins.machine {
        Some(MachineRow::Android(d)) => d,
        _ => drawn_device,
    };
    let client_hints = ClientHints {
        platform: "Android".to_owned(),
        platform_version: device.android.to_owned(),
        // Chrome on Android reports no architecture or bitness.
        architecture: String::new(),
        bitness: String::new(),
        model: device.model.to_owned(),
        mobile: true,
        form_factors: vec!["Mobile".to_owned()],
        ..chromium_hints(major)
    };
    Profile {
        browser_type: BrowserType::Chromium,
        core_version: Some(major),
        // Chromium freezes the Android version and hides the model in the
        // user agent; both are only in the Client Hints.
        user_agent: chromium_user_agent_for_os(OsType::Android, major),
        vendor: CHROMIUM_VENDOR,
        client_hints: Some(client_hints),
        cores: 8,
        memory_gb: Some(8.0),
        touch_points: 5,
        screen: device.screen,
        color_depth: 24,
        graphics: direct_graphics(device.gpu),
    }
}

fn ios(rng: &mut Rng, pins: &Pins<'_>) -> Profile {
    let drawn_version = *rng.pick(&MOBILE_SAFARI_VERSIONS);
    let (version, os_version) = match pins.build.and_then(|b| b.safari) {
        Some(pair) => pair,
        None => drawn_version,
    };
    let drawn_cores = *rng.pick(&[4, 6, 6]);
    let cores = if pins.machine.is_some() {
        6
    } else {
        drawn_cores
    };
    let drawn_screen = pick_weighted(rng, &IPHONE_SCREENS, |m| m.weight).screen;
    let screen = match pins.machine {
        Some(MachineRow::Iphone(m)) => m.screen,
        _ => drawn_screen,
    };
    Profile {
        browser_type: BrowserType::MobileSafari,
        core_version: None,
        user_agent: format!(
            "Mozilla/5.0 (iPhone; CPU iPhone OS {os_version} like Mac OS X) \
             AppleWebKit/605.1.15 (KHTML, like Gecko) Version/{version} \
             Mobile/15E148 Safari/604.1"
        ),
        vendor: SAFARI_VENDOR,
        // Safari has no `navigator.userAgentData`.
        client_hints: None,
        cores,
        // Safari has no `navigator.deviceMemory` either.
        memory_gb: None,
        touch_points: 5,
        screen,
        color_depth: 24,
        graphics: direct_graphics(("Apple Inc.", "Apple GPU")),
    }
}

/// A Chromium user agent. Chromium freezes everything after the major version.
pub(super) fn chromium_user_agent(platform: &str, major: u32, mobile: bool) -> String {
    let mobile = if mobile { " Mobile" } else { "" };
    format!(
        "Mozilla/5.0 ({platform}) AppleWebKit/537.36 (KHTML, like Gecko) \
         Chrome/{major}.0.0.0{mobile} Safari/537.36"
    )
}

/// A realistic Chromium / Google Chrome user agent string for `os` and `major`.
///
/// Desktop targets and Android use Chrome's frozen UA reduction tokens (`10_15_7`
/// on macOS, `Android 10; K` on Android). iOS uses WebKit's `CriOS/` token.
#[must_use]
pub fn chromium_user_agent_for_os(os: OsType, major: u32) -> String {
    match os {
        OsType::Windows => chromium_user_agent("Windows NT 10.0; Win64; x64", major, false),
        OsType::Macos => chromium_user_agent("Macintosh; Intel Mac OS X 10_15_7", major, false),
        OsType::Linux => chromium_user_agent("X11; Linux x86_64", major, false),
        OsType::Android => chromium_user_agent("Linux; Android 10; K", major, true),
        OsType::Ios => format!(
            "Mozilla/5.0 (iPhone; CPU iPhone OS 18_7 like Mac OS X) \
             AppleWebKit/605.1.15 (KHTML, like Gecko) CriOS/{major}.0.0.0 \
             Mobile/15E148 Safari/604.1"
        ),
    }
}

/// A Microsoft Edge user agent: the Chromium one with Edge's own `Edg/` token
/// after Safari's, as Edge sends it.
#[must_use]
pub fn edge_user_agent(platform: &str, major: u32) -> String {
    format!(
        "{} Edg/{major}.0.0.0",
        chromium_user_agent(platform, major, false)
    )
}

/// A realistic Microsoft Edge user agent string for `os` and `major`.
///
/// Uses `Edg/` on desktop platforms, `EdgA/` on Android, and `EdgiOS/` on iOS.
#[must_use]
pub fn edge_user_agent_for_os(os: OsType, major: u32) -> String {
    match os {
        OsType::Windows => edge_user_agent("Windows NT 10.0; Win64; x64", major),
        OsType::Macos => edge_user_agent("Macintosh; Intel Mac OS X 10_15_7", major),
        OsType::Linux => edge_user_agent("X11; Linux x86_64", major),
        OsType::Android => format!(
            "{} EdgA/{major}.0.0.0",
            chromium_user_agent("Linux; Android 10; K", major, true)
        ),
        OsType::Ios => format!(
            "Mozilla/5.0 (iPhone; CPU iPhone OS 18_7 like Mac OS X) \
             AppleWebKit/605.1.15 (KHTML, like Gecko) EdgiOS/{major}.0.0.0 \
             Mobile/15E148 Safari/605.1.15"
        ),
    }
}

/// A realistic Mozilla Firefox user agent string for `os` and `major`.
///
/// Desktop Firefox uses Gecko's frozen `Gecko/20100101` build token; Android
/// Firefox mirrors `{major}.0` in the Gecko token; iOS Firefox renders under
/// WebKit with `FxiOS/{major}.0`.
#[must_use]
pub fn firefox_user_agent(os: OsType, major: u32) -> String {
    match os {
        OsType::Windows => format!(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:{major}.0) Gecko/20100101 Firefox/{major}.0"
        ),
        OsType::Macos => format!(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:{major}.0) Gecko/20100101 Firefox/{major}.0"
        ),
        OsType::Linux => format!(
            "Mozilla/5.0 (X11; Linux x86_64; rv:{major}.0) Gecko/20100101 Firefox/{major}.0"
        ),
        OsType::Android => format!(
            "Mozilla/5.0 (Android 14; Mobile; rv:{major}.0) Gecko/{major}.0 Firefox/{major}.0"
        ),
        OsType::Ios => format!(
            "Mozilla/5.0 (iPhone; CPU iPhone OS 18_7 like Mac OS X) \
             AppleWebKit/605.1.15 (KHTML, like Gecko) FxiOS/{major}.0 \
             Mobile/15E148 Safari/605.1.15"
        ),
    }
}

/// The `navigator.oscpu` string Firefox reports on `os`, or `None` on iOS
/// (where Firefox uses WebKit and `navigator.oscpu` is `undefined`).
#[must_use]
pub const fn gecko_oscpu(os: OsType) -> Option<&'static str> {
    match os {
        OsType::Windows => Some("Windows NT 10.0; Win64; x64"),
        OsType::Macos => Some("Intel Mac OS X 10.15"),
        OsType::Linux => Some("Linux x86_64"),
        OsType::Android => Some("Linux aarch64"),
        OsType::Ios => None,
    }
}

/// The frozen `navigator.buildID` reported by desktop and Android Firefox.
///
/// Firefox froze `navigator.buildID` to `"20181001000000"` in Firefox 64 to
/// prevent fingerprinting of exact build timestamps.
#[must_use]
pub const fn gecko_build_id() -> &'static str {
    "20181001000000"
}

/// The Client Hints of a Microsoft Edge release.
///
/// Edge builds its brand list the way Chromium does, and names itself
/// `Microsoft Edge` where Chrome names itself `Google Chrome`, so this is
/// [`chromium_hints`] with that one brand renamed and in the same position. The
/// full versions are `{major}.0.0.0`: [`KNOWN_FULL_VERSIONS`] holds Chrome
/// builds, and Edge's own build numbers differ from them. The caller fills in
/// the platform fields.
#[must_use]
pub fn edge_hints(major: u32) -> ClientHints {
    let mut hints = chromium_hints(major);
    let full_version = format!("{major}.0.0.0");
    for brand in hints
        .brands
        .iter_mut()
        .chain(hints.full_version_list.iter_mut())
    {
        if brand.brand == "Google Chrome" {
            "Microsoft Edge".clone_into(&mut brand.brand);
        }
    }
    for brand in &mut hints.full_version_list {
        if brand.brand == "Chromium" || brand.brand == "Microsoft Edge" {
            brand.version.clone_from(&full_version);
        }
    }
    hints.ua_full_version = full_version;
    hints
}

/// The characters Chromium picks from to word its decoy ("GREASE") brand.
const GREASE_CHARS: [&str; 11] = [" ", "(", ":", "-", ".", "/", ")", ";", "=", "?", "_"];

/// The versions Chromium gives its decoy brand.
const GREASE_VERSIONS: [&str; 3] = ["8", "99", "24"];

/// The orders Chromium lists its three brands in, as the position of the decoy,
/// `Chromium` and `Google Chrome` in turn.
const BRAND_ORDERS: [[usize; 3]; 6] = [
    [0, 1, 2],
    [0, 2, 1],
    [1, 0, 2],
    [1, 2, 0],
    [2, 0, 1],
    [2, 1, 0],
];

/// The full version Chrome `major` reports in its high-entropy hints: the row
/// of [`KNOWN_FULL_VERSIONS`], else `{major}.0.0.0` for a major the table does
/// not know.
fn full_version_of(major: u32) -> String {
    known_full_version(major).map_or_else(
        || format!("{major}.0.0.0"),
        |row| row.full_version.to_owned(),
    )
}

/// The brand lists and full version of a Chromium release, built the way
/// Chromium builds them: a decoy brand whose wording, version and position are
/// all derived from the major version. The caller fills in the platform fields.
///
/// The full versions (`uaFullVersion` and the `fullVersionList` entries of
/// Chromium and Google Chrome) are the real build in [`KNOWN_FULL_VERSIONS`]
/// when the table has the major, and `{major}.0.0.0` otherwise; the decoy's
/// full version is always `{version}.0.0.0`, as Chrome sends it.
#[must_use]
pub fn chromium_hints(major: u32) -> ClientHints {
    let seed = usize::try_from(major).unwrap_or_default();
    let decoy = format!(
        "Not{}A{}Brand",
        GREASE_CHARS[seed % GREASE_CHARS.len()],
        GREASE_CHARS[(seed + 1) % GREASE_CHARS.len()]
    );
    let decoy_version = GREASE_VERSIONS[seed % GREASE_VERSIONS.len()];
    let order = BRAND_ORDERS[seed % BRAND_ORDERS.len()];

    let list = |decoy_version: String, version: String| {
        let mut slots: [Option<BrandVersion>; 3] = [None, None, None];
        for (brand, slot) in [
            BrandVersion::new(decoy.as_str(), decoy_version),
            BrandVersion::new("Chromium", version.clone()),
            BrandVersion::new("Google Chrome", version),
        ]
        .into_iter()
        .zip(order)
        {
            slots[slot] = Some(brand);
        }
        slots.into_iter().flatten().collect::<Vec<_>>()
    };

    let full_version = full_version_of(major);
    ClientHints {
        brands: list(decoy_version.to_owned(), major.to_string()),
        full_version_list: list(format!("{decoy_version}.0.0.0"), full_version.clone()),
        ua_full_version: full_version,
        form_factors: vec!["Desktop".to_owned()],
        ..ClientHints::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn brands(major: u32) -> Vec<(String, String)> {
        chromium_hints(major)
            .brands
            .into_iter()
            .map(|b| (b.brand, b.version))
            .collect()
    }

    fn pair(brand: &str, version: &str) -> (String, String) {
        (brand.to_owned(), version.to_owned())
    }

    #[test]
    fn a_drawn_display_follows_the_system() {
        use super::super::fingerprint::Fingerprint;

        for os in [
            OsType::Windows,
            OsType::Macos,
            OsType::Linux,
            OsType::Android,
            OsType::Ios,
        ] {
            for seed in 0..100 {
                let fp = Fingerprint::randomized(os, seed);
                let features = fp
                    .media_features()
                    .value()
                    .copied()
                    .unwrap_or_else(|| panic!("{os:?} {seed}: a drawn persona has a display"));
                let gamut = if matches!(os, OsType::Macos | OsType::Ios) {
                    ColorGamut::P3
                } else {
                    ColorGamut::Srgb
                };
                assert_eq!(features.color_gamut, gamut, "{os:?} {seed}");
                assert!(!features.forced_colors, "{os:?} {seed}");
                assert!(!features.reduced_transparency, "{os:?} {seed}");
                assert_eq!(
                    features.contrast,
                    super::super::settings::Contrast::NoPreference
                );
            }
        }
    }

    #[test]
    fn a_drawn_quota_is_sixty_percent_of_a_disk_of_the_machine_s_kind() {
        use super::super::fingerprint::Fingerprint;

        let gib = 1024_u64 * 1024 * 1024;
        for os in [
            OsType::Windows,
            OsType::Macos,
            OsType::Linux,
            OsType::Android,
            OsType::Ios,
        ] {
            // The smallest disk with the most the system keeps, and the largest with the least.
            let (smallest, largest) = if os.is_mobile() {
                (quota_for_disk(128, 40 * gib), quota_for_disk(512, 8 * gib))
            } else {
                (quota_for_disk(256, 40 * gib), quota_for_disk(2000, 8 * gib))
            };
            let mut seen = std::collections::HashSet::new();
            for seed in 0..100 {
                let fp = Fingerprint::randomized(os, seed);
                let quota = *fp
                    .storage_quota()
                    .value()
                    .unwrap_or_else(|| panic!("{os:?} {seed}: a drawn persona has a quota"));
                assert!(
                    (smallest..=largest).contains(&quota),
                    "{os:?} {seed}: {quota} is not 60 % of a disk of its kind"
                );
                assert_ne!(
                    quota % 1_000_000_000,
                    0,
                    "{os:?} {seed}: the system keeps some of the disk, so no round number"
                );
                seen.insert(quota);
            }
            assert!(seen.len() > 50, "{os:?}: the draw varies: {}", seen.len());
        }
    }

    #[test]
    fn a_preset_claims_the_quota_of_a_512_gb_desktop_and_a_128_gb_phone() {
        let gib = 1024_u64 * 1024 * 1024;

        assert_eq!(
            storage_quota_for(OsType::Windows),
            quota_for_disk(512, 20 * gib)
        );
        assert_eq!(
            storage_quota_for(OsType::Android),
            quota_for_disk(128, 20 * gib)
        );
        assert_eq!(
            storage_quota_for(OsType::Macos),
            storage_quota_for(OsType::Linux)
        );
        assert_eq!(
            storage_quota_for(OsType::Ios),
            storage_quota_for(OsType::Android)
        );
        // 60 % of (512 GB less 20 GiB), the order of magnitude Chrome reports.
        assert_eq!(storage_quota_for(OsType::Windows), 294_315_098_112);
    }

    #[test]

    fn brand_lists_match_the_ones_real_chromium_builds() {
        // Brand lists that Chrome 120, 124, 130 and 131 are known to send.
        assert_eq!(
            brands(120),
            [
                pair("Not_A Brand", "8"),
                pair("Chromium", "120"),
                pair("Google Chrome", "120"),
            ]
        );
        assert_eq!(
            brands(124),
            [
                pair("Chromium", "124"),
                pair("Google Chrome", "124"),
                pair("Not-A.Brand", "99"),
            ]
        );
        assert_eq!(
            brands(130),
            [
                pair("Chromium", "130"),
                pair("Google Chrome", "130"),
                pair("Not?A_Brand", "99"),
            ]
        );
        assert_eq!(
            brands(131),
            [
                pair("Google Chrome", "131"),
                pair("Chromium", "131"),
                pair("Not_A Brand", "24"),
            ]
        );
        // Measured: Chrome 155.0.8059.40 on macOS 27.0.1
        // (`tests/client_hints_chrome.rs`).
        assert_eq!(
            brands(155),
            [
                pair("Google Chrome", "155"),
                pair("Chromium", "155"),
                pair("Not(A:Brand", "24"),
            ]
        );
    }

    #[test]
    fn the_full_version_list_of_155_is_the_one_chrome_155_sent_on_this_mac() {
        let hints = chromium_hints(155);
        let full: Vec<(String, String)> = hints
            .full_version_list
            .into_iter()
            .map(|b| (b.brand, b.version))
            .collect();
        assert_eq!(
            full,
            [
                pair("Google Chrome", "155.0.8059.40"),
                pair("Chromium", "155.0.8059.40"),
                pair("Not(A:Brand", "24.0.0.0"),
            ]
        );
        assert_eq!(hints.ua_full_version, "155.0.8059.40");
    }

    #[test]
    fn full_version_list_mirrors_the_brand_list() {
        for major in CHROMIUM_MAJORS {
            let hints = chromium_hints(major);
            let short: Vec<&str> = hints.brands.iter().map(|b| b.brand.as_str()).collect();
            let full: Vec<&str> = hints
                .full_version_list
                .iter()
                .map(|b| b.brand.as_str())
                .collect();
            assert_eq!(short, full, "same brands in the same order for {major}");
            let known = known_full_version(major).expect("every claimable major is in the table");
            assert_eq!(hints.ua_full_version, known.full_version);
            for brand in ["Chromium", "Google Chrome"] {
                assert!(
                    hints
                        .full_version_list
                        .iter()
                        .any(|b| b.brand == brand && b.version == known.full_version),
                    "{brand} carries the full version of {major}"
                );
            }
        }
    }

    #[test]
    fn every_known_full_version_is_a_real_four_part_build_of_its_major() {
        for row in KNOWN_FULL_VERSIONS {
            let parts: Vec<&str> = row.full_version.split('.').collect();
            assert_eq!(parts.len(), 4, "{row:?}");
            assert_eq!(parts[0], row.major.to_string(), "{row:?}");
            assert!(parts.iter().all(|p| p.parse::<u32>().is_ok()), "{row:?}");
            assert_ne!(parts[2], "0", "a real build number, not .0.0: {row:?}");
        }
        let majors: Vec<u32> = KNOWN_FULL_VERSIONS.iter().map(|row| row.major).collect();
        assert!(
            majors.windows(2).all(|pair| pair[0] < pair[1]),
            "one row per major, oldest first: {majors:?}"
        );
        assert_eq!(
            KNOWN_FULL_VERSIONS
                .iter()
                .filter(|row| row.source == FullVersionSource::MeasuredOnMac)
                .map(|row| row.major)
                .collect::<Vec<_>>(),
            [155, 157],
            "Chrome 155 and 157 were measured on this Mac"
        );
    }

    #[test]
    fn the_full_version_of_157_is_the_one_chrome_157_reports_on_this_mac() {
        let v157 = known_full_version(157).expect("157 must be known");
        assert_eq!(v157.full_version, "157.0.8103.39");
        assert_eq!(v157.source, FullVersionSource::MeasuredOnMac);
    }

    #[test]
    fn a_major_the_table_does_not_know_keeps_the_zero_build() {
        assert!(known_full_version(161).is_none());
        let hints = chromium_hints(161);
        assert_eq!(hints.ua_full_version, "161.0.0.0");
        assert!(hints
            .full_version_list
            .iter()
            .any(|b| b.brand == "Google Chrome" && b.version == "161.0.0.0"));
    }

    #[test]
    fn the_table_is_not_older_than_the_current_stable_major() {
        let newest = KNOWN_FULL_VERSIONS.iter().map(|row| row.major).max();
        assert!(
            newest >= Some(CURRENT_STABLE_MAJOR),
            "KNOWN_FULL_VERSIONS stops at {newest:?}; add a row for {CURRENT_STABLE_MAJOR}"
        );
        assert!(
            CHROMIUM_MAJORS.iter().max() >= Some(&CURRENT_STABLE_MAJOR),
            "CHROMIUM_MAJORS must draw {CURRENT_STABLE_MAJOR}"
        );
    }

    #[test]

    fn the_presets_claim_the_newest_major_that_may_be_drawn() {
        assert_eq!(Some(&PRESET_CHROMIUM_MAJOR), CHROMIUM_MAJORS.iter().max());
    }

    #[test]
    fn safari_26_on_an_iphone_names_the_frozen_ios_version() {
        let safari_26 = MOBILE_SAFARI_VERSIONS
            .iter()
            .find(|(version, _)| *version == "26.0")
            .expect("Safari 26.0 is listed");
        assert_eq!(safari_26.1, "18_6");
        for (version, os) in MOBILE_SAFARI_VERSIONS {
            if version.starts_with("18.") {
                assert_eq!(os, version.replace('.', "_"), "Safari 18 names its own iOS");
            }
        }
        // Some seed draws Safari 26, and its user agent is the one WebKit bug
        // 298717 quotes.
        let agent = (0..200)
            .map(|seed| draw(OsType::Ios, seed, &Pins::default()).profile.user_agent)
            .find(|ua| ua.contains("Version/26.0"))
            .expect("a seed draws Safari 26");
        assert_eq!(
            agent,
            "Mozilla/5.0 (iPhone; CPU iPhone OS 18_6 like Mac OS X) AppleWebKit/605.1.15 \
             (KHTML, like Gecko) Version/26.0 Mobile/15E148 Safari/604.1"
        );
    }

    #[test]
    fn user_agents_follow_chromiums_frozen_format() {
        assert_eq!(
            chromium_user_agent("Windows NT 10.0; Win64; x64", 155, false),
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
             (KHTML, like Gecko) Chrome/155.0.0.0 Safari/537.36"
        );
        assert!(chromium_user_agent("Linux; Android 10; K", 155, true)
            .ends_with("Chrome/155.0.0.0 Mobile Safari/537.36"));
    }

    /// Rows of a table must be distinct: a repeated row is a copy-paste slip
    /// that silently doubles a machine's weight.
    fn assert_distinct<T: PartialEq + std::fmt::Debug>(rows: &[T], table: &str) {
        for (i, row) in rows.iter().enumerate() {
            assert!(
                !rows[..i].contains(row),
                "{table} lists {row:?} more than once"
            );
        }
    }

    #[test]
    fn the_tables_have_no_repeated_rows() {
        let windows: Vec<_> = WINDOWS_ADAPTERS
            .iter()
            .map(|a| (a.vendor.pci_id(), a.device_id, a.name))
            .collect();
        assert_distinct(&windows, "WINDOWS_ADAPTERS");
        let win_slugs: Vec<_> = WINDOWS_ADAPTERS.iter().map(|a| a.slug).collect();
        assert_distinct(&win_slugs, "WINDOWS_ADAPTERS slugs");
        let linux: Vec<_> = LINUX_ADAPTERS
            .iter()
            .map(|a| (a.vendor.pci_id(), a.device_id, a.name))
            .collect();
        assert_distinct(&linux, "LINUX_ADAPTERS");
        let lin_slugs: Vec<_> = LINUX_ADAPTERS.iter().map(|a| a.slug).collect();
        assert_distinct(&lin_slugs, "LINUX_ADAPTERS slugs");
        let macs: Vec<_> = MAC_MODELS
            .iter()
            .map(|m| (m.chip, m.cores, m.screen.width))
            .collect();
        assert_distinct(&macs, "MAC_MODELS");
        let mac_names: Vec<_> = MAC_MODELS.iter().map(|m| m.name).collect();
        assert_distinct(&mac_names, "MAC_MODELS names");
        let phones: Vec<_> = ANDROID_DEVICES.iter().map(|d| d.model).collect();
        assert_distinct(&phones, "ANDROID_DEVICES");
        let android_names: Vec<_> = ANDROID_DEVICES.iter().map(|d| d.name).collect();
        assert_distinct(&android_names, "ANDROID_DEVICES names");
        let iphone_names: Vec<_> = IPHONE_SCREENS.iter().map(|i| i.name).collect();
        assert_distinct(&iphone_names, "IPHONE_SCREENS names");
        let places: Vec<_> = LOCALES.iter().map(|r| r.timezone).collect();
        assert_distinct(&places, "LOCALES");
    }

    /// The wording of the header before the table carried a language list per entry.
    fn accept_language_of_the_first_seventeen(tag: &str) -> String {
        let language = tag.split('-').next().unwrap_or(tag);
        if language == "en" {
            format!("{tag},en;q=0.9")
        } else {
            format!("{tag},{language};q=0.9,en-US;q=0.8,en;q=0.7")
        }
    }

    #[test]
    fn languages_take_the_shape_chrome_sends() {
        let region = |languages| Locale::new("xx-XX", "XX", languages, "Zone/Name", 0.0, 0.0);
        assert_eq!(region(&["en-US", "en"]).accept_language(), "en-US,en;q=0.9");
        assert_eq!(
            region(&["de-DE", "de", "en-US", "en"]).accept_language(),
            "de-DE,de;q=0.9,en-US;q=0.8,en;q=0.7"
        );
        assert_eq!(region(&["fr"]).accept_language(), "fr");
        assert_eq!(region(&[]).accept_language(), "xx-XX");
    }

    #[test]
    fn the_q_value_falls_a_tenth_per_language_down_to_a_floor() {
        const MANY: &[&str] = &["a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l"];
        let header = Locale::new("a", "XX", MANY, "Zone/Name", 0.0, 0.0).accept_language();
        assert!(header.starts_with("a,b;q=0.9,c;q=0.8,d;q=0.7"), "{header}");
        assert!(
            header.contains("i;q=0.2") && header.contains("j;q=0.1"),
            "{header}"
        );
        assert!(
            header.ends_with("l;q=0.1"),
            "the floor is one tenth: {header}"
        );
    }

    fn list(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| (*item).to_owned()).collect()
    }

    #[test]
    fn a_raw_list_takes_the_shape_chrome_sends() {
        assert_eq!(
            Locale::accept_language_list(&list(&["en-US", "en"])).as_deref(),
            Some("en-US,en;q=0.9")
        );
        assert_eq!(
            Locale::accept_language_list(&list(&["de-DE", "de", "en-US", "en"])).as_deref(),
            Some("de-DE,de;q=0.9,en-US;q=0.8,en;q=0.7")
        );
        assert_eq!(
            Locale::accept_language_list(&list(&["fr"])).as_deref(),
            Some("fr")
        );
        assert_eq!(
            Locale::accept_language_list(&["de-DE", "de"]).as_deref(),
            Some("de-DE,de;q=0.9"),
            "a list of &str is a list too"
        );
    }

    #[test]
    fn a_raw_list_with_no_language_gives_no_header() {
        assert_eq!(Locale::accept_language_list(&list(&[" ", ""])), None);
        assert_eq!(Locale::accept_language_list(&list(&[])), None);
    }

    #[test]
    fn a_raw_list_is_trimmed_and_its_repeats_collapse() {
        assert_eq!(
            Locale::accept_language_list(&list(&["en", "en", " en "])).as_deref(),
            Some("en")
        );
        assert_eq!(
            Locale::accept_language_list(&list(&[" de-DE ", "", "de", "de-DE", "en"])).as_deref(),
            Some("de-DE,de;q=0.9,en;q=0.8"),
            "an empty entry and a repeat do not take a place in the quality order"
        );
    }

    #[test]
    fn a_raw_list_keeps_ten_languages_and_the_tenth_is_a_tenth() {
        let many: Vec<String> = (0..30).map(|n| format!("l{n}")).collect();
        let header = Locale::accept_language_list(&many).expect("a header");
        assert_eq!(header.matches(',').count(), 9, "{header}");
        assert!(header.starts_with("l0,l1;q=0.9,l2;q=0.8"), "{header}");
        assert!(header.ends_with("l9;q=0.1"), "{header}");
        assert!(!header.contains("l10"), "{header}");
    }

    #[test]
    fn the_two_forms_agree_on_a_curated_list() {
        for locale in LOCALES {
            assert_eq!(
                Locale::accept_language_list(locale.languages),
                Some(locale.accept_language()),
                "{}",
                locale.tag
            );
        }
    }

    #[test]
    fn every_locale_keeps_the_wording_the_first_seventeen_always_had() {
        for locale in LOCALES {
            assert_eq!(
                locale.accept_language(),
                accept_language_of_the_first_seventeen(locale.tag),
                "{}",
                locale.tag
            );
        }
    }

    #[test]
    fn the_drawn_regions_are_the_leading_locales() {
        assert_eq!(REGIONS.as_slice(), &LOCALES[..RANDOM_REGION_COUNT]);
        assert!(
            LOCALES.len() > RANDOM_REGION_COUNT,
            "the table is a superset of the drawn places"
        );
    }

    #[test]
    fn a_position_stays_near_its_city_and_repeats_for_a_seed() {
        let berlin = LOCALES.iter().find(|l| l.tag == "de-DE").expect("present");
        let first = berlin.position(&mut Rng::new(5));
        assert_eq!(first, berlin.position(&mut Rng::new(5)));
        assert!((first.latitude - berlin.latitude).abs() <= POSITION_JITTER_DEGREES + 1e-4);
        assert!((first.longitude - berlin.longitude).abs() <= POSITION_JITTER_DEGREES + 1e-4);
        assert!((MIN_ACCURACY_METRES..=MAX_ACCURACY_METRES).contains(&first.accuracy));
    }

    #[test]
    fn a_forced_major_changes_the_version_and_keeps_the_rest_of_the_seed() {
        let free = draw(OsType::Windows, 9, &Pins::default());
        let pinned = draw(
            OsType::Windows,
            9,
            &Pins {
                major: Some(155),
                ..Pins::default()
            },
        );
        assert_eq!(pinned.profile.core_version, Some(155));
        assert!(pinned.profile.user_agent.contains("Chrome/155."));
        assert_eq!(free.profile.screen, pinned.profile.screen);
        assert_eq!(free.profile.graphics, pinned.profile.graphics);
        assert_eq!(free.position, pinned.position);
    }

    #[test]
    fn a_given_place_replaces_the_region_and_keeps_the_rest_of_the_seed() {
        let berlin = *locale_of_timezone("Europe/Berlin").expect("a known zone");
        for seed in 0..30 {
            let free = draw(OsType::Macos, seed, &Pins::default());
            let placed = draw(
                OsType::Macos,
                seed,
                &Pins {
                    place: Some(&berlin),
                    ..Pins::default()
                },
            );
            assert_eq!(placed.region, berlin, "seed {seed}");
            assert_eq!(placed.profile.user_agent, free.profile.user_agent);
            assert_eq!(placed.profile.graphics, free.profile.graphics);
            assert_eq!(placed.profile.screen, free.profile.screen);
            assert!(
                (placed.position.latitude - berlin.latitude).abs()
                    <= POSITION_JITTER_DEGREES + 1e-4
            );
        }
    }

    #[test]
    fn a_pinned_machine_replaces_the_drawn_one_and_keeps_the_rest_of_the_seed() {
        for seed in [1, 7, 42] {
            // Windows
            let win_adapter = &WINDOWS_ADAPTERS[0];
            let win_row = MachineRow::Pc(OsType::Windows, win_adapter);
            let free = draw(OsType::Windows, seed, &Pins::default());
            let pinned = draw(
                OsType::Windows,
                seed,
                &Pins {
                    machine: Some(win_row),
                    ..Pins::default()
                },
            );
            assert_eq!(pinned.region, free.region);
            assert_eq!(pinned.position, free.position);
            assert_eq!(pinned.profile.core_version, free.profile.core_version);
            assert_eq!(pinned.profile.graphics, win_row.webgl());
            assert_eq!(pinned.profile.cores, win_adapter.tier.spec().cores[0]);

            // Mac
            let mac_model = &MAC_MODELS[0];
            let mac_row = MachineRow::Mac(mac_model);
            let free = draw(OsType::Macos, seed, &Pins::default());
            let pinned = draw(
                OsType::Macos,
                seed,
                &Pins {
                    machine: Some(mac_row),
                    ..Pins::default()
                },
            );
            assert_eq!(pinned.region, free.region);
            assert_eq!(pinned.position, free.position);
            assert_eq!(pinned.profile.core_version, free.profile.core_version);
            assert_eq!(pinned.profile.graphics, mac_row.webgl());
            assert_eq!(pinned.profile.cores, mac_model.cores);
            assert_eq!(pinned.profile.screen, mac_model.screen);

            // Linux
            let lin_adapter = &LINUX_ADAPTERS[0];
            let lin_row = MachineRow::Pc(OsType::Linux, lin_adapter);
            let free = draw(OsType::Linux, seed, &Pins::default());
            let pinned = draw(
                OsType::Linux,
                seed,
                &Pins {
                    machine: Some(lin_row),
                    ..Pins::default()
                },
            );
            assert_eq!(pinned.region, free.region);
            assert_eq!(pinned.position, free.position);
            assert_eq!(pinned.profile.core_version, free.profile.core_version);
            assert_eq!(pinned.profile.graphics, lin_row.webgl());
            assert_eq!(pinned.profile.cores, lin_adapter.tier.spec().cores[0]);

            // Android
            let dev = &ANDROID_DEVICES[0];
            let android_row = MachineRow::Android(dev);
            let free = draw(OsType::Android, seed, &Pins::default());
            let pinned = draw(
                OsType::Android,
                seed,
                &Pins {
                    machine: Some(android_row),
                    ..Pins::default()
                },
            );
            assert_eq!(pinned.region, free.region);
            assert_eq!(pinned.position, free.position);
            assert_eq!(pinned.profile.core_version, free.profile.core_version);
            assert_eq!(pinned.profile.graphics, android_row.webgl());
            assert_eq!(pinned.profile.screen, dev.screen);

            // iOS
            let iphone = &IPHONE_SCREENS[0];
            let iphone_row = MachineRow::Iphone(iphone);
            let free = draw(OsType::Ios, seed, &Pins::default());
            let pinned = draw(
                OsType::Ios,
                seed,
                &Pins {
                    machine: Some(iphone_row),
                    ..Pins::default()
                },
            );
            assert_eq!(pinned.region, free.region);
            assert_eq!(pinned.position, free.position);
            assert_eq!(pinned.profile.graphics, iphone_row.webgl());
            assert_eq!(pinned.profile.cores, 6);
            assert_eq!(pinned.profile.screen, iphone.screen);
        }
    }

    #[test]
    fn a_pinned_pc_takes_the_first_values_of_its_tier() {
        let adapter = &WINDOWS_ADAPTERS[0];
        let spec = adapter.tier.spec();
        let mac = MachineRow::Pc(OsType::Windows, adapter);
        let pinned = draw(
            OsType::Windows,
            42,
            &Pins {
                machine: Some(mac),
                ..Pins::default()
            },
        );
        assert_eq!(pinned.profile.cores, spec.cores[0]);
        assert_eq!(pinned.profile.memory_gb, Some(spec.memory_gb[0]));
        assert_eq!(pinned.profile.screen, spec.windows_screens[0]);
    }

    #[test]
    fn a_pinned_edge_build_draws_an_edge_user_agent() {
        let edge_build = BUILDS
            .iter()
            .find(|b| b.name == "edge-156-windows")
            .expect("edge build");
        let pinned = draw(
            OsType::Windows,
            42,
            &Pins {
                build: Some(edge_build),
                ..Pins::default()
            },
        );
        assert!(pinned.profile.user_agent.contains("Edg/156.0.0.0"));
    }

    #[test]
    fn a_pinned_safari_build_draws_its_version() {
        let safari_build = BUILDS
            .iter()
            .find(|b| b.name == "safari-26-0-ios")
            .expect("safari build");
        let pinned = draw(
            OsType::Ios,
            42,
            &Pins {
                build: Some(safari_build),
                ..Pins::default()
            },
        );
        assert!(pinned.profile.user_agent.contains("Version/26.0"));
        assert!(pinned
            .profile
            .user_agent
            .contains("CPU iPhone OS 18_6 like Mac OS X"));
    }

    #[test]
    fn the_extras_repeat_for_a_seed_and_do_not_depend_on_each_other() {
        let first = draw(OsType::Windows, 12, &Pins::default());
        let second = draw(OsType::Windows, 12, &Pins::default());
        assert_eq!(first.fonts(), second.fonts());
        assert_eq!(first.media_devices(), second.media_devices());
        assert_eq!(first.webrtc(), second.webrtc());
        assert_eq!(first.battery(), second.battery());
        assert_eq!(first.connection(), second.connection());
        assert_eq!(first.bluetooth(), second.bluetooth());
        // Another seed gives another set of addresses.
        assert_ne!(
            first.webrtc(),
            draw(OsType::Windows, 13, &Pins::default()).webrtc()
        );
    }

    #[test]
    fn the_extras_are_what_the_system_and_the_browser_have() {
        for seed in 0..30 {
            let windows = draw(OsType::Windows, seed, &Pins::default());
            assert!(windows.fonts().iter().any(|f| f == "Segoe UI"));
            assert!(windows
                .battery()
                .is_some_and(|b| (0.0..=1.0).contains(&b.level)));
            let webrtc = windows.webrtc();
            assert!(webrtc
                .local_ip
                .as_deref()
                .is_some_and(|ip| ip.starts_with("192.168.")));
            let phone = draw(OsType::Android, seed, &Pins::default());
            assert_eq!(phone.media_devices(), MediaDevices::counts(1, 1, 2));
            let safari = draw(OsType::Ios, seed, &Pins::default());
            assert!(safari.battery().is_none() && safari.connection().is_none());
            assert!(safari.bluetooth().is_none());
            assert!(!safari.speech_voices().is_empty());
        }
    }

    #[test]
    fn a_forced_major_reaches_the_user_agent_and_the_client_hints_alike() {
        for os in [
            OsType::Windows,
            OsType::Macos,
            OsType::Linux,
            OsType::Android,
        ] {
            let profile = draw(
                os,
                4,
                &Pins {
                    major: Some(161),
                    ..Pins::default()
                },
            )
            .profile;
            assert_eq!(profile.core_version, Some(161), "{os:?}");
            let ua = profile.user_agent.as_str();
            assert!(ua.contains("Chrome/161.0.0.0"), "{os:?}: {ua}");
            let hints = profile.client_hints.expect("Chromium has client hints");
            assert_eq!(hints.ua_full_version, "161.0.0.0", "{os:?}");
            for name in ["Chromium", "Google Chrome"] {
                assert!(
                    hints
                        .brands
                        .iter()
                        .any(|b| b.brand == name && b.version == "161"),
                    "{os:?}: {name} must claim 161"
                );
            }
        }
    }

    #[test]
    fn ios_has_no_chromium_major_to_force() {
        let free = draw(OsType::Ios, 4, &Pins::default()).profile;
        let forced = draw(
            OsType::Ios,
            4,
            &Pins {
                major: Some(161),
                ..Pins::default()
            },
        )
        .profile;
        assert_eq!(free.user_agent, forced.user_agent);
        assert_eq!(forced.core_version, None);
    }

    #[test]
    fn every_region_is_internally_consistent() {
        for region in LOCALES {
            assert!(
                region.accept_language().starts_with(region.tag),
                "{} must lead its own language list",
                region.tag
            );
            assert_eq!(region.languages.first(), Some(&region.tag));
            assert_eq!(region.country.len(), 2);
            assert!(region.country.bytes().all(|b| b.is_ascii_uppercase()));
            assert!(
                region.timezone.contains('/'),
                "{} is not IANA",
                region.timezone
            );
            assert!((-90.0..=90.0).contains(&region.latitude));
            assert!((-180.0..=180.0).contains(&region.longitude));
        }
    }

    #[test]
    fn every_screen_is_sane() {
        let tiers = [&ENTRY, &MID, &HIGH];
        let screens = tiers
            .iter()
            .flat_map(|t| t.windows_screens.iter().chain(t.linux_screens.iter()))
            .chain(MAC_MODELS.iter().map(|m| &m.screen))
            .chain(ANDROID_DEVICES.iter().map(|d| &d.screen))
            .chain(IPHONE_SCREENS.iter().map(|m| &m.screen));
        for screen in screens {
            assert!(screen.width >= 360 && screen.height >= 640, "{screen:?}");
            assert!((1.0..=4.0).contains(&screen.pixel_ratio), "{screen:?}");
        }
    }

    #[test]
    fn device_memory_never_exceeds_the_cap_chromium_applies() {
        for tier in [&ENTRY, &MID, &HIGH] {
            assert!(tier.memory_gb.iter().all(|gb| (0.25..=8.0).contains(gb)));
        }
    }

    #[test]
    fn every_os_produces_an_identity() {
        for os in [
            OsType::Windows,
            OsType::Macos,
            OsType::Linux,
            OsType::Android,
            OsType::Ios,
        ] {
            let fp = crate::evasion::fingerprint::Fingerprint::randomized(os, 1);
            assert_eq!(fp.os_type(), os);
            assert!(fp.validate().is_coherent());
        }
    }

    #[test]
    fn the_noise_seed_of_an_identity_is_the_seed_it_was_drawn_from() {
        for os in [OsType::Windows, OsType::Ios] {
            for seed in [0, 3, 42, u64::MAX] {
                let fp = crate::evasion::fingerprint::Fingerprint::randomized(os, seed);
                assert_eq!(fp.seed(), seed, "{os:?}");
            }
        }
    }

    fn brand_names(hints: &ClientHints) -> Vec<&str> {
        hints.brands.iter().map(|b| b.brand.as_str()).collect()
    }

    #[test]
    fn edge_names_itself_where_chrome_would_and_keeps_chromiums_order() {
        for major in CHROMIUM_MAJORS {
            let edge = edge_hints(major);
            let chrome = chromium_hints(major);
            let names = brand_names(&edge);
            assert!(names.contains(&"Microsoft Edge"), "{names:?}");
            assert!(names.contains(&"Chromium"), "{names:?}");
            assert!(!names.contains(&"Google Chrome"), "{names:?}");
            // The same slot holds the renamed brand.
            let renamed: Vec<&str> = brand_names(&chrome)
                .into_iter()
                .map(|b| {
                    if b == "Google Chrome" {
                        "Microsoft Edge"
                    } else {
                        b
                    }
                })
                .collect();
            assert_eq!(names, renamed, "{major}");
        }
    }

    #[test]
    fn edges_full_versions_are_the_frozen_form_not_a_chrome_build() {
        let edge = edge_hints(155);
        assert_eq!(edge.ua_full_version, "155.0.0.0");
        for brand in &edge.full_version_list {
            if brand.brand == "Chromium" || brand.brand == "Microsoft Edge" {
                assert_eq!(brand.version, "155.0.0.0", "{}", brand.brand);
            }
        }
        assert!(!edge.full_version_list.is_empty());
    }

    #[test]
    fn the_edge_user_agent_is_the_chromium_one_with_edges_token() {
        assert_eq!(
            edge_user_agent("Windows NT 10.0; Win64; x64", 155),
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) \
             Chrome/155.0.0.0 Safari/537.36 Edg/155.0.0.0"
        );
    }

    #[test]
    fn a_time_zone_finds_its_city() {
        let berlin = locale_of_timezone("Europe/Berlin").expect("a known zone");
        assert_eq!((berlin.country, berlin.tag), ("DE", "de-DE"));
        assert!(locale_of_timezone("Pacific/Honolulu").is_none());
        let chicago = locale_of_timezone("America/Chicago").expect("a US zone");
        assert!((chicago.latitude - 41.88).abs() < 0.1);
    }

    #[test]
    fn the_linux_and_mac_presets_word_webgl_as_a_generated_identity_does() {
        let (vendor, renderer) = linux_preset_webgl();
        assert_eq!(vendor, "Google Inc. (NVIDIA Corporation)");
        assert!(
            renderer.starts_with("ANGLE (NVIDIA Corporation, NVIDIA GeForce RTX 4070"),
            "{renderer}"
        );
        assert!(!renderer.contains("Direct3D"), "{renderer}");
        // It is one of the strings the Linux generator can produce.
        assert!((0..200).any(|seed| {
            let profile = draw(OsType::Linux, seed, &Pins::default()).profile;
            profile.graphics.renderer.as_deref() == Some(renderer.as_str())
        }));

        let (vendor, renderer) = macos_webgl("Apple M3");
        assert_eq!(vendor, "Google Inc. (Apple)");
        assert_eq!(
            renderer,
            "ANGLE (Apple, ANGLE Metal Renderer: Apple M3, Unspecified Version)"
        );
    }

    #[test]
    fn firefox_user_agent_and_gecko_metadata_match_each_os() {
        assert_eq!(
            firefox_user_agent(OsType::Windows, 134),
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:134.0) Gecko/20100101 Firefox/134.0"
        );
        assert_eq!(
            firefox_user_agent(OsType::Macos, 134),
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:134.0) Gecko/20100101 Firefox/134.0"
        );
        assert_eq!(
            firefox_user_agent(OsType::Linux, 130),
            "Mozilla/5.0 (X11; Linux x86_64; rv:130.0) Gecko/20100101 Firefox/130.0"
        );
        assert_eq!(
            firefox_user_agent(OsType::Android, 134),
            "Mozilla/5.0 (Android 14; Mobile; rv:134.0) Gecko/134.0 Firefox/134.0"
        );
        assert!(firefox_user_agent(OsType::Ios, 134).contains("FxiOS/134.0"));

        assert_eq!(
            gecko_oscpu(OsType::Windows),
            Some("Windows NT 10.0; Win64; x64")
        );
        assert_eq!(gecko_oscpu(OsType::Macos), Some("Intel Mac OS X 10.15"));
        assert_eq!(gecko_oscpu(OsType::Linux), Some("Linux x86_64"));
        assert_eq!(gecko_oscpu(OsType::Android), Some("Linux aarch64"));
        assert_eq!(gecko_oscpu(OsType::Ios), None);
        assert_eq!(gecko_build_id(), "20181001000000");

        let ff = crate::evasion::fingerprint::Fingerprint::builder()
            .browser_type(BrowserType::Firefox)
            .os_type(OsType::Linux)
            .user_agent(firefox_user_agent(OsType::Linux, 134))
            .platform("Linux x86_64")
            .oscpu(gecko_oscpu(OsType::Linux).expect("Linux Firefox has oscpu"))
            .build_id(gecko_build_id())
            .locale("en-US")
            .languages("en-US,en;q=0.9")
            .timezone("America/New_York")
            .build()
            .expect("valid Firefox profile");
        let report = ff.validate();
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    }

    #[test]
    fn chromium_and_edge_helpers_and_form_factors_are_coherent() {
        assert!(chromium_user_agent_for_os(OsType::Windows, 157).contains("Chrome/157.0.0.0"));
        assert!(chromium_user_agent_for_os(OsType::Android, 157).contains("Android 10; K"));
        assert!(chromium_user_agent_for_os(OsType::Ios, 157).contains("CriOS/157.0.0.0"));
        assert!(edge_user_agent_for_os(OsType::Macos, 157).ends_with("Edg/157.0.0.0"));
        assert!(edge_user_agent_for_os(OsType::Android, 157).ends_with("EdgA/157.0.0.0"));
        assert!(edge_user_agent_for_os(OsType::Ios, 157).contains("EdgiOS/157.0.0.0"));

        assert_eq!(chromium_hints(157).form_factors, ["Desktop"]);
        assert_eq!(edge_hints(157).form_factors, ["Desktop"]);

        for os in [
            OsType::Windows,
            OsType::Macos,
            OsType::Linux,
            OsType::Android,
            OsType::Ios,
        ] {
            for seed in 1..=25 {
                let fp = crate::evasion::fingerprint::Fingerprint::randomized(os, seed);
                let report = fp.validate();
                assert!(
                    report.errors.is_empty(),
                    "{os:?} seed {seed}: {:?}",
                    report.errors
                );
                assert!(
                    report.warnings.is_empty(),
                    "{os:?} seed {seed}: {:?}",
                    report.warnings
                );
                if os == OsType::Android {
                    let ch = fp.client_hints().value().expect("Android client hints");
                    assert_eq!(ch.form_factors, ["Mobile"]);
                    assert!(ch.mobile);
                } else if os != OsType::Ios {
                    let ch = fp.client_hints().value().expect("Desktop client hints");
                    assert_eq!(ch.form_factors, ["Desktop"]);
                    assert!(!ch.mobile);
                }
            }
        }
    }

    #[test]
    fn weighted_draws_favor_common_hardware_over_legacy_tail() {
        let mut rtx_common = 0usize;
        let mut legacy_gt1030 = 0usize;
        for seed in 0..300 {
            let renderer = draw(OsType::Windows, seed, &Pins::default())
                .profile
                .graphics
                .renderer
                .unwrap_or_default();
            if renderer.contains("RTX 4060") || renderer.contains("RTX 3060") {
                rtx_common += 1;
            }
            if renderer.contains("GT 1030") {
                legacy_gt1030 += 1;
            }
        }
        assert!(
            rtx_common > legacy_gt1030,
            "expected weighted common GPUs ({rtx_common}) to outnumber GT 1030 ({legacy_gt1030})"
        );
    }
}
