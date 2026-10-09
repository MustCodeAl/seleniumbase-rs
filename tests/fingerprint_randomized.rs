//! `Fingerprint::randomized`: determinism, coherence for every OS and seed,
//! variety, and that the generated values reach the evasion scripts.
//!
//! No browser is needed. Run with `cargo test --test fingerprint_randomized`.

use std::collections::BTreeSet;

use seleniumbase_rs::{default_registry, EvasionContext, Fingerprint, MaskingMode, OsType};

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

fn text(field: &Option<String>) -> &str {
    field.as_deref().unwrap_or_default()
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
fn different_seeds_give_different_identities() {
    for os in ALL_OS {
        let identities: BTreeSet<String> = (0..500)
            .map(|seed| serde_json::to_string(&Fingerprint::randomized(os, seed)).unwrap())
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
            distinct(os, &|f| text(&f.webgl_renderer).to_owned()) >= 8,
            "{os:?} renderer"
        );
        assert!(
            distinct(os, &|f| format!("{:?}", f.hardware_concurrency)) >= 3,
            "{os:?} cores"
        );
        assert!(
            distinct(os, &|f| format!(
                "{:?}x{:?}",
                f.screen_width, f.screen_height
            )) >= 4,
            "{os:?} screen"
        );
        assert!(
            distinct(os, &|f| text(&f.user_agent).to_owned()) >= 3,
            "{os:?} user agent"
        );
    }
    for os in ALL_OS {
        assert!(
            distinct(os, &|f| text(&f.timezone).to_owned()) >= 10,
            "{os:?} timezone"
        );
        assert!(
            distinct(os, &|f| text(&f.locale).to_owned()) >= 8,
            "{os:?} locale"
        );
        assert!(
            distinct(os, &|f| format!("{:?}", f.seed)) >= 790,
            "{os:?} noise seed"
        );
    }
    assert!(distinct(OsType::Android, &|f| text(&f.webgl_renderer).to_owned()) >= 3);
    assert!(
        distinct(OsType::Ios, &|f| format!(
            "{:?}x{:?}",
            f.screen_width, f.screen_height
        )) >= 4
    );
}

#[test]
fn windows_reports_direct3d_and_never_metal_or_mesa() {
    for seed in seeds() {
        let fp = Fingerprint::randomized(OsType::Windows, seed);
        let renderer = text(&fp.webgl_renderer);
        assert!(renderer.starts_with("ANGLE ("), "{renderer}");
        assert!(renderer.contains("Direct3D11"), "{renderer}");
        assert!(
            !renderer.contains("Metal") && !renderer.contains("Mesa"),
            "{renderer}"
        );
        assert!(!renderer.contains("OpenGL"), "{renderer}");
        assert_eq!(fp.platform.as_deref(), Some("Win32"));
        assert!(text(&fp.user_agent).contains("Windows NT"));
    }
}

#[test]
fn macos_reports_apple_silicon_through_metal_and_never_direct3d() {
    for seed in seeds() {
        let fp = Fingerprint::randomized(OsType::Macos, seed);
        let renderer = text(&fp.webgl_renderer);
        assert_eq!(text(&fp.webgl_vendor), "Google Inc. (Apple)");
        assert!(
            renderer.starts_with("ANGLE (Apple, ANGLE Metal Renderer: Apple M"),
            "{renderer}"
        );
        assert!(
            !renderer.contains("Direct3D") && !renderer.contains("Mesa"),
            "{renderer}"
        );
        assert_eq!(fp.platform.as_deref(), Some("MacIntel"));
        let hints = fp.client_hints.as_ref().unwrap();
        assert_eq!(hints.platform, "macOS");
        assert_eq!(hints.architecture, "arm", "Apple Silicon");
        // Retina panels report 30-bit colour; plain displays report 24.
        let retina = fp.pixel_ratio.unwrap() >= 2.0;
        assert_eq!(fp.color_depth, Some(if retina { 30 } else { 24 }));
        assert_eq!(fp.device_memory, Some(8.0));
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
        let renderer = text(&fp.webgl_renderer);
        assert!(renderer.starts_with("ANGLE ("), "{renderer}");
        assert!(renderer.contains("OpenGL"), "{renderer}");
        assert!(
            renderer.contains(marker(text(&fp.webgl_vendor))),
            "{renderer}"
        );
        assert!(
            !renderer.contains("Direct3D") && !renderer.contains("Metal"),
            "{renderer}"
        );
        assert_eq!(fp.platform.as_deref(), Some("Linux x86_64"));
        assert!(text(&fp.user_agent).contains("X11; Linux x86_64"));
    }
}

#[test]
fn android_reports_a_phone_gpu_and_a_touch_screen() {
    for seed in seeds() {
        let fp = Fingerprint::randomized(OsType::Android, seed);
        let renderer = text(&fp.webgl_renderer);
        assert!(
            renderer.starts_with("Adreno") || renderer.starts_with("Mali"),
            "{renderer}"
        );
        assert!(!renderer.contains("ANGLE"), "{renderer}");
        assert!(text(&fp.user_agent).contains("Android"));
        assert!(text(&fp.user_agent).contains("Mobile Safari"));
        assert_eq!(fp.max_touch_points, Some(5));
        assert!(fp.screen_width.unwrap() < 500, "a phone screen");
        let hints = fp.client_hints.as_ref().unwrap();
        assert!(hints.mobile);
        assert_eq!(hints.platform, "Android");
        assert!(!hints.model.is_empty());
    }
}

#[test]
fn ios_reports_safari_on_an_iphone() {
    for seed in seeds() {
        let fp = Fingerprint::randomized(OsType::Ios, seed);
        assert_eq!(text(&fp.webgl_vendor), "Apple Inc.");
        assert_eq!(text(&fp.webgl_renderer), "Apple GPU");
        assert_eq!(text(&fp.vendor), "Apple Computer, Inc.");
        assert_eq!(fp.platform.as_deref(), Some("iPhone"));
        assert!(text(&fp.user_agent).contains("iPhone; CPU iPhone OS"));
        assert!(text(&fp.user_agent).contains("Safari/604.1"));
        assert_eq!(fp.max_touch_points, Some(5));
        assert!(fp.screen_width.unwrap() < 500, "a phone screen");
        // Safari exposes neither Client Hints nor device memory.
        assert!(fp.client_hints.is_none());
        assert!(fp.device_memory.is_none());
    }
}

#[test]
fn desktops_have_no_touch_and_phones_are_never_wider_than_a_desktop() {
    for os in DESKTOP_OS {
        for seed in seeds().take(300) {
            let fp = Fingerprint::randomized(os, seed);
            assert_eq!(fp.max_touch_points, Some(0));
            assert!(fp.screen_width.unwrap() >= 1280, "{os:?} seed {seed}");
            assert!(!text(&fp.user_agent).contains("Mobile"));
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
            let locale = text(&fp.locale);
            let zone = text(&fp.timezone);
            assert!(
                zones_for(locale).contains(&zone),
                "{os:?} seed {seed}: {locale} paired with {zone}"
            );

            // The language list leads with the locale and is also the
            // Accept-Language header.
            let languages = text(&fp.languages);
            assert!(languages.starts_with(locale), "{languages} vs {locale}");
            assert_eq!(fp.accept_languages, fp.languages);

            // The position is in the time zone's city, within the jitter.
            let (lat, lon) = city_of(zone);
            let (got_lat, got_lon) = (fp.latitude.unwrap(), fp.longitude.unwrap());
            assert!((got_lat - lat).abs() <= 0.06, "{zone}: latitude {got_lat}");
            assert!((got_lon - lon).abs() <= 0.06, "{zone}: longitude {got_lon}");
            assert!((20.0..=150.0).contains(&fp.accuracy.unwrap()));
        }
    }
}

#[test]
fn hardware_values_are_plausible_for_the_os() {
    for os in ALL_OS {
        for seed in seeds() {
            let fp = Fingerprint::randomized(os, seed);
            let cores = fp.hardware_concurrency.unwrap();
            let ratio = fp.pixel_ratio.unwrap();
            let depth = fp.color_depth.unwrap();
            let (width, height) = (fp.screen_width.unwrap(), fp.screen_height.unwrap());

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
            if let Some(memory) = fp.device_memory {
                assert!(
                    [4.0, 8.0].contains(&memory),
                    "{os:?} seed {seed}: {memory} GB"
                );
            }
        }
    }
    // A phone has eight cores; an iPhone four or six.
    for seed in seeds() {
        assert_eq!(
            Fingerprint::randomized(OsType::Android, seed).hardware_concurrency,
            Some(8)
        );
        let cores = Fingerprint::randomized(OsType::Ios, seed)
            .hardware_concurrency
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
            let major = chrome_major(text(&fp.user_agent)).expect("a Chrome version");
            assert_eq!(fp.core_version, Some(major));

            let hints = fp.client_hints.as_ref().unwrap();
            assert_eq!(hints.ua_full_version, format!("{major}.0.0.0"));
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
            let vendor = text(&fp.webgl_vendor);
            let maker = vendor
                .strip_prefix("Google Inc. (")
                .and_then(|rest| rest.strip_suffix(')'))
                .unwrap_or_else(|| panic!("{os:?}: odd vendor {vendor}"));
            let renderer = text(&fp.webgl_renderer);
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
            let vendor_id = text(&fp.webgl_vendor_id);
            assert!(
                ["0x10de", "0x8086", "0x1002"].contains(&vendor_id),
                "{vendor_id}"
            );
            assert!(text(&fp.webgl_renderer_id).starts_with("0x"));
        }
        for os in [OsType::Macos, OsType::Android, OsType::Ios] {
            let fp = Fingerprint::randomized(os, seed);
            assert!(fp.webgl_vendor_id.is_none() && fp.webgl_renderer_id.is_none());
        }
    }
}

#[test]
fn the_windows_renderer_carries_the_adapters_device_id() {
    // Chrome words it `<name> (0x0000XXXX)`, and the same id is in the field.
    for seed in seeds().take(300) {
        let fp = Fingerprint::randomized(OsType::Windows, seed);
        let id = text(&fp.webgl_renderer_id)
            .trim_start_matches("0x")
            .to_uppercase();
        assert!(
            text(&fp.webgl_renderer).contains(&format!("(0x{id:0>8})")),
            "{} vs {id}",
            text(&fp.webgl_renderer)
        );
    }
}

#[test]
fn the_noise_seed_is_set_and_stable() {
    for os in ALL_OS {
        for seed in seeds().take(100) {
            let fp = Fingerprint::randomized(os, seed);
            let noise = fp.seed.expect("an explicit noise seed");
            assert_eq!(fp.seed_value(), noise, "seed_value must use it");
        }
    }
    assert_ne!(
        Fingerprint::randomized(OsType::Linux, 1).seed,
        Fingerprint::randomized(OsType::Linux, 2).seed
    );
}

#[test]
fn edge_seeds_do_not_panic_and_differ_from_each_other() {
    for os in ALL_OS {
        let all: BTreeSet<String> = EDGE_SEEDS
            .iter()
            .map(|seed| serde_json::to_string(&Fingerprint::randomized(os, *seed)).unwrap())
            .collect();
        assert_eq!(all.len(), EDGE_SEEDS.len(), "{os:?}");
    }
}

#[test]
fn the_masking_flags_apply_the_generated_values() {
    // A value that is generated but not applied would be a quiet failure.
    let fp = Fingerprint::randomized(OsType::Windows, 11);
    for mode in [
        fp.flags.navigator_masking,
        fp.flags.screen_masking,
        fp.flags.graphics_masking,
        fp.flags.timezone_masking,
        fp.flags.localization_masking,
        fp.flags.geolocation_masking,
    ] {
        assert_eq!(mode, MaskingMode::Mask);
    }
    assert!(!fp.flags.native_spoofing);
}

#[test]
fn the_generated_values_reach_the_bootstrap_script() {
    for os in DESKTOP_OS {
        for seed in [0, 1, 99, u64::MAX] {
            let fp = Fingerprint::randomized(os, seed);
            let script = default_registry().bootstrap(&EvasionContext::new(&fp));
            assert!(
                script.contains(text(&fp.webgl_renderer)),
                "{os:?} seed {seed}: renderer"
            );
            assert!(
                script.contains(text(&fp.webgl_vendor)),
                "{os:?} seed {seed}: vendor"
            );
            assert!(
                script.contains(text(&fp.timezone)),
                "{os:?} seed {seed}: time zone"
            );
            assert!(
                script.contains(text(&fp.user_agent)),
                "{os:?} seed {seed}: user agent"
            );
            assert!(
                script.contains(&format!("{}", fp.screen_width.unwrap())),
                "{os:?} seed {seed}: screen"
            );
        }
    }
}
