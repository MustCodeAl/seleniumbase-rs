//! `Fingerprint::randomized`: determinism, coherence for every OS and seed,
//! variety, and that the generated values reach the evasion scripts.
//!
//! No browser is needed. Run with `cargo test --test fingerprint_randomized`.

use std::collections::BTreeSet;

use seleniumbase_rs::stealth::fingerprint::{Geolocation, Spoof};
use seleniumbase_rs::stealth::randomize::known_full_version;
use seleniumbase_rs::{default_registry, Dimension, EvasionContext, Fingerprint, OsType};

const ALL_OS: [OsType; 5] = [
    OsType::Windows,
    OsType::Macos,
    OsType::Linux,
    OsType::Android,
    OsType::Ios,
];

const DESKTOP_OS: [OsType; 3] = [OsType::Windows, OsType::Macos, OsType::Linux];

/// Seeds that stress a generator: the ends of the range, powers of two, and
/// bit patterns that are all ones or alternating.
const EDGE_SEEDS: [u64; 10] = [
    0,
    1,
    2,
    u64::MAX,
    u64::MAX - 1,
    1 << 63,
    (1 << 63) - 1,
    0xAAAA_AAAA_AAAA_AAAA,
    0x5555_5555_5555_5555,
    0x9E37_79B9_7F4A_7C15,
];

/// Edge seeds followed by a long run of consecutive ones.
fn seeds() -> impl Iterator<Item = u64> {
    EDGE_SEEDS.into_iter().chain(0..1500)
}

fn text(setting: &Spoof<String>) -> &str {
    setting.as_str().unwrap_or_default()
}

fn user_agent(fp: &Fingerprint) -> &str {
    text(fp.user_agent())
}

fn webgl_vendor(fp: &Fingerprint) -> &str {
    fp.webgl()
        .value()
        .and_then(|gl| gl.vendor.as_deref())
        .unwrap_or_default()
}

fn webgl_renderer(fp: &Fingerprint) -> &str {
    fp.webgl()
        .value()
        .and_then(|gl| gl.renderer.as_deref())
        .unwrap_or_default()
}

fn screen(fp: &Fingerprint) -> (u32, u32) {
    let size = fp.screen().value().expect("a screen");
    (size.width, size.height)
}

fn position(fp: &Fingerprint) -> &Geolocation {
    fp.geolocation().value().expect("a position")
}

/// The identity as JSON, without the seed that makes any two of them differ.
fn without_seed(fp: &Fingerprint) -> String {
    let mut json = serde_json::to_value(fp).unwrap();
    json.as_object_mut().unwrap().remove("seed");
    json.to_string()
}

/// The Chromium major version in a user agent.
fn chrome_major(user_agent: &str) -> Option<u32> {
    let rest = user_agent.split("Chrome/").nth(1)?;
    rest.split('.').next()?.parse().ok()
}

#[test]
fn every_os_and_seed_validates_without_errors_or_warnings() {
    for os in ALL_OS {
        for seed in seeds() {
            let fp = Fingerprint::randomized(os, seed);
            let report = fp.validate();
            assert!(
                report.is_coherent() && report.warnings.is_empty(),
                "{os:?} seed {seed}: {report:?}"
            );
        }
    }
}

#[test]
fn the_same_seed_gives_the_same_identity() {
    for os in ALL_OS {
        for seed in seeds().take(200) {
            assert_eq!(
                Fingerprint::randomized(os, seed),
                Fingerprint::randomized(os, seed),
                "{os:?} seed {seed}"
            );
        }
    }
}

#[test]
fn an_identity_survives_a_json_round_trip() {
    for os in ALL_OS {
        for seed in seeds().take(100) {
            let fp = Fingerprint::randomized(os, seed);
            let json = serde_json::to_string(&fp).unwrap();
            let back: Fingerprint = serde_json::from_str(&json).unwrap();
            assert_eq!(fp, back, "{os:?} seed {seed}");
        }
    }
}

#[test]
fn an_identity_is_edited_through_its_builder_and_stays_an_identity() {
    for os in ALL_OS {
        let fp = Fingerprint::randomized(os, 21);
        let moved = fp
            .to_builder()
            .timezone("Asia/Tokyo")
            .geolocation((35.68, 139.65))
            .build()
            .unwrap();
        assert_eq!(text(moved.timezone()), "Asia/Tokyo");
        assert_eq!(moved.screen(), fp.screen());
        assert_eq!(moved.webgl(), fp.webgl());
        assert_eq!(moved.seed(), fp.seed());
        assert_eq!(fp.to_builder().build().unwrap(), fp, "{os:?}");
    }
}

#[test]
fn different_seeds_give_different_identities() {
    for os in ALL_OS {
        let identities: BTreeSet<String> = (0..500)
            .map(|seed| without_seed(&Fingerprint::randomized(os, seed)))
            .collect();
        assert!(
            identities.len() >= 495,
            "{os:?}: only {} of 500 seeds gave distinct identities",
            identities.len()
        );
    }
}

#[test]
fn identities_vary_along_every_dimension() {
    // Over many seeds each dimension takes several values, so that no field is
    // secretly constant. Phones and the iPhone have few devices, so the bar is
    // the number of distinct values their tables hold.
    let distinct = |os: OsType, pick: &dyn Fn(&Fingerprint) -> String| -> usize {
        (0..800)
            .map(|seed| pick(&Fingerprint::randomized(os, seed)))
            .collect::<BTreeSet<_>>()
            .len()
    };
    for os in DESKTOP_OS {
        assert!(
            distinct(os, &|f| webgl_renderer(f).to_owned()) >= 8,
            "{os:?} renderer"
        );
        assert!(
            distinct(os, &|f| format!("{:?}", f.hardware_concurrency())) >= 3,
            "{os:?} cores"
        );
        assert!(
            distinct(os, &|f| format!("{:?}", screen(f))) >= 4,
            "{os:?} screen"
        );
        assert!(
            distinct(os, &|f| user_agent(f).to_owned()) >= 3,
            "{os:?} user agent"
        );
    }
    for os in ALL_OS {
        assert!(
            distinct(os, &|f| text(f.timezone()).to_owned()) >= 10,
            "{os:?} timezone"
        );
        assert!(
            distinct(os, &|f| text(f.locale()).to_owned()) >= 8,
            "{os:?} locale"
        );
        assert!(
            distinct(os, &|f| format!("{:?}", f.seed())) >= 790,
            "{os:?} noise seed"
        );
    }
    assert!(distinct(OsType::Android, &|f| webgl_renderer(f).to_owned()) >= 3);
    assert!(distinct(OsType::Ios, &|f| format!("{:?}", screen(f))) >= 4);
}

#[test]
fn windows_reports_direct3d_and_never_metal_or_mesa() {
    for seed in seeds() {
        let fp = Fingerprint::randomized(OsType::Windows, seed);
        let renderer = webgl_renderer(&fp);
        assert!(renderer.starts_with("ANGLE ("), "{renderer}");
        assert!(renderer.contains("Direct3D11"), "{renderer}");
        assert!(
            !renderer.contains("Metal") && !renderer.contains("Mesa"),
            "{renderer}"
        );
        assert!(!renderer.contains("OpenGL"), "{renderer}");
        assert_eq!(fp.platform().as_str(), Some("Win32"));
        assert!(user_agent(&fp).contains("Windows NT"));
    }
}

#[test]
fn macos_reports_apple_silicon_through_metal_and_never_direct3d() {
    for seed in seeds() {
        let fp = Fingerprint::randomized(OsType::Macos, seed);
        let renderer = webgl_renderer(&fp);
        assert_eq!(webgl_vendor(&fp), "Google Inc. (Apple)");
        assert!(
            renderer.starts_with("ANGLE (Apple, ANGLE Metal Renderer: Apple M"),
            "{renderer}"
        );
        assert!(
            !renderer.contains("Direct3D") && !renderer.contains("Mesa"),
            "{renderer}"
        );
        assert_eq!(fp.platform().as_str(), Some("MacIntel"));
        let hints = fp.client_hints().value().unwrap();
        assert_eq!(hints.platform, "macOS");
        assert_eq!(hints.architecture, "arm", "Apple Silicon");
        // Retina panels report 30-bit colour; plain displays report 24.
        let retina = *fp.pixel_ratio().value().unwrap() >= 2.0;
        assert_eq!(
            fp.color_depth().value(),
            Some(&if retina { 30 } else { 24 })
        );
        assert_eq!(fp.device_memory().value(), Some(&8.0));
    }
}

#[test]
fn linux_reports_opengl_through_mesa_or_the_nvidia_driver() {
    // What each stack puts in its renderer string, keyed by the ANGLE maker.
    let marker = |vendor: &str| match vendor {
        "Google Inc. (Intel)" => "Mesa Intel",
        "Google Inc. (AMD)" => "radeonsi",
        "Google Inc. (NVIDIA Corporation)" => "/PCIe/SSE2",
        other => panic!("unexpected Linux vendor {other}"),
    };
    for seed in seeds() {
        let fp = Fingerprint::randomized(OsType::Linux, seed);
        let renderer = webgl_renderer(&fp);
        assert!(renderer.starts_with("ANGLE ("), "{renderer}");
        assert!(renderer.contains("OpenGL"), "{renderer}");
        assert!(renderer.contains(marker(webgl_vendor(&fp))), "{renderer}");
        assert!(
            !renderer.contains("Direct3D") && !renderer.contains("Metal"),
            "{renderer}"
        );
        assert_eq!(fp.platform().as_str(), Some("Linux x86_64"));
        assert!(user_agent(&fp).contains("X11; Linux x86_64"));
    }
}

#[test]
fn android_reports_a_phone_gpu_and_a_touch_screen() {
    for seed in seeds() {
        let fp = Fingerprint::randomized(OsType::Android, seed);
        let renderer = webgl_renderer(&fp);
        assert!(
            renderer.starts_with("Adreno") || renderer.starts_with("Mali"),
            "{renderer}"
        );
        assert!(!renderer.contains("ANGLE"), "{renderer}");
        assert!(user_agent(&fp).contains("Android"));
        assert!(user_agent(&fp).contains("Mobile Safari"));
        assert_eq!(fp.max_touch_points().value(), Some(&5));
        assert!(screen(&fp).0 < 500, "a phone screen");
        let hints = fp.client_hints().value().unwrap();
        assert!(hints.mobile);
        assert_eq!(hints.platform, "Android");
        assert!(!hints.model.is_empty());
    }
}

#[test]
fn ios_reports_safari_on_an_iphone() {
    for seed in seeds() {
        let fp = Fingerprint::randomized(OsType::Ios, seed);
        assert_eq!(webgl_vendor(&fp), "Apple Inc.");
        assert_eq!(webgl_renderer(&fp), "Apple GPU");
        assert_eq!(text(fp.vendor()), "Apple Computer, Inc.");
        assert_eq!(fp.platform().as_str(), Some("iPhone"));
        assert!(user_agent(&fp).contains("iPhone; CPU iPhone OS"));
        assert!(user_agent(&fp).contains("Safari/604.1"));
        assert_eq!(fp.max_touch_points().value(), Some(&5));
        assert!(screen(&fp).0 < 500, "a phone screen");
        // Safari exposes neither Client Hints nor device memory, and has no
        // battery, network information or Web Bluetooth.
        assert!(fp.client_hints().is_off());
        assert!(fp.device_memory().is_natural());
        assert!(fp.battery().is_off() && fp.connection().is_off() && fp.bluetooth().is_off());
    }
}

#[test]
fn desktops_have_no_touch_and_phones_are_never_wider_than_a_desktop() {
    for os in DESKTOP_OS {
        for seed in seeds().take(300) {
            let fp = Fingerprint::randomized(os, seed);
            assert_eq!(fp.max_touch_points().value(), Some(&0));
            assert!(screen(&fp).0 >= 1280, "{os:?} seed {seed}");
            assert!(!user_agent(&fp).contains("Mobile"));
        }
    }
}

/// The time zones each locale may be paired with. Written out here, apart from
/// the generator's own table, so that a bad pairing in the table is caught.
fn zones_for(locale: &str) -> &'static [&'static str] {
    match locale {
        "en-US" => &[
            "America/New_York",
            "America/Chicago",
            "America/Denver",
            "America/Los_Angeles",
        ],
        "en-GB" => &["Europe/London"],
        "en-CA" => &["America/Toronto"],
        "en-AU" => &["Australia/Sydney"],
        "en-IN" => &["Asia/Kolkata"],
        "de-DE" => &["Europe/Berlin"],
        "fr-FR" => &["Europe/Paris"],
        "es-ES" => &["Europe/Madrid"],
        "it-IT" => &["Europe/Rome"],
        "nl-NL" => &["Europe/Amsterdam"],
        "pt-BR" => &["America/Sao_Paulo"],
        "ja-JP" => &["Asia/Tokyo"],
        "pl-PL" => &["Europe/Warsaw"],
        "sv-SE" => &["Europe/Stockholm"],
        other => panic!("generated an unexpected locale {other}"),
    }
}

/// Roughly where a time zone's main city is, as `(latitude, longitude)`.
fn city_of(zone: &str) -> (f64, f64) {
    match zone {
        "America/New_York" => (40.71, -74.01),
        "America/Chicago" => (41.88, -87.63),
        "America/Denver" => (39.74, -104.99),
        "America/Los_Angeles" => (34.05, -118.24),
        "Europe/London" => (51.51, -0.13),
        "America/Toronto" => (43.65, -79.38),
        "Australia/Sydney" => (-33.87, 151.21),
        "Asia/Kolkata" => (19.08, 72.88),
        "Europe/Berlin" => (52.52, 13.41),
        "Europe/Paris" => (48.86, 2.35),
        "Europe/Madrid" => (40.42, -3.70),
        "Europe/Rome" => (41.90, 12.50),
        "Europe/Amsterdam" => (52.37, 4.90),
        "America/Sao_Paulo" => (-23.55, -46.63),
        "Asia/Tokyo" => (35.68, 139.65),
        "Europe/Warsaw" => (52.23, 21.01),
        "Europe/Stockholm" => (59.33, 18.07),
        other => panic!("generated an unexpected time zone {other}"),
    }
}

#[test]
fn locale_language_timezone_and_position_agree() {
    for os in ALL_OS {
        for seed in seeds().take(600) {
            let fp = Fingerprint::randomized(os, seed);
            let locale = text(fp.locale());
            let zone = text(fp.timezone());
            assert!(
                zones_for(locale).contains(&zone),
                "{os:?} seed {seed}: {locale} paired with {zone}"
            );

            // The language list leads with the locale and is also the
            // Accept-Language header.
            let languages = text(fp.languages());
            assert!(languages.starts_with(locale), "{languages} vs {locale}");
            assert_eq!(fp.accept_languages(), fp.languages());

            // The position is in the time zone's city, within the jitter.
            let (lat, lon) = city_of(zone);
            let place = position(&fp);
            assert!(
                (place.latitude - lat).abs() <= 0.06,
                "{zone}: latitude {}",
                place.latitude
            );
            assert!(
                (place.longitude - lon).abs() <= 0.06,
                "{zone}: longitude {}",
                place.longitude
            );
            assert!((20.0..=150.0).contains(&place.accuracy.unwrap()));
        }
    }
}

#[test]
fn a_random_place_follows_a_given_time_zone_for_every_zone_of_the_table() {
    for zone in [
        "Europe/Berlin",
        "Asia/Tokyo",
        "America/Chicago",
        "Australia/Sydney",
    ] {
        let (lat, lon) = city_of(zone);
        for seed in 0..40 {
            let fp = Fingerprint::builder()
                .seed(seed)
                .timezone(zone)
                .locale(Spoof::Random)
                .geolocation(Spoof::Random)
                .build()
                .unwrap();
            assert!(
                zones_for(text(fp.locale())).contains(&zone),
                "{zone}: {:?}",
                fp.locale()
            );
            let place = position(&fp);
            assert!((place.latitude - lat).abs() <= 0.06 && (place.longitude - lon).abs() <= 0.06);
        }
    }
}

#[test]
fn hardware_values_are_plausible_for_the_os() {
    for os in ALL_OS {
        for seed in seeds() {
            let fp = Fingerprint::randomized(os, seed);
            let cores = *fp.hardware_concurrency().value().unwrap();
            let ratio = *fp.pixel_ratio().value().unwrap();
            let depth = *fp.color_depth().value().unwrap();
            let (width, height) = screen(&fp);

            assert!(
                (4..=20).contains(&cores),
                "{os:?} seed {seed}: {cores} cores"
            );
            assert!(
                (1.0..=4.0).contains(&ratio),
                "{os:?} seed {seed}: ratio {ratio}"
            );
            assert!(
                [24, 30].contains(&depth),
                "{os:?} seed {seed}: depth {depth}"
            );
            assert!(
                width >= 360 && height >= 640,
                "{os:?} seed {seed}: {width}x{height}"
            );

            // `navigator.deviceMemory` is one of Chromium's buckets, capped at 8.
            if let Some(memory) = fp.device_memory().value() {
                assert!(
                    [4.0, 8.0].contains(memory),
                    "{os:?} seed {seed}: {memory} GB"
                );
            }
        }
    }
    // A phone has eight cores; an iPhone four or six.
    for seed in seeds() {
        assert_eq!(
            Fingerprint::randomized(OsType::Android, seed)
                .hardware_concurrency()
                .value(),
            Some(&8)
        );
        let cores = *Fingerprint::randomized(OsType::Ios, seed)
            .hardware_concurrency()
            .value()
            .unwrap();
        assert!([4, 6].contains(&cores));
    }
}

#[test]
fn client_hints_agree_with_the_user_agent() {
    for os in [
        OsType::Windows,
        OsType::Macos,
        OsType::Linux,
        OsType::Android,
    ] {
        for seed in seeds().take(300) {
            let fp = Fingerprint::randomized(os, seed);
            let major = chrome_major(user_agent(&fp)).expect("a Chrome version");
            assert_eq!(fp.core_version(), Some(major));

            let hints = fp.client_hints().value().unwrap();
            // The high-entropy full version is a real build of the major the
            // user agent claims, not the frozen `{major}.0.0.0`.
            let known = known_full_version(major).expect("a drawn major has a known build");
            assert_eq!(hints.ua_full_version, known.full_version);
            assert!(hints.ua_full_version.starts_with(&format!("{major}.")));
            let version_of = |brand: &str| {
                hints
                    .brands
                    .iter()
                    .find(|b| b.brand == brand)
                    .map(|b| b.version.as_str())
            };
            assert_eq!(version_of("Chromium"), Some(major.to_string().as_str()));
            assert_eq!(
                version_of("Google Chrome"),
                Some(major.to_string().as_str())
            );
            assert_eq!(hints.brands.len(), 3);
            assert!(hints.brands.iter().any(|b| b.brand.starts_with("Not")));
            assert_eq!(hints.full_version_list.len(), 3);
            assert_eq!(hints.mobile, os == OsType::Android);
        }
    }
}

#[test]
fn webgl_vendor_names_the_maker_that_the_renderer_names() {
    for os in DESKTOP_OS {
        for seed in seeds().take(600) {
            let fp = Fingerprint::randomized(os, seed);
            let vendor = webgl_vendor(&fp);
            let maker = vendor
                .strip_prefix("Google Inc. (")
                .and_then(|rest| rest.strip_suffix(')'))
                .unwrap_or_else(|| panic!("{os:?}: odd vendor {vendor}"));
            let renderer = webgl_renderer(&fp);
            assert!(
                renderer.starts_with(&format!("ANGLE ({maker},")),
                "{os:?} seed {seed}: {vendor} vs {renderer}"
            );
        }
    }
}

#[test]
fn pci_ids_are_present_for_pc_adapters_and_absent_elsewhere() {
    for seed in seeds().take(200) {
        for os in [OsType::Windows, OsType::Linux] {
            let fp = Fingerprint::randomized(os, seed);
            let gl = fp.webgl().value().unwrap();
            let vendor_id = gl.vendor_id.as_deref().unwrap_or_default();
            assert!(
                ["0x10de", "0x8086", "0x1002"].contains(&vendor_id),
                "{vendor_id}"
            );
            assert!(gl
                .renderer_id
                .as_deref()
                .unwrap_or_default()
                .starts_with("0x"));
        }
        for os in [OsType::Macos, OsType::Android, OsType::Ios] {
            let fp = Fingerprint::randomized(os, seed);
            let gl = fp.webgl().value().unwrap();
            assert!(gl.vendor_id.is_none() && gl.renderer_id.is_none());
        }
    }
}

#[test]
fn the_windows_renderer_carries_the_adapters_device_id() {
    // Chrome words it `<name> (0x0000XXXX)`, and the same id is in the field.
    for seed in seeds().take(300) {
        let fp = Fingerprint::randomized(OsType::Windows, seed);
        let gl = fp.webgl().value().unwrap();
        let id = gl
            .renderer_id
            .as_deref()
            .unwrap_or_default()
            .trim_start_matches("0x")
            .to_uppercase();
        assert!(
            webgl_renderer(&fp).contains(&format!("(0x{id:0>8})")),
            "{} vs {id}",
            webgl_renderer(&fp)
        );
    }
}

#[test]
fn the_noise_seed_is_the_seed_it_was_drawn_from() {
    for os in ALL_OS {
        for seed in seeds().take(100) {
            assert_eq!(Fingerprint::randomized(os, seed).seed(), seed);
        }
    }
    assert_ne!(
        Fingerprint::randomized(OsType::Linux, 1).seed(),
        Fingerprint::randomized(OsType::Linux, 2).seed()
    );
}

#[test]
fn edge_seeds_do_not_panic_and_differ_from_each_other() {
    for os in ALL_OS {
        let all: BTreeSet<String> = EDGE_SEEDS
            .iter()
            .map(|seed| without_seed(&Fingerprint::randomized(os, *seed)))
            .collect();
        assert_eq!(all.len(), EDGE_SEEDS.len(), "{os:?}");
    }
}

#[test]
fn the_policy_applies_the_generated_values() {
    // A value that is generated but not applied would be a quiet failure.
    for os in ALL_OS {
        let fp = Fingerprint::randomized(os, 11);
        for dimension in [
            Dimension::Navigator,
            Dimension::Screen,
            Dimension::Graphics,
            Dimension::Timezone,
            Dimension::Localization,
            Dimension::Geolocation,
            Dimension::Canvas,
            Dimension::Audio,
        ] {
            assert!(fp.is_masked(dimension), "{os:?}: {dimension:?} is off");
        }
        assert!(!fp.policy().native_spoofing);
    }
}

#[test]
fn the_generated_values_reach_the_bootstrap_script() {
    for os in DESKTOP_OS {
        for seed in [0, 1, 99, u64::MAX] {
            let fp = Fingerprint::randomized(os, seed);
            let script = default_registry().bootstrap(&EvasionContext::new(&fp));
            assert!(
                script.contains(webgl_renderer(&fp)),
                "{os:?} seed {seed}: renderer"
            );
            assert!(
                script.contains(webgl_vendor(&fp)),
                "{os:?} seed {seed}: vendor"
            );
            assert!(
                script.contains(text(fp.timezone())),
                "{os:?} seed {seed}: time zone"
            );
            assert!(
                script.contains(user_agent(&fp)),
                "{os:?} seed {seed}: user agent"
            );
            assert!(
                script.contains(&format!("{}", screen(&fp).0)),
                "{os:?} seed {seed}: screen"
            );
        }
    }
}
