//! Browser, machine, place and rendering specimens a fingerprint setting may mimic.
//!
//! A [`Specimen`] names a real browser release, device model, locale or recorded
//! output bundle. Settings in a [`Fingerprint`](crate::Fingerprint) can mimic a specimen
//! using [`Spoof::Mimic`](crate::evasion::spoof::Spoof::Mimic).

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::fingerprint::{BrowserType, OsType};
use super::randomize::{Brand, Locale, MachineRow, NamedMachine, BUILDS, MACHINES, PLACES};
use super::settings::WebGl;

/// What a `Spoof::Mimic` names: one real browser build, machine, place or rendering.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Specimen {
    /// A browser release on one system family.
    Build(Build),
    /// A device model or graphics adapter.
    Machine(Machine),
    /// A locale with its city and time zone.
    Place(Place),
    /// One build on one machine, recorded.
    Rendering(Rendering),
}

/// A browser release on one system family, such as `chrome-156-windows`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Build(pub(crate) &'static str);

/// A device model, such as `macbook-13-m1` or `windows-uhd-620`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Machine(pub(crate) &'static str);

/// A locale with its city, such as `de-de-berlin`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Place(pub(crate) &'static str);

/// One build on one machine, recorded: what the reference drawings return there.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Rendering(pub(crate) &'static str);

/// Where a recorded rendering comes from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    /// The date the recording was taken, ISO 8601.
    pub recorded: String,
    /// The test or binary that recorded the output.
    pub tool: String,
    /// The browser and its full version.
    pub browser: String,
    /// Whether the browser ran headless.
    pub headless: bool,
    /// The host operating system and hardware model.
    pub host: String,
}

/// A specimen name this version does not know.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct UnknownSpecimen {
    name: String,
    kind: Option<&'static str>,
    message: String,
}

impl UnknownSpecimen {
    /// The unknown name that was looked up.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The kind of specimen that was asked for, or `None` if looked up through [`Specimen::named`].
    #[must_use]
    pub fn kind(&self) -> Option<&'static str> {
        self.kind
    }

    pub(crate) fn for_kind(
        name: &str,
        kind: &'static str,
        candidates: impl IntoIterator<Item = &'static str>,
    ) -> Self {
        let list: Vec<&'static str> = candidates.into_iter().collect();
        let plural = match kind {
            "build" => "builds",
            "machine" => "machines",
            "place" => "places",
            "rendering" => "renderings",
            other => other,
        };
        let message = format!(
            "there is no {kind} called \"{name}\"; the {plural} are: {}",
            list.join(", ")
        );
        Self {
            name: name.to_owned(),
            kind: Some(kind),
            message,
        }
    }

    pub(crate) fn for_specimen(name: &str) -> Self {
        let mut lines = vec![
            format!("there is no build, machine, place or rendering called \"{name}\";"),
            format!(
                "builds: {}",
                Build::all()
                    .map(|b| b.name())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            format!(
                "machines: {}",
                Machine::all()
                    .map(|m| m.name())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            format!(
                "places: {}",
                Place::all()
                    .map(|p| p.name())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        ];
        let renderings: Vec<_> = Rendering::all().map(|r| r.name()).collect();
        if !renderings.is_empty() {
            lines.push(format!("renderings: {}", renderings.join(", ")));
        }
        let message = lines.join("\n");
        Self {
            name: name.to_owned(),
            kind: None,
            message,
        }
    }
}

impl From<UnknownSpecimen> for crate::error::SeleniumBaseError {
    fn from(err: UnknownSpecimen) -> Self {
        Self::InvalidConfig(err.to_string())
    }
}

impl Specimen {
    /// Looks up any specimen by name.
    ///
    /// # Errors
    ///
    /// Returns [`UnknownSpecimen`] if `name` matches no known build, machine, place or rendering.
    pub fn named(name: &str) -> Result<Self, UnknownSpecimen> {
        if let Ok(b) = Build::named(name) {
            return Ok(Self::Build(b));
        }
        if let Ok(m) = Machine::named(name) {
            return Ok(Self::Machine(m));
        }
        if let Ok(p) = Place::named(name) {
            return Ok(Self::Place(p));
        }
        if let Ok(r) = Rendering::named(name) {
            return Ok(Self::Rendering(r));
        }
        Err(UnknownSpecimen::for_specimen(name))
    }

    /// Every known specimen: builds, machines, places and renderings.
    pub fn all() -> impl Iterator<Item = Self> {
        Build::all()
            .map(Self::Build)
            .chain(Machine::all().map(Self::Machine))
            .chain(Place::all().map(Self::Place))
            .chain(Rendering::all().map(Self::Rendering))
    }

    /// The name of this specimen, in lower case with hyphens.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Build(b) => b.name(),
            Self::Machine(m) => m.name(),
            Self::Place(p) => p.name(),
            Self::Rendering(r) => r.name(),
        }
    }

    /// The kind of specimen: `"build"`, `"machine"`, `"place"` or `"rendering"`.
    #[must_use]
    pub const fn kind(self) -> &'static str {
        match self {
            Self::Build(_) => "build",
            Self::Machine(_) => "machine",
            Self::Place(_) => "place",
            Self::Rendering(_) => "rendering",
        }
    }
}

impl From<Build> for Specimen {
    fn from(b: Build) -> Self {
        Self::Build(b)
    }
}

impl From<Machine> for Specimen {
    fn from(m: Machine) -> Self {
        Self::Machine(m)
    }
}

impl From<Place> for Specimen {
    fn from(p: Place) -> Self {
        Self::Place(p)
    }
}

impl From<Rendering> for Specimen {
    fn from(r: Rendering) -> Self {
        Self::Rendering(r)
    }
}

impl fmt::Display for Specimen {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for Specimen {
    type Err = UnknownSpecimen;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::named(s)
    }
}

impl Serialize for Specimen {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.name())
    }
}

impl<'de> Deserialize<'de> for Specimen {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::named(&text).map_err(serde::de::Error::custom)
    }
}

static DYNAMIC_BUILDS: std::sync::Mutex<Vec<&'static str>> = std::sync::Mutex::new(Vec::new());

impl Build {
    /// Looks up a browser build by name.
    ///
    /// Accepts shipped catalogue builds as well as dynamic builds for any Chromium/Edge major
    /// version on any supported operating system (e.g. `chrome-128-windows`, `chrome-130-macos`).
    ///
    /// # Errors
    ///
    /// Returns [`UnknownSpecimen`] if `name` is not a known build release or valid pattern.
    pub fn named(name: &str) -> Result<Self, UnknownSpecimen> {
        if let Some(b) = BUILDS.iter().find(|b| b.name == name) {
            return Ok(Self(&b.name));
        }
        let parts: Vec<&str> = name.split('-').collect();
        if parts.len() == 3 {
            let (brand_str, major_str, os_str) = (parts[0], parts[1], parts[2]);
            let valid_brand = matches!(brand_str, "chrome" | "edge");
            let valid_os = matches!(os_str, "windows" | "macos" | "linux" | "android");
            if valid_brand && valid_os && major_str.parse::<u32>().is_ok() {
                if let Ok(mut cache) = DYNAMIC_BUILDS.lock() {
                    if let Some(&existing) = cache.iter().find(|&&s| s == name) {
                        return Ok(Self(existing));
                    }
                    let leaked: &'static str = Box::leak(name.to_string().into_boxed_str());
                    cache.push(leaked);
                    return Ok(Self(leaked));
                }
                let leaked: &'static str = Box::leak(name.to_string().into_boxed_str());
                return Ok(Self(leaked));
            }
        }
        Err(UnknownSpecimen::for_kind(
            name,
            "build",
            BUILDS.iter().map(|b| b.name.as_str()),
        ))
    }

    /// Every known browser build.
    pub fn all() -> impl Iterator<Item = Self> {
        BUILDS.iter().map(|b| Self(&b.name))
    }

    /// The name of this build.
    #[must_use]
    pub const fn name(self) -> &'static str {
        self.0
    }

    /// The browser engine this build claims.
    #[must_use]
    pub fn browser(self) -> BrowserType {
        if let Some(b) = BUILDS.iter().find(|b| b.name == self.0) {
            match b.brand {
                Brand::Chrome | Brand::Edge => BrowserType::Chromium,
                Brand::Safari => BrowserType::MobileSafari,
            }
        } else if self.0.starts_with("safari-") {
            BrowserType::MobileSafari
        } else {
            BrowserType::Chromium
        }
    }

    /// The operating system this build runs on.
    #[must_use]
    pub fn os(self) -> OsType {
        if let Some(b) = BUILDS.iter().find(|b| b.name == self.0) {
            b.os
        } else if let Some(os_part) = self.0.split('-').nth(2) {
            match os_part {
                "windows" => OsType::Windows,
                "macos" => OsType::Macos,
                "linux" => OsType::Linux,
                "android" => OsType::Android,
                _ => OsType::Windows,
            }
        } else {
            OsType::Windows
        }
    }

    /// The major version (e.g. 157 for Chrome, 18 or 26 for Safari).
    #[must_use]
    pub fn major(self) -> u32 {
        if let Some(b) = BUILDS.iter().find(|b| b.name == self.0) {
            b.major
        } else {
            self.0
                .split('-')
                .nth(1)
                .and_then(|s| s.parse().ok())
                .unwrap_or(157)
        }
    }
}

impl fmt::Display for Build {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for Build {
    type Err = UnknownSpecimen;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::named(s)
    }
}

impl Machine {
    /// Looks up a machine model or graphics adapter by name.
    ///
    /// # Errors
    ///
    /// Returns [`UnknownSpecimen`] if `name` is not a known machine.
    pub fn named(name: &str) -> Result<Self, UnknownSpecimen> {
        MACHINES
            .iter()
            .find(|m| m.name == name)
            .map(|m| Self(&m.name))
            .ok_or_else(|| {
                UnknownSpecimen::for_kind(name, "machine", MACHINES.iter().map(|m| m.name.as_str()))
            })
    }

    /// Every known machine.
    pub fn all() -> impl Iterator<Item = Self> {
        MACHINES.iter().map(|m| Self(&m.name))
    }

    /// The name of this machine.
    #[must_use]
    pub const fn name(self) -> &'static str {
        self.0
    }

    fn row(self) -> &'static NamedMachine {
        MACHINES
            .iter()
            .find(|m| m.name == self.0)
            .expect("static machine must exist")
    }

    /// The operating system this machine runs.
    #[must_use]
    pub fn os(self) -> OsType {
        match self.row().row {
            MachineRow::Pc(os, _) => os,
            MachineRow::Mac(_) => OsType::Macos,
            MachineRow::Android(_) => OsType::Android,
            MachineRow::Iphone(_) => OsType::Ios,
        }
    }

    /// The unmasked WebGL vendor and renderer this machine reports.
    #[must_use]
    pub fn webgl(self) -> WebGl {
        self.row().row.webgl()
    }
}

impl fmt::Display for Machine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for Machine {
    type Err = UnknownSpecimen;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::named(s)
    }
}

impl Place {
    /// Looks up a place by name.
    ///
    /// # Errors
    ///
    /// Returns [`UnknownSpecimen`] if `name` is not a known place.
    pub fn named(name: &str) -> Result<Self, UnknownSpecimen> {
        PLACES
            .iter()
            .find(|p| p.0 == name)
            .map(|p| Self(&p.0))
            .ok_or_else(|| {
                UnknownSpecimen::for_kind(name, "place", PLACES.iter().map(|p| p.0.as_str()))
            })
    }

    /// Every known place.
    pub fn all() -> impl Iterator<Item = Self> {
        PLACES.iter().map(|p| Self(&p.0))
    }

    /// The name of this place.
    #[must_use]
    pub const fn name(self) -> &'static str {
        self.0
    }

    /// The locale of this place.
    #[must_use]
    pub fn locale(self) -> &'static Locale {
        PLACES
            .iter()
            .find(|p| p.0 == self.0)
            .map(|p| p.1)
            .expect("static place must exist")
    }
}

impl fmt::Display for Place {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for Place {
    type Err = UnknownSpecimen;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::named(s)
    }
}

impl Rendering {
    /// Looks up a recorded rendering by name.
    ///
    /// # Errors
    ///
    /// Returns [`UnknownSpecimen`] if `name` is not a known rendering.
    pub fn named(name: &str) -> Result<Self, UnknownSpecimen> {
        Err(UnknownSpecimen::for_kind(
            name,
            "rendering",
            std::iter::empty(),
        ))
    }

    /// Every shipped rendering.
    pub fn all() -> impl Iterator<Item = Self> {
        std::iter::empty()
    }

    /// The name of this rendering.
    #[must_use]
    pub const fn name(self) -> &'static str {
        self.0
    }

    /// The build of this rendering.
    #[must_use]
    pub fn build(self) -> Build {
        unimplemented!("renderings arrive in step 8")
    }

    /// The machine of this rendering.
    #[must_use]
    pub fn machine(self) -> Machine {
        unimplemented!("renderings arrive in step 8")
    }

    /// The provenance record for this rendering.
    #[must_use]
    pub fn provenance(self) -> &'static Provenance {
        unimplemented!("renderings arrive in step 8")
    }
}

impl fmt::Display for Rendering {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for Rendering {
    type Err = UnknownSpecimen;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::named(s)
    }
}

/// The settings a specimen of each kind records, in file order.
#[allow(dead_code, reason = "crate-internal introspection helpers")]
pub(crate) fn settings_of(specimen: Specimen) -> &'static [&'static str] {
    match specimen {
        Specimen::Build(_) => &[
            "user_agent",
            "platform",
            "vendor",
            "product_sub",
            "app_version",
            "connection",
            "client_hints",
        ],
        Specimen::Machine(_) => &[
            "platform",
            "hardware_concurrency",
            "device_memory",
            "max_touch_points",
            "screen",
            "avail_screen",
            "pixel_ratio",
            "color_depth",
            "webgl",
            "media_devices",
            "fonts",
            "battery",
            "speech_voices",
            "bluetooth",
        ],
        Specimen::Place(_) => &[
            "locale",
            "languages",
            "accept_languages",
            "timezone",
            "geolocation",
        ],
        Specimen::Rendering(_) => &[
            "user_agent",
            "platform",
            "hardware_concurrency",
            "device_memory",
            "max_touch_points",
            "vendor",
            "product_sub",
            "app_version",
            "screen",
            "avail_screen",
            "pixel_ratio",
            "color_depth",
            "webgl",
            "webgl_output",
            "media_devices",
            "fonts",
            "canvas",
            "audio",
            "battery",
            "connection",
            "speech_voices",
            "bluetooth",
            "client_hints",
        ],
    }
}

/// The kinds a setting can mimic; empty for proxy, webrtc, oscpu and build_id.
#[allow(dead_code, reason = "crate-internal introspection helpers")]
pub(crate) fn takes(setting: &str) -> &'static [&'static str] {
    match setting {
        "user_agent" | "app_version" | "vendor" | "product_sub" | "client_hints" | "connection" => {
            &["build", "rendering"]
        }
        "platform" => &["build", "machine", "rendering"],
        "hardware_concurrency"
        | "device_memory"
        | "max_touch_points"
        | "screen"
        | "avail_screen"
        | "pixel_ratio"
        | "color_depth"
        | "webgl"
        | "fonts"
        | "media_devices"
        | "speech_voices"
        | "battery"
        | "bluetooth" => &["machine", "rendering"],
        "locale" | "languages" | "accept_languages" | "timezone" | "geolocation" => &["place"],
        "webgl_output" | "canvas" | "audio" => &["rendering"],
        _ => &[],
    }
}

/// The record-mode script the rendering recorder installs (test-util only).
#[cfg(feature = "test-util")]
pub fn recording_script() -> String {
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_specimen_name_is_unique_across_the_four_kinds() {
        let mut seen = std::collections::HashSet::new();
        let mut count = 0;
        for s in Specimen::all() {
            count += 1;
            assert!(
                seen.insert(s.name()),
                "duplicate specimen name: {}",
                s.name()
            );
        }
        assert_eq!(seen.len(), count);
    }

    #[test]
    fn every_name_is_lower_case_with_hyphens() {
        for s in Specimen::all() {
            let name = s.name();
            assert!(!name.is_empty(), "name cannot be empty");
            assert!(
                name.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "name '{name}' contains illegal characters"
            );
            assert!(
                !name.starts_with('-') && !name.ends_with('-'),
                "name '{name}' cannot start or end with a hyphen"
            );
            assert!(
                !name.contains("--"),
                "name '{name}' cannot contain consecutive hyphens"
            );
        }
    }

    #[test]
    fn a_name_says_its_kind() {
        assert_eq!(Specimen::named("macbook-13-m1").unwrap().kind(), "machine");
        assert_eq!(Specimen::named("chrome-157-macos").unwrap().kind(), "build");
        assert_eq!(Specimen::named("de-de-berlin").unwrap().kind(), "place");
    }

    #[test]
    fn an_unknown_machine_is_refused_with_every_machine_name() {
        let err = Machine::named("nonexistent-machine").unwrap_err();
        assert_eq!(err.name(), "nonexistent-machine");
        assert_eq!(err.kind(), Some("machine"));
        let msg = err.to_string();
        assert!(
            msg.starts_with(
                "there is no machine called \"nonexistent-machine\"; the machines are: "
            ),
            "unexpected message: {msg}"
        );
        assert!(msg.contains("macbook-13-m1"), "missing macbook: {msg}");
        assert!(msg.contains("windows-gtx-1650"), "missing windows: {msg}");
    }

    #[test]
    fn an_unknown_specimen_is_refused_with_the_names_of_each_kind() {
        let err = Specimen::named("nonexistent-specimen").unwrap_err();
        assert_eq!(err.name(), "nonexistent-specimen");
        assert_eq!(err.kind(), None);
        let err_str = err.to_string();
        let lines: Vec<&str> = err_str.lines().collect();
        assert_eq!(lines.len(), 4, "expected 4 lines, got: {lines:?}");
        assert_eq!(
            lines[0],
            "there is no build, machine, place or rendering called \"nonexistent-specimen\";"
        );
        assert!(lines[1].starts_with("builds: "), "line 1 was: {}", lines[1]);
        assert!(
            lines[2].starts_with("machines: "),
            "line 2 was: {}",
            lines[2]
        );
        assert!(lines[3].starts_with("places: "), "line 3 was: {}", lines[3]);
    }

    #[test]
    fn the_builds_are_every_known_major_on_four_systems_edge_and_the_safari_releases() {
        let builds: Vec<_> = Build::all().collect();
        // 4 majors * 4 systems Chrome + 4 Edge + 4 Safari = 24
        assert_eq!(builds.len(), 24);
        assert!(Build::named("chrome-154-windows").is_ok());
        assert!(Build::named("chrome-157-macos").is_ok());
        assert!(Build::named("edge-156-windows").is_ok());
        assert!(Build::named("safari-18-1-ios").is_ok());
        assert!(Build::named("safari-26-0-ios").is_ok());
    }

    #[test]
    fn every_mac_phone_and_iphone_row_is_a_named_machine() {
        for mac in MACHINES
            .iter()
            .filter(|m| matches!(m.row, MachineRow::Mac(_)))
        {
            assert!(Machine::named(&mac.name).is_ok());
        }
        for phone in MACHINES
            .iter()
            .filter(|m| matches!(m.row, MachineRow::Android(_)))
        {
            assert!(Machine::named(&phone.name).is_ok());
        }
        for iphone in MACHINES
            .iter()
            .filter(|m| matches!(m.row, MachineRow::Iphone(_)))
        {
            assert!(Machine::named(&iphone.name).is_ok());
        }
        assert!(Machine::named("macbook-13-m1").is_ok());
        assert!(Machine::named("pixel-8").is_ok());
        assert!(Machine::named("iphone-15").is_ok());
    }

    #[test]
    fn every_pc_adapter_is_a_named_machine_of_its_system() {
        let pc_count = MACHINES
            .iter()
            .filter(|m| matches!(m.row, MachineRow::Pc(_, _)))
            .count();
        // 16 Windows + 10 Linux = 26
        assert_eq!(pc_count, 26);
        assert!(Machine::named("windows-uhd-620").is_ok());
        assert!(Machine::named("linux-rtx-4070").is_ok());
    }

    #[test]
    fn every_place_is_a_row_of_the_locale_table() {
        assert_eq!(
            Place::all().count(),
            crate::evasion::randomize::LOCALES.len()
        );
        assert!(Place::named("en-us-new-york").is_ok());
        assert!(Place::named("de-de-berlin").is_ok());
        assert!(Place::named("pt-br-sao-paulo").is_ok());
    }

    #[test]
    fn takes_and_settings_of_agree() {
        for specimen in [
            Specimen::Build(Build::named("chrome-157-macos").unwrap()),
            Specimen::Machine(Machine::named("macbook-13-m1").unwrap()),
            Specimen::Place(Place::named("de-de-berlin").unwrap()),
        ] {
            let k = specimen.kind();
            for s in settings_of(specimen) {
                assert!(
                    takes(s).contains(&k),
                    "setting '{s}' in settings_of({k}) but takes('{s}') does not contain '{k}'"
                );
            }
        }
    }

    #[test]
    fn dynamic_version_parsing_for_arbitrary_majors() {
        let majors = [120, 128, 130, 140, 160];
        let systems = [
            ("windows", OsType::Windows),
            ("macos", OsType::Macos),
            ("linux", OsType::Linux),
            ("android", OsType::Android),
        ];
        let brands = ["chrome", "edge"];

        for brand in brands {
            for &major in &majors {
                for &(os_str, expected_os) in &systems {
                    let name = format!("{brand}-{major}-{os_str}");

                    // Build::named
                    let build = Build::named(&name)
                        .unwrap_or_else(|e| panic!("failed to parse build {name}: {e}"));
                    assert_eq!(build.name(), name);
                    assert_eq!(build.browser(), BrowserType::Chromium);
                    assert_eq!(build.os(), expected_os);
                    assert_eq!(build.major(), major);

                    // Specimen::named
                    let specimen = Specimen::named(&name)
                        .unwrap_or_else(|e| panic!("failed to parse specimen {name}: {e}"));
                    assert_eq!(specimen, Specimen::Build(build));
                    assert_eq!(specimen.kind(), "build");
                    assert_eq!(specimen.name(), name);

                    // Round-trip serde
                    let json = serde_json::to_string(&specimen).expect("serialize specimen");
                    assert_eq!(json, format!("\"{name}\""));
                    let parsed: Specimen =
                        serde_json::from_str(&json).expect("deserialize specimen");
                    assert_eq!(parsed, specimen);

                    // Idempotent lookups (tests dynamic cache)
                    let build_again = Build::named(&name).expect("second lookup must succeed");
                    assert_eq!(build, build_again);
                }
            }
        }

        // Invalid patterns fail cleanly
        assert!(Build::named("chrome-invalid-windows").is_err());
        assert!(Build::named("firefox-130-windows").is_err());
        assert!(Build::named("chrome-130-solaris").is_err());
        assert!(Build::named("chrome-130").is_err());
        assert!(Build::named("edge--windows").is_err());
        assert!(Specimen::named("chrome-xyz-macos").is_err());
    }
}
