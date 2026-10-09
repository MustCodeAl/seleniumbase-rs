//! Generates a random but coherent browser identity from a seed.
//!
//! A detector does not only look at each signal; it checks that the signals
//! agree with each other. A macOS user agent over a Direct3D renderer, a Tokyo
//! time zone under an `en-US` locale, or a phone-sized screen on a machine with
//! sixteen cores each give a fake identity away. Randomising one field at a time
//! produces exactly those mismatches, so [`generate`] does not. It draws a whole
//! machine (a graphics adapter or device model from a table of real ones, with
//! the core count, memory and screens such a machine ships with) and a whole
//! place (a locale, its time zone and a spot in that time zone), then derives
//! every field of the [`Fingerprint`] from them.
//!
//! The public entry point is [`Fingerprint::randomized`]; this module holds the
//! tables and the assembly. Everything here is pure and uses the crate's
//! seedable [`Rng`], so a seed always gives the same identity.

use super::fingerprint::{
    BrandVersion, BrowserType, ClientHints, Fingerprint, OsType, StealthFlags,
};
use super::humanize::Rng;

/// The Chromium major versions a generated identity may claim: the recent
/// stable releases. Refresh with each Chromium release.
const CHROMIUM_MAJORS: [u32; 3] = [153, 154, 155];

/// Mobile Safari releases a generated iPhone identity may claim.
const MOBILE_SAFARI_VERSIONS: [&str; 4] = ["17.7", "18.1", "18.3.1", "18.5"];

/// `navigator.vendor` of every Chromium browser.
const CHROMIUM_VENDOR: &str = "Google Inc.";

/// `navigator.vendor` of Safari.
const SAFARI_VENDOR: &str = "Apple Computer, Inc.";

/// A screen as the page sees it: size in CSS pixels and the device pixel ratio.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Screen {
    width: u32,
    height: u32,
    pixel_ratio: f64,
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
enum Tier {
    /// Laptops and office desktops with integrated or entry graphics.
    Entry,
    /// Mainstream gaming and workstation desktops.
    Mid,
    /// Enthusiast desktops.
    High,
}

/// What a [`Tier`] of PC reports. Repeating a value weights it.
#[derive(Debug)]
struct TierSpec {
    /// `navigator.hardwareConcurrency`.
    cores: [u32; 3],
    /// `navigator.deviceMemory` (Chromium caps it at 8).
    memory_gb: [f64; 3],
    /// Screens on Windows, where fractional display scaling is common.
    windows_screens: [Screen; 4],
    /// Screens on Linux.
    linux_screens: [Screen; 4],
}

static ENTRY: TierSpec = TierSpec {
    cores: [4, 4, 8],
    memory_gb: [4.0, 8.0, 8.0],
    windows_screens: [
        Screen::new(1366, 768, 1.0),
        Screen::new(1920, 1080, 1.0),
        Screen::new(1536, 864, 1.25),
        Screen::new(1280, 720, 1.5),
    ],
    linux_screens: [
        Screen::new(1366, 768, 1.0),
        Screen::new(1920, 1080, 1.0),
        Screen::new(1600, 900, 1.0),
        Screen::new(1920, 1080, 1.0),
    ],
};

static MID: TierSpec = TierSpec {
    cores: [6, 8, 12],
    memory_gb: [8.0, 8.0, 8.0],
    windows_screens: [
        Screen::new(1920, 1080, 1.0),
        Screen::new(1536, 864, 1.25),
        Screen::new(1600, 900, 1.0),
        Screen::new(1920, 1080, 1.0),
    ],
    linux_screens: [
        Screen::new(1920, 1080, 1.0),
        Screen::new(1920, 1200, 1.0),
        Screen::new(2560, 1440, 1.0),
        Screen::new(1920, 1080, 1.0),
    ],
};

static HIGH: TierSpec = TierSpec {
    cores: [12, 16, 20],
    memory_gb: [8.0, 8.0, 8.0],
    windows_screens: [
        Screen::new(2560, 1440, 1.0),
        Screen::new(1920, 1080, 1.0),
        Screen::new(1920, 1080, 2.0),
        Screen::new(3840, 2160, 1.0),
    ],
    linux_screens: [
        Screen::new(2560, 1440, 1.0),
        Screen::new(3840, 2160, 1.0),
        Screen::new(1920, 1080, 2.0),
        Screen::new(2560, 1440, 1.0),
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
enum GpuVendor {
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
struct Adapter {
    vendor: GpuVendor,
    /// On Windows the adapter name Direct3D reports; on Linux the part of the
    /// renderer string after the maker, as Mesa or the NVIDIA driver words it.
    name: &'static str,
    /// The PCI device id.
    device_id: u16,
    tier: Tier,
}

impl Adapter {
    const fn new(vendor: GpuVendor, name: &'static str, device_id: u16, tier: Tier) -> Self {
        Self {
            vendor,
            name,
            device_id,
            tier,
        }
    }
}

/// Windows adapters, named the way Chrome's ANGLE layer reports them.
#[rustfmt::skip]
static WINDOWS_ADAPTERS: [Adapter; 16] = [
    Adapter::new(GpuVendor::Nvidia, "NVIDIA GeForce GTX 1650", 0x1f82, Tier::Entry),
    Adapter::new(GpuVendor::Nvidia, "NVIDIA GeForce GTX 1660 SUPER", 0x21c4, Tier::Mid),
    Adapter::new(GpuVendor::Nvidia, "NVIDIA GeForce RTX 2060", 0x1f08, Tier::Mid),
    Adapter::new(GpuVendor::Nvidia, "NVIDIA GeForce RTX 3050", 0x2507, Tier::Mid),
    Adapter::new(GpuVendor::Nvidia, "NVIDIA GeForce RTX 3060", 0x2503, Tier::Mid),
    Adapter::new(GpuVendor::Nvidia, "NVIDIA GeForce RTX 3070", 0x2484, Tier::High),
    Adapter::new(GpuVendor::Nvidia, "NVIDIA GeForce RTX 4060", 0x28a0, Tier::Mid),
    Adapter::new(GpuVendor::Nvidia, "NVIDIA GeForce RTX 4070", 0x2786, Tier::High),
    Adapter::new(GpuVendor::Intel, "Intel(R) UHD Graphics 620", 0x5917, Tier::Entry),
    Adapter::new(GpuVendor::Intel, "Intel(R) UHD Graphics 630", 0x3e9b, Tier::Entry),
    Adapter::new(GpuVendor::Intel, "Intel(R) Iris(R) Xe Graphics", 0x46a6, Tier::Entry),
    Adapter::new(GpuVendor::Intel, "Intel(R) UHD Graphics 770", 0x4680, Tier::Mid),
    Adapter::new(GpuVendor::Amd, "AMD Radeon(TM) Graphics", 0x1638, Tier::Entry),
    Adapter::new(GpuVendor::Amd, "AMD Radeon RX 580 Series", 0x67df, Tier::Mid),
    Adapter::new(GpuVendor::Amd, "AMD Radeon RX 6600", 0x73ff, Tier::Mid),
    Adapter::new(GpuVendor::Amd, "AMD Radeon RX 6700 XT", 0x73df, Tier::High),
];

/// Linux adapters, worded the way Mesa and the NVIDIA driver word them.
static LINUX_ADAPTERS: [Adapter; 10] = [
    Adapter::new(
        GpuVendor::Intel,
        "Mesa Intel(R) UHD Graphics 630 (CFL GT2), OpenGL 4.6",
        0x3e9b,
        Tier::Entry,
    ),
    Adapter::new(
        GpuVendor::Intel,
        "Mesa Intel(R) UHD Graphics 620 (KBL GT2), OpenGL 4.6",
        0x5917,
        Tier::Entry,
    ),
    Adapter::new(
        GpuVendor::Intel,
        "Mesa Intel(R) Iris(R) Xe Graphics (TGL GT2), OpenGL 4.6",
        0x9a49,
        Tier::Entry,
    ),
    Adapter::new(
        GpuVendor::Intel,
        "Mesa Intel(R) UHD Graphics 770 (ADL-S GT1), OpenGL 4.6",
        0x4680,
        Tier::Mid,
    ),
    Adapter::new(
        GpuVendor::Amd,
        "AMD Radeon RX 6600 (radeonsi, navi23, LLVM 17.0.6, DRM 3.57, 6.8.0-45-generic), OpenGL 4.6",
        0x73ff,
        Tier::Mid,
    ),
    Adapter::new(
        GpuVendor::Amd,
        "AMD Radeon RX 580 Series (radeonsi, polaris10, LLVM 15.0.7, DRM 3.49, 6.1.0-18-amd64), OpenGL 4.6",
        0x67df,
        Tier::Mid,
    ),
    Adapter::new(
        GpuVendor::Amd,
        "AMD Radeon RX 7800 XT (radeonsi, navi32, LLVM 17.0.6, DRM 3.57, 6.8.0-45-generic), OpenGL 4.6",
        0x747e,
        Tier::High,
    ),
    Adapter::new(
        GpuVendor::Nvidia,
        "NVIDIA GeForce GTX 1660 SUPER/PCIe/SSE2, OpenGL 4.5.0 NVIDIA 535.183.01",
        0x21c4,
        Tier::Mid,
    ),
    Adapter::new(
        GpuVendor::Nvidia,
        "NVIDIA GeForce RTX 3060/PCIe/SSE2, OpenGL 4.5.0 NVIDIA 550.107.02",
        0x2503,
        Tier::Mid,
    ),
    Adapter::new(
        GpuVendor::Nvidia,
        "NVIDIA GeForce RTX 4070/PCIe/SSE2, OpenGL 4.5.0 NVIDIA 550.107.02",
        0x2786,
        Tier::High,
    ),
];

/// An Apple Silicon Mac: the chip, its logical core count, and the screen that
/// ships with the model that carries it.
#[derive(Clone, Copy, Debug)]
struct MacModel {
    chip: &'static str,
    cores: u32,
    screen: Screen,
}

impl MacModel {
    const fn new(chip: &'static str, cores: u32, screen: Screen) -> Self {
        Self {
            chip,
            cores,
            screen,
        }
    }
}

static MAC_MODELS: [MacModel; 13] = [
    MacModel::new("Apple M1", 8, Screen::new(1440, 900, 2.0)),
    MacModel::new("Apple M1 Pro", 10, Screen::new(1512, 982, 2.0)),
    MacModel::new("Apple M1 Max", 10, Screen::new(1728, 1117, 2.0)),
    MacModel::new("Apple M2", 8, Screen::new(1470, 956, 2.0)),
    MacModel::new("Apple M2", 8, Screen::new(1920, 1080, 1.0)),
    MacModel::new("Apple M2 Pro", 12, Screen::new(1512, 982, 2.0)),
    MacModel::new("Apple M2 Max", 12, Screen::new(1728, 1117, 2.0)),
    MacModel::new("Apple M3", 8, Screen::new(1470, 956, 2.0)),
    MacModel::new("Apple M3", 8, Screen::new(2240, 1260, 2.0)),
    MacModel::new("Apple M3 Pro", 12, Screen::new(1512, 982, 2.0)),
    MacModel::new("Apple M3 Max", 16, Screen::new(1728, 1117, 2.0)),
    MacModel::new("Apple M4", 10, Screen::new(1512, 982, 2.0)),
    MacModel::new("Apple M4 Pro", 14, Screen::new(1728, 1117, 2.0)),
];

/// macOS releases a generated Mac may run. All are recent enough for every chip
/// in [`MAC_MODELS`].
const MACOS_VERSIONS: [&str; 4] = ["15.1.0", "15.3.1", "15.5.0", "15.6.0"];

/// Windows platform versions as Client Hints report them (Windows 10, then
/// successive Windows 11 releases).
const WINDOWS_VERSIONS: [&str; 4] = ["10.0.0", "13.0.0", "15.0.0", "19.0.0"];

/// Linux kernel versions as Client Hints report them.
const LINUX_VERSIONS: [&str; 3] = ["5.15.0", "6.5.0", "6.8.0"];

/// A phone GPU as WebGL reports it: `UNMASKED_VENDOR_WEBGL` and
/// `UNMASKED_RENDERER_WEBGL`.
type PhoneGpu = (&'static str, &'static str);

const ADRENO_740: PhoneGpu = ("Qualcomm", "Adreno (TM) 740");
const MALI_G715: PhoneGpu = ("ARM", "Mali-G715");
const MALI_G710: PhoneGpu = ("ARM", "Mali-G710");
const MALI_G78: PhoneGpu = ("ARM", "Mali-G78");

/// An Android phone, with the graphics chip it is built around.
#[derive(Clone, Copy, Debug)]
struct AndroidDevice {
    /// The model name that Client Hints report.
    model: &'static str,
    /// The Android release that Client Hints report as the platform version.
    android: &'static str,
    screen: Screen,
    gpu: PhoneGpu,
}

impl AndroidDevice {
    const fn new(
        model: &'static str,
        android: &'static str,
        screen: Screen,
        gpu: PhoneGpu,
    ) -> Self {
        Self {
            model,
            android,
            screen,
            gpu,
        }
    }
}

#[rustfmt::skip]
static ANDROID_DEVICES: [AndroidDevice; 7] = [
    AndroidDevice::new("Pixel 8", "14.0.0", Screen::new(412, 915, 2.625), MALI_G715),
    AndroidDevice::new("Pixel 7", "14.0.0", Screen::new(412, 915, 2.625), MALI_G710),
    AndroidDevice::new("Pixel 7 Pro", "14.0.0", Screen::new(412, 892, 3.5), MALI_G710),
    AndroidDevice::new("SM-S911B", "14.0.0", Screen::new(360, 780, 3.0), ADRENO_740),
    AndroidDevice::new("SM-S918B", "14.0.0", Screen::new(412, 883, 3.5), ADRENO_740),
    AndroidDevice::new("CPH2449", "13.0.0", Screen::new(412, 919, 3.5), ADRENO_740),
    AndroidDevice::new("SM-G991B", "13.0.0", Screen::new(360, 800, 3.0), MALI_G78),
];

/// iPhone screens in CSS pixels.
static IPHONE_SCREENS: [Screen; 6] = [
    Screen::new(375, 667, 2.0),
    Screen::new(375, 812, 3.0),
    Screen::new(390, 844, 3.0),
    Screen::new(393, 852, 3.0),
    Screen::new(402, 874, 3.0),
    Screen::new(430, 932, 3.0),
];

/// A place: a locale, the time zone it is spoken in, and a city in that zone.
#[derive(Clone, Copy, Debug)]
struct Region {
    /// `navigator.language`, such as `de-DE`.
    locale: &'static str,
    /// The IANA time zone.
    timezone: &'static str,
    latitude: f64,
    longitude: f64,
}

impl Region {
    const fn new(
        locale: &'static str,
        timezone: &'static str,
        latitude: f64,
        longitude: f64,
    ) -> Self {
        Self {
            locale,
            timezone,
            latitude,
            longitude,
        }
    }

    /// `navigator.languages` and `Accept-Language`, most preferred first, in the
    /// shape Chrome sends them: the locale, its bare language, then English.
    fn languages(&self) -> String {
        let locale = self.locale;
        let language = locale.split('-').next().unwrap_or(locale);
        if language == "en" {
            format!("{locale},en;q=0.9")
        } else {
            format!("{locale},{language};q=0.9,en-US;q=0.8,en;q=0.7")
        }
    }
}

/// The places a generated identity may be in. The US appears once per time zone
/// so that it is the most common, as it is on the web.
static REGIONS: [Region; 17] = [
    Region::new("en-US", "America/New_York", 40.7128, -74.0060),
    Region::new("en-US", "America/Chicago", 41.8781, -87.6298),
    Region::new("en-US", "America/Denver", 39.7392, -104.9903),
    Region::new("en-US", "America/Los_Angeles", 34.0522, -118.2437),
    Region::new("en-GB", "Europe/London", 51.5074, -0.1278),
    Region::new("en-CA", "America/Toronto", 43.6532, -79.3832),
    Region::new("en-AU", "Australia/Sydney", -33.8688, 151.2093),
    Region::new("en-IN", "Asia/Kolkata", 19.0760, 72.8777),
    Region::new("de-DE", "Europe/Berlin", 52.5200, 13.4050),
    Region::new("fr-FR", "Europe/Paris", 48.8566, 2.3522),
    Region::new("es-ES", "Europe/Madrid", 40.4168, -3.7038),
    Region::new("it-IT", "Europe/Rome", 41.9028, 12.4964),
    Region::new("nl-NL", "Europe/Amsterdam", 52.3676, 4.9041),
    Region::new("pt-BR", "America/Sao_Paulo", -23.5505, -46.6333),
    Region::new("ja-JP", "Asia/Tokyo", 35.6762, 139.6503),
    Region::new("pl-PL", "Europe/Warsaw", 52.2297, 21.0122),
    Region::new("sv-SE", "Europe/Stockholm", 59.3293, 18.0686),
];

/// How far a reported position may sit from the city centre, in degrees
/// (about four kilometres).
const POSITION_JITTER_DEGREES: f64 = 0.04;

/// The WebGL strings, ids and so on that a graphics stack reports.
#[derive(Debug)]
struct Graphics {
    vendor: String,
    renderer: String,
    vendor_id: Option<String>,
    renderer_id: Option<String>,
}

impl Graphics {
    /// A PC adapter behind ANGLE: `maker` is how ANGLE names the maker.
    fn pc(adapter: &Adapter, maker: &str, renderer: String) -> Self {
        Self {
            vendor: format!("Google Inc. ({maker})"),
            renderer,
            vendor_id: Some(format!("0x{:04x}", adapter.vendor.pci_id())),
            renderer_id: Some(format!("0x{:04x}", adapter.device_id)),
        }
    }

    /// A phone or tablet, which reports its chip directly.
    fn direct((vendor, renderer): PhoneGpu) -> Self {
        Self {
            vendor: vendor.to_owned(),
            renderer: renderer.to_owned(),
            vendor_id: None,
            renderer_id: None,
        }
    }
}

/// Everything that is decided by the operating system and the machine.
#[derive(Debug)]
struct Profile {
    browser_type: BrowserType,
    /// The Chromium major the user agent claims, when it is Chromium.
    core_version: Option<u32>,
    user_agent: String,
    vendor: &'static str,
    client_hints: Option<ClientHints>,
    cores: u32,
    memory_gb: Option<f64>,
    touch_points: u32,
    screen: Screen,
    color_depth: u32,
    graphics: Graphics,
}

/// Builds an identity for `os` that is fully determined by `seed`.
pub(super) fn generate(os: OsType, seed: u64) -> Fingerprint {
    let mut rng = Rng::new(seed);
    // The noise seed gets a draw of its own, so it is not simply the caller's seed.
    let noise_seed = rng.next_u64();
    let region = *rng.pick(&REGIONS);
    let profile = match os {
        OsType::Windows => windows(&mut rng),
        OsType::Macos => macos(&mut rng),
        OsType::Linux => linux(&mut rng),
        OsType::Android => android(&mut rng),
        OsType::Ios => ios(&mut rng),
    };

    let latitude = round_degrees(
        region.latitude + rng.range(-POSITION_JITTER_DEGREES, POSITION_JITTER_DEGREES),
    );
    let longitude = round_degrees(
        region.longitude + rng.range(-POSITION_JITTER_DEGREES, POSITION_JITTER_DEGREES),
    );
    let accuracy = rng.range(20.0, 150.0).round();
    let languages = region.languages();

    Fingerprint {
        browser_type: profile.browser_type,
        os_type: os,
        core_version: profile.core_version,
        user_agent: Some(profile.user_agent),
        platform: Some(os.platform().to_owned()),
        hardware_concurrency: Some(profile.cores),
        device_memory: profile.memory_gb,
        max_touch_points: Some(profile.touch_points),
        vendor: Some(profile.vendor.to_owned()),
        locale: Some(region.locale.to_owned()),
        accept_languages: Some(languages.clone()),
        languages: Some(languages),
        timezone: Some(region.timezone.to_owned()),
        screen_width: Some(profile.screen.width),
        screen_height: Some(profile.screen.height),
        pixel_ratio: Some(profile.screen.pixel_ratio),
        color_depth: Some(profile.color_depth),
        webgl_vendor: Some(profile.graphics.vendor),
        webgl_renderer: Some(profile.graphics.renderer),
        webgl_vendor_id: profile.graphics.vendor_id,
        webgl_renderer_id: profile.graphics.renderer_id,
        latitude: Some(latitude),
        longitude: Some(longitude),
        accuracy: Some(accuracy),
        seed: Some(noise_seed),
        client_hints: profile.client_hints,
        flags: StealthFlags::balanced(),
        ..Fingerprint::default()
    }
}

/// Rounds a coordinate to four decimal places (about eleven metres).
fn round_degrees(degrees: f64) -> f64 {
    (degrees * 10_000.0).round() / 10_000.0
}

fn windows(rng: &mut Rng) -> Profile {
    let major = *rng.pick(&CHROMIUM_MAJORS);
    let adapter = rng.pick(&WINDOWS_ADAPTERS);
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
    Profile {
        browser_type: BrowserType::Chromium,
        core_version: Some(major),
        user_agent: chromium_user_agent("Windows NT 10.0; Win64; x64", major, false),
        vendor: CHROMIUM_VENDOR,
        client_hints: Some(client_hints),
        cores: *rng.pick(&spec.cores),
        memory_gb: Some(*rng.pick(&spec.memory_gb)),
        touch_points: 0,
        screen: *rng.pick(&spec.windows_screens),
        color_depth: 24,
        graphics: Graphics::pc(adapter, maker, renderer),
    }
}

fn macos(rng: &mut Rng) -> Profile {
    let major = *rng.pick(&CHROMIUM_MAJORS);
    let model = rng.pick(&MAC_MODELS);
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
        user_agent: chromium_user_agent("Macintosh; Intel Mac OS X 10_15_7", major, false),
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
        graphics: Graphics {
            vendor: "Google Inc. (Apple)".to_owned(),
            renderer: format!(
                "ANGLE (Apple, ANGLE Metal Renderer: {}, Unspecified Version)",
                model.chip
            ),
            vendor_id: None,
            renderer_id: None,
        },
    }
}

fn linux(rng: &mut Rng) -> Profile {
    let major = *rng.pick(&CHROMIUM_MAJORS);
    let adapter = rng.pick(&LINUX_ADAPTERS);
    let spec = adapter.tier.spec();
    let maker = adapter.vendor.opengl_name();
    let renderer = format!("ANGLE ({maker}, {})", adapter.name);
    let client_hints = ClientHints {
        platform: "Linux".to_owned(),
        platform_version: (*rng.pick(&LINUX_VERSIONS)).to_owned(),
        architecture: "x86".to_owned(),
        bitness: "64".to_owned(),
        ..chromium_hints(major)
    };
    Profile {
        browser_type: BrowserType::Chromium,
        core_version: Some(major),
        user_agent: chromium_user_agent("X11; Linux x86_64", major, false),
        vendor: CHROMIUM_VENDOR,
        client_hints: Some(client_hints),
        cores: *rng.pick(&spec.cores),
        memory_gb: Some(*rng.pick(&spec.memory_gb)),
        touch_points: 0,
        screen: *rng.pick(&spec.linux_screens),
        color_depth: 24,
        graphics: Graphics::pc(adapter, maker, renderer),
    }
}

fn android(rng: &mut Rng) -> Profile {
    let major = *rng.pick(&CHROMIUM_MAJORS);
    let device = rng.pick(&ANDROID_DEVICES);
    let client_hints = ClientHints {
        platform: "Android".to_owned(),
        platform_version: device.android.to_owned(),
        // Chrome on Android reports no architecture or bitness.
        architecture: String::new(),
        bitness: String::new(),
        model: device.model.to_owned(),
        mobile: true,
        ..chromium_hints(major)
    };
    Profile {
        browser_type: BrowserType::Chromium,
        core_version: Some(major),
        // Chromium freezes the Android version and hides the model in the
        // user agent; both are only in the Client Hints.
        user_agent: chromium_user_agent("Linux; Android 10; K", major, true),
        vendor: CHROMIUM_VENDOR,
        client_hints: Some(client_hints),
        cores: 8,
        memory_gb: Some(8.0),
        touch_points: 5,
        screen: device.screen,
        color_depth: 24,
        graphics: Graphics::direct(device.gpu),
    }
}

fn ios(rng: &mut Rng) -> Profile {
    let version = *rng.pick(&MOBILE_SAFARI_VERSIONS);
    let os_version = version.replace('.', "_");
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
        cores: *rng.pick(&[4, 6, 6]),
        // Safari has no `navigator.deviceMemory` either.
        memory_gb: None,
        touch_points: 5,
        screen: *rng.pick(&IPHONE_SCREENS),
        color_depth: 24,
        graphics: Graphics::direct(("Apple Inc.", "Apple GPU")),
    }
}

/// A Chromium user agent. Chromium freezes everything after the major version.
fn chromium_user_agent(platform: &str, major: u32, mobile: bool) -> String {
    let mobile = if mobile { " Mobile" } else { "" };
    format!(
        "Mozilla/5.0 ({platform}) AppleWebKit/537.36 (KHTML, like Gecko) \
         Chrome/{major}.0.0.0{mobile} Safari/537.36"
    )
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

/// The brand lists and full version of a Chromium release, built the way
/// Chromium builds them: a decoy brand whose wording, version and position are
/// all derived from the major version. The caller fills in the platform fields.
fn chromium_hints(major: u32) -> ClientHints {
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

    ClientHints {
        brands: list(decoy_version.to_owned(), major.to_string()),
        full_version_list: list(format!("{decoy_version}.0.0.0"), format!("{major}.0.0.0")),
        ua_full_version: format!("{major}.0.0.0"),
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
            assert_eq!(hints.ua_full_version, format!("{major}.0.0.0"));
        }
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
        let linux: Vec<_> = LINUX_ADAPTERS
            .iter()
            .map(|a| (a.vendor.pci_id(), a.device_id, a.name))
            .collect();
        assert_distinct(&linux, "LINUX_ADAPTERS");
        let macs: Vec<_> = MAC_MODELS
            .iter()
            .map(|m| (m.chip, m.cores, m.screen.width))
            .collect();
        assert_distinct(&macs, "MAC_MODELS");
        let phones: Vec<_> = ANDROID_DEVICES.iter().map(|d| d.model).collect();
        assert_distinct(&phones, "ANDROID_DEVICES");
        let places: Vec<_> = REGIONS.iter().map(|r| r.timezone).collect();
        assert_distinct(&places, "REGIONS");
    }

    #[test]
    fn languages_take_the_shape_chrome_sends() {
        let region = |locale| Region::new(locale, "Zone/Name", 0.0, 0.0);
        assert_eq!(region("en-US").languages(), "en-US,en;q=0.9");
        assert_eq!(region("en-GB").languages(), "en-GB,en;q=0.9");
        assert_eq!(
            region("de-DE").languages(),
            "de-DE,de;q=0.9,en-US;q=0.8,en;q=0.7"
        );
    }

    #[test]
    fn every_region_is_internally_consistent() {
        for region in &REGIONS {
            assert!(
                region.languages().starts_with(region.locale),
                "{} must lead its own language list",
                region.locale
            );
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
            .chain(IPHONE_SCREENS.iter());
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
            let fp = generate(os, 1);
            assert_eq!(fp.os_type, os);
            assert!(fp.validate().is_coherent());
        }
    }

    #[test]
    fn the_noise_seed_is_not_the_callers_seed() {
        assert_ne!(generate(OsType::Windows, 0).seed, Some(0));
        assert_ne!(generate(OsType::Windows, 42).seed, Some(42));
    }
}
