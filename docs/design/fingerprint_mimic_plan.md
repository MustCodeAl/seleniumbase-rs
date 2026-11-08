---
title: "Plan: Mimic, the Chromium lane"
sidebarTitle: "Mimic plan"
---

Status: implementation plan (2026-11-07) for `lane/mimic`, written from the code at `c6f3eb73`
(`lane/mimic` = `main` + [Mimic](/design/fingerprint_mimic) + [Core split](/design/core_split))
and from `lane/fp-consistency` at `9575db95`. It builds the **Chromium lane** of the Mimic note.
The owner approved the note's eleven recommendations; the choices the note left open are decided
below ([Decisions](#decisions)), each with its reason. The implementer follows the
[steps](#steps) in order, one commit each, test first.

The owner's purpose (2026-11-06): "show a chrome canvas even when using firefox or any other
browser", "same with the other fields", and "focused on chrome for now". So this lane gives every
setting the fifth state and replays recorded Chrome canvas, WebGL and audio outputs **in
Chromium**. Firefox (the BiDi preload path), cross-engine APIs and CanvasKit wait for later lanes
([Left out](#left-out-and-why)).

## Rules the lane keeps

- Code stays in `src/evasion/`, `src/mcp/fingerprint.rs`, `src/cli/fingerprint_cmd.rs` and
  `crates/cli/src/fingerprint.rs` (the core split's two rules,
  [Where Mimic is implemented](/design/core_split#where-mimic-is-implemented)). No new type of
  `evasion` is used from `browser/`, `sb_cdp/` or `api/`. Tests go to `tests/chrome/`,
  `tests/support/` and `tests/fixtures/`; shipped data to `src/evasion/mimic/`.
- Names follow the project: `Spoof`, `Specimen`, `Build`, `Machine`, `Place`, `Rendering`.
  "mimic" means the Mimic state and nothing else.
- Test first: write the test, see it fail (or not compile) for the stated reason, then the change,
  then green. One commit per step, conventional subject (`feat(evasion)!:` for a breaking change).
  No push, no stash, no attribution lines.
- Chrome tests run headless only (`SB_HEADLESS=true`), one binary at a time
  (`--test-threads=1`), against the tests' own loopback servers and pinned local copies of the
  detector scripts. No hosted detector, captcha or bot-protection site, ever.
- Every step updates its docs and `CHANGELOG.md` `[Unreleased]` in the same commit, then
  `rtk python3 -I tools/docs7_changelog.py`.

## What the plan builds on

Read with TokenSave at the commits above.

| Fact | Where |
|---|---|
| `Spoof<T>` has four variants, not `#[non_exhaustive]`; serde writes a word or the value, reads `Word` or `Value` through an untagged `Wire` | `src/evasion/spoof.rs:37-49`, `:184-191`, `:221-254` |
| The only exhaustive `match` on `Spoof` is inside `spoof.rs` (`as_ref`, `map`, `word`); a literal search for `Spoof::Off =>` and `Spoof::Random =>` finds nothing else | `spoof.rs:109-136` |
| `Fingerprint(Fields)` is `#[serde(transparent)]` over `Fields`; `Fields` is `deny_unknown_fields`, one `Spoof` per setting, Natural left out | `src/evasion/fingerprint.rs:512-641`, `:734-746` |
| The per-setting lists that every new setting joins: `Fields` and its `Default`, `accessors!`, `setters!`, `without_random`, `refuse_useless_off`, `has_random`, `draw_random`, `is_masked`, `compare::SETTINGS` (29 names), `schema::OFF_SETTINGS`/`NO_RANDOM`, the provider table test | `fingerprint.rs:643-687`, `:813-881`, `:1557-1594`, `:1629-1697`, `:1913-1977`, `:1994-2172`; `fingerprint/compare.rs:13-43`; `fingerprint/schema.rs:19-31`; `providers/mod.rs` tests (`setters()`) |
| `build()`: refuse useless Off, proxy host, OS from the user agent, seed (entropy when Random, else from the user agent), `draw_random`, languages and platform, then the policy last | `fingerprint.rs:1856-1909` |
| One seeded `randomize::draw(os, seed, forced_major, place)` decides the machine and the place; `forced_major` and `place` replace a pick but the pick still happens, so the rest of the seed is unchanged | `src/evasion/randomize.rs:1095-1119`, the OS profiles `:1371-1560` |
| Machine rows: `WINDOWS_ADAPTERS` (16), `LINUX_ADAPTERS` (10), cores, memory and screens per `Tier`, `MAC_MODELS` (13), `ANDROID_DEVICES` (7), `IPHONE_SCREENS` (6); none has a name | `randomize.rs:150-210`, `:284-365`, `:386-400`, `:465-483` |
| Builds: `CHROMIUM_MAJORS = [154, 155, 156]` (what Random draws), `KNOWN_FULL_VERSIONS` (3 rows), `MOBILE_SAFARI_VERSIONS` (4), `edge_user_agent`, `edge_hints`, `chromium_hints` | `randomize.rs:31-111`, `:1563-1679` |
| Places: `LOCALE_TABLE`/`LOCALES` (`Locale` is public and read by the stealth repository) | `randomize.rs:489-535`, `:647-1028` |
| Canvas and audio are policy only (`CanvasNoise`, `AudioNoise`), the presets turn both on (`StealthPolicy::balanced`) | `src/evasion/policy.rs:27-86`, `:177-180` |
| The noise providers hook `toDataURL`, `toBlob`, `getImageData`, `getChannelData`, `copyFromChannel`, `getFloatFrequencyData`; snippets are IIFEs built by `render()` with `@MARKER@`s and the `sb` helpers | `src/evasion/providers/builtin.rs:26-96`, `:644-817` |
| The provider audit (`audit_each!`) runs every session provider against one `everything()` fingerprint and refuses `window.__sb`, unfilled markers and browser-owned names | `providers/mod.rs` tests (`audit`, `AUDITED`) |
| Readers outside `evasion` read settings through `.value()` and `.as_str()`: `sb_cdp/launch/layers.rs:172-173` (user agent, locale), `sb_cdp/launch.rs:568` (proxy), and `evasion/evasions.rs` (launch flags, overrides) | literal searches for `().value()` and `().as_str()` |
| `capture.js` draws one canvas (240x48, `toDataURL`, FNV hash) and renders one `OfflineAudioContext` (1, 5000, 44100); the hashes land in `CapturedFingerprint::unmapped` | `src/evasion/js/capture.js` (consistency lane `:287-332`), `src/evasion/capture.rs` |
| `sbrs fingerprint export` takes `--preset` or `--random`; `presets` prints a table; the arguments are `usage` derives in the CLI crate | `crates/cli/src/fingerprint.rs:37-128`, `:196-222`; `src/cli/fingerprint_cmd.rs:19-106`, `:158-167` |
| `LaunchOptionsBuilder::host_resolver_rules` exists (to keep a test browser off the network) | `src/sb_cdp/launch/builder.rs:515` |
| This Mac: `MacBookPro17,1`, Apple M1, 8 cores; installed Google Chrome `157.0.8103.39` (2026-11-07) | `sysctl hw.model`, the app's `Info.plist` |
| `lane/fp-consistency` adds `fingerprint/consistency.rs` (`Contradiction`, `Severity`), the capture readings, the `webgpu` provider, `compare_fingerprint`/`explain_fingerprint`, and `tests/chrome/fingerprint_consistency_chrome.rs` (four personas, `CLOSED`/`OPEN` rule lists) | `git diff --stat 2327ffb8 lane/fp-consistency` |

## Decisions

1. **A built fingerprint holds the specimen's values and keeps its name beside them.** `build()`
   replaces every `Mimic` with the `Value` its specimen records and stores the pair
   `(setting, specimen)` in a private list of `Fields`; `Fingerprint::mimic_of` and `mimics()` read
   it back, and the file writes the name (`"screen": {"mimic": "macbook-13-m1"}`), never the
   values. Reason: the readers outside `evasion` (`sb_cdp/launch/layers.rs`, `sb_cdp/launch.rs`)
   and every provider read `.value()`; if `fp.screen()` answered `Mimic`, they would apply nothing
   and the claim would be half applied, and fixing them would touch `sb_cdp/`, which the core split
   forbids. This is the note's "the providers read a setting through one accessor that answers
   `Value` and `Mimic` alike", with the accessor being the existing one. `fp.screen().is_value()`
   is therefore true for a mimicked screen; `fp.mimic_of("screen")` says it is a mimic.
2. **`Spoof::mimic(specimen)` constructs the state; there is no `From<Specimen>` for `Spoof<T>`.**
   `impl<T> From<Specimen> for Spoof<T>` overlaps with `impl<T> From<T> for Spoof<T>` at
   `T = Specimen`; one constructor replaces the per-type impl matrix the note sketched.
3. **No associated constant per row.** About 130 names are generated from the rows; constants
   would write each name twice. Lookups are `Build::named("chrome-156-windows")` and so on, and
   `Specimen::named(name)` for any kind (names are unique across kinds, so a name says its kind).
4. **Names are made by rules from the rows**, lower case with hyphens (the stealth crate's
   `chrome-155-macos` style): builds `{brand}-{version}-{system}`, PC machines `{system}-{gpu slug}`,
   Macs, phones and iPhones by an explicit model name, places `{tag}-{city}`, renderings
   `{build}-{machine}`. The tables are in [The catalogue](#the-catalogue).
5. **Builds come from `KNOWN_FULL_VERSIONS`, not from `CHROMIUM_MAJORS`.** Mimic needs a real full
   version; Random keeps drawing 154 to 156. The lane adds the row 157 measured on this Mac
   (`157.0.8103.39`, the installed Chrome) so that the shipped rendering has a build. Moving the
   drawn window to 157 is a separate change ([Left out](#left-out-and-why)).
6. **A named PC machine is its GPU plus the first cores, memory and screen of its tier**
   (`spec.cores[0]`, `spec.memory_gb[0]`, `spec.windows_screens[0]` or `linux_screens[0]`).
   Random keeps drawing the parts of a PC independently, as real PCs vary. So "a drawn machine is a
   named row" holds whole for Macs, Android phones and iPhones and for the GPU of a PC; the
   catalogue test asserts exactly that.
7. **Random and Mimic read one pinned draw.** `randomize::draw` takes `Pins` (major, place, build,
   machine). A pin replaces a pick and the pick still happens, as `forced_major` and `place` do
   today, so a seed without pins draws what it drew before. `draw_random` resolves a `Mimic` the
   way it resolves a `Random`, from the same draw; there is one code path for both.
8. **The claimed-browser checks run in `build()`, the cross-engine one included.** Every launch
   path that applies a fingerprint drives Chromium in this lane (the note's table: "a fingerprint
   reaches only Chromium today"), so a non-Chromium build is a cross-engine claim wherever it is
   used, and `build()` is inside `src/evasion`. When the Firefox lane gives a fingerprint a second
   engine, the cross-engine check moves to the launch, where the running engine is known.
9. **A mimic against a mimic, or against a claim given with `browser_type`, `os_type` or
   `core_version`, is refused by `build()`; a `Value` against a mimic wins and `validate()`
   reports it as an error.** The note asks both "refused, not applied in part" (one claimed
   browser) and "a setting given afterwards still wins, and is checked"; this split keeps both.
10. **A new `Dimension::Rendering`** masks the three recorded settings; `Dimension::Canvas` and
    `Dimension::Audio` stay the noise. A `Value` or `Mimic` on `canvas` or `audio` with that noise
    on is refused (`NoiseOverRecording`): noise breaks the exact output a recording exists to show.
11. **The value types are recorded outputs keyed by drawing**: `CanvasOutputs`, `WebGlOutputs`,
    `AudioOutputs`. A drawing's key is an incremental hash of its call log (methods, arguments,
    state setters, canvas size), computed by one script that has a record mode and a replay mode.
    The replay script carries no record code, so the provider audit stays as strict as for the
    others.
12. **Shipped recordings live in `src/evasion/mimic/renderings/*.json`**, embedded with
    `include_str!` and parsed once; detector copies live in `tests/fixtures/detectors/` and are
    never shipped. One rendering ships: `chrome-157-macos-macbook-13-m1`, recorded headless on this
    Mac, with its `Provenance` saying so. Windows, Linux and phone renderings need captures from
    those machines.
13. **CreepJS is vendored only if its built bundle runs offline.** FingerprintJS v3 (MIT) is
    vendored from its npm build. CreepJS (MIT) ships as an application bundle that calls its own
    API; the recorder runs it with every host but the loopback unresolvable. If it does not run
    that way, or its bundle is over 2 MB, it is left out and the note's "As implemented" says
    which probes are covered.
14. **The crate root re-exports `Specimen` only.** `Build`, `Machine`, `Place` and `Rendering` stay
    at `seleniumbase_rs::evasion::mimic`: at the root they would read as generic words.
15. **`sbrs fingerprint export --mimic NAME` is repeatable** (`Vec<String>`, like the global
    `--plugin`), any kind, applied in order; `presets` lists the names by kind. No `SB_MIMIC`, no
    global `--mimic`, no `--mimic` on the runner (the note's question 11).
16. **`Fingerprint::mimic(specimens)` uses the presets' policy** (`StealthPolicy::balanced()`), with
    canvas and audio noise turned off when a rendering is named.
17. **A fingerprint whose settings are only mimics gets a stable seed** derived from the specimen
    names, so the same mimic persona gives the same drawn extras and noise on every build.
18. **`Spoof` stays exhaustive.** Adding `#[non_exhaustive]` would force a wildcard arm on every
    user for a sixth state nobody plans; the variant is listed under Breaking.
19. **No `mimic:NAME` text form.** No flag or variable carries a setting as text today, so it
    would have no reader; it comes with the first one that does.
20. **The browser type, system and major follow a mimicked build** unless they were given with the
    builder's `browser_type`, `os_type` or `core_version` (or written in a file) and differ, which
    is refused. A builder flag `browser_type_given` tracks the explicit call; files keep
    `from_fields` as it is.
21. **Merge `lane/fp-consistency` first.** Both lanes change `fingerprint.rs`, `compare.rs`,
    `capture.rs`, `capture.js`, the providers and `src/mcp/fingerprint.rs`; merging before the
    first Mimic commit keeps every later step a plain change on one base.
22. **`src/mcp/dev/diagnose.rs` is not changed.** Its summary reads `.value()`, which now holds the
    mimicked value (decision 1). `compare_fingerprint` in `src/mcp/fingerprint.rs` adds the mimics.

## The API

Written out; doc comments are abbreviated here and must be full sentences in the code, with a
doctest on each public item that has a natural example.

### `src/evasion/spoof.rs`

```rust
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Spoof<T> {
    Natural,
    Random,
    Value(T),
    /// What the named real build, machine, place or rendering shows. `build()` reads the value
    /// from the catalogue; the fingerprint keeps the name (`Fingerprint::mimic_of`).
    Mimic(Specimen),
    Off,
}

impl<T> Spoof<T> {
    pub const fn is_mimic(&self) -> bool;
    /// The specimen, if this is a `Mimic`.
    pub const fn specimen(&self) -> Option<Specimen>;
    /// The `Mimic` state of `specimen`: `Spoof::mimic(Machine::named("macbook-13-m1")?)`.
    pub fn mimic(specimen: impl Into<Specimen>) -> Self;
}
// as_ref / map: `Self::Mimic(s) => Spoof::Mimic(*s)`; word(): `Self::Mimic(_) => None`.
// Serialize: `Self::Mimic(s)` writes the map { "mimic": s.name() } (serialize_map, one entry).
// Deserialize: `Wire<T>` becomes `Word(Word) | Mimic(MimicName) | Value(T)`, in that order, with
//   #[derive(Deserialize)] #[serde(deny_unknown_fields)] struct MimicName { mimic: String }
// and `Some(Wire::Mimic(MimicName { mimic })) => Self::Mimic(Specimen::named(&mimic)
//   .map_err(serde::de::Error::custom)?)`, so an unknown name keeps its own message.
// The test-only JsonSchema adds { "type": "object", "properties": { "mimic": { "type": "string" } },
//   "required": ["mimic"], "additionalProperties": false } to the anyOf.
```

### `src/evasion/mimic.rs` (new, `pub mod mimic;` in `src/evasion/mod.rs`)

```rust
/// What a `Spoof::Mimic` names: one real browser build, machine, place or rendering.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Specimen { Build(Build), Machine(Machine), Place(Place), Rendering(Rendering) }

/// A browser release on one system family, such as `chrome-156-windows`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Build(&'static str);
/// A device model, such as `macbook-13-m1` or `windows-uhd-620`.
pub struct Machine(&'static str);            // same derives
/// A locale with its city, such as `de-de-berlin`.
pub struct Place(&'static str);              // same derives
/// One build on one machine, recorded: what the reference drawings return there.
pub struct Rendering(&'static str);          // same derives

impl Specimen {
    pub fn named(name: &str) -> Result<Self, UnknownSpecimen>;
    pub fn all() -> impl Iterator<Item = Self>;       // builds, machines, places, renderings
    pub const fn name(self) -> &'static str;
    pub const fn kind(self) -> &'static str;          // "build" | "machine" | "place" | "rendering"
}
impl From<Build> for Specimen {}   // and Machine, Place, Rendering
impl fmt::Display for Specimen {}  // the name; Display for each kind too
impl FromStr for Specimen { type Err = UnknownSpecimen; }   // and for each kind
impl Serialize for Specimen {}     // the name as a string
impl<'de> Deserialize<'de> for Specimen {}  // a name, through `named`

impl Build {
    pub fn named(name: &str) -> Result<Self, UnknownSpecimen>;
    pub fn all() -> impl Iterator<Item = Self>;
    pub const fn name(self) -> &'static str;
    pub fn browser(self) -> BrowserType;   // Chromium for chrome-* and edge-*, MobileSafari for safari-*
    pub fn os(self) -> OsType;
    pub fn major(self) -> u32;             // Chromium major, or Safari's (18, 26)
}
impl Machine {
    pub fn named(name: &str) -> Result<Self, UnknownSpecimen>;
    pub fn all() -> impl Iterator<Item = Self>;
    pub const fn name(self) -> &'static str;
    pub fn os(self) -> OsType;
    pub fn webgl(self) -> WebGl;           // the unmasked vendor and renderer it reports
}
impl Place {
    pub fn named(name: &str) -> Result<Self, UnknownSpecimen>;
    pub fn all() -> impl Iterator<Item = Self>;
    pub const fn name(self) -> &'static str;
    pub fn locale(self) -> &'static Locale;
}
impl Rendering {
    pub fn named(name: &str) -> Result<Self, UnknownSpecimen>;
    pub fn all() -> impl Iterator<Item = Self>;
    pub const fn name(self) -> &'static str;
    pub fn build(self) -> Build;
    pub fn machine(self) -> Machine;
    pub fn provenance(self) -> &'static Provenance;
}

/// Where a recorded rendering comes from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    pub recorded: String,   // "2026-11-07"
    pub tool: String,       // "tests/chrome/rendering_record_chrome.rs"
    pub browser: String,    // "Google Chrome 157.0.8103.39"
    pub headless: bool,
    pub host: String,       // "macOS 27.0.1, MacBookPro17,1"
}

/// A specimen name this version does not know.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct UnknownSpecimen { name: String, kind: Option<&'static str>, message: String }
impl UnknownSpecimen {
    pub fn name(&self) -> &str;
    pub fn kind(&self) -> Option<&'static str>;   // None when looked up by Specimen::named
}
impl From<UnknownSpecimen> for crate::error::SeleniumBaseError {}   // InvalidConfig

/// The settings a specimen of each kind records, in file order (see the table below).
pub(crate) fn settings_of(specimen: Specimen) -> &'static [&'static str];
/// The kinds a setting can mimic; empty for proxy, webrtc, oscpu and build_id.
pub(crate) fn takes(setting: &str) -> &'static [&'static str];

/// The record-mode script the rendering recorder installs (test-util only).
#[cfg(feature = "test-util")]
pub fn recording_script() -> String;
```

The kind newtypes hold a `&'static str` that is either a literal or borrowed from the catalogue
statics (`LazyLock<Vec<...>>` in `randomize.rs`, so the borrow is `'static`). The field is private,
so a value exists only through `named` and `all`. The messages of `UnknownSpecimen`:

- `Machine::named("x")`: `there is no machine called "x"; the machines are: macbook-13-m1, ...`
  (every machine name, comma-separated).
- `Specimen::named("x")` (a file): `there is no build, machine, place or rendering called "x";`
  followed by one line per kind, `builds: ...`, `machines: ...`, `places: ...`, `renderings: ...`.

### `src/evasion/settings.rs` (the recorded outputs), re-exported from `evasion::fingerprint`

```rust
/// What the recorded 2D drawings return, by drawing key.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanvasOutputs { pub drawings: BTreeMap<String, DrawnImage> }

/// One recorded drawing: its size, `toDataURL()` and the pixels `getImageData` returns.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DrawnImage {
    pub width: u32,
    pub height: u32,
    /// `toDataURL()`, a `data:image/png;base64,` URL. Empty for a WebGL drawing read with readPixels only.
    pub png: String,
    /// The whole canvas as RGBA bytes, base64 (WebGL: `readPixels` order, bottom row first).
    pub rgba: String,
}

/// What a WebGL context answers, by context type, and the recorded WebGL drawings.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WebGlOutputs {
    pub webgl: ContextOutputs,
    pub webgl2: ContextOutputs,
    pub drawings: BTreeMap<String, DrawnImage>,
}

/// The answers of one WebGL context type.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextOutputs {
    /// `getParameter`, by parameter number in decimal. 37445 and 37446 (the unmasked vendor and
    /// renderer) are never here: they are the `webgl` setting.
    pub parameters: BTreeMap<String, serde_json::Value>,
    pub extensions: Vec<String>,
    /// `getShaderPrecisionFormat`, by "VERTEX_SHADER.HIGH_FLOAT" and so on.
    pub shader_precisions: BTreeMap<String, ShaderPrecision>,
    pub context_attributes: BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShaderPrecision { pub range_min: i32, pub range_max: i32, pub precision: i32 }

/// What the recorded offline audio graphs render, by graph key.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioOutputs { pub renders: BTreeMap<String, AudioRender> }

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioRender {
    pub channels: u32,
    pub length: u32,
    pub sample_rate: u32,
    /// Each channel's samples as little-endian `f32`, base64.
    pub samples: Vec<String>,
}
```

Each derives `schemars::JsonSchema` under `cfg_attr(test, ...)` like the other setting types.
`WebGlOutputs` and `ContextOutputs` are not `Eq` (they hold JSON values); `Fields` is `PartialEq`
only, so that is enough.

### `src/evasion/fingerprint.rs`

```rust
// Fields gains, after `webgl`:
    #[serde(skip_serializing_if = "Spoof::is_natural")]
    webgl_output: Spoof<WebGlOutputs>,
    #[serde(skip_serializing_if = "Spoof::is_natural")]
    canvas: Spoof<CanvasOutputs>,
    #[serde(skip_serializing_if = "Spoof::is_natural")]
    audio: Spoof<AudioOutputs>,
// and, last:
    /// The settings that mimic a specimen, in file order, with the specimen. Their values above
    /// were read from it; the file writes the name instead (see `declared`).
    #[serde(skip)]
    mimics: Vec<(&'static str, Specimen)>,

impl Fields {
    /// These fields with every mimicked setting back to `Spoof::Mimic`, the list emptied: what a
    /// file says and what `to_builder` starts from.
    fn declared(&self) -> Fields;
    /// Sets `setting` to `Spoof::Mimic(specimen)`; false for a name that is not a setting.
    fn set_mimic(&mut self, setting: &str, specimen: Specimen) -> bool;
    /// The specimen `setting` mimics before it is built.
    fn specimen_of(&self, setting: &str) -> Option<Specimen>;
}
// `set_mimic` and `specimen_of` are two matches over the 32 setting names; write both with one
// `macro_rules!` over the list, as `accessors!` is written.

// Fingerprint: Serialize becomes a manual impl that serializes `self.0.declared()`
// (keep `#[serde(transparent)]` semantics: the same keys). JsonSchema stays derived.
impl Fingerprint {
    /// The specimen `setting` mimics, if it does.
    pub fn mimic_of(&self, setting: &str) -> Option<Specimen>;
    /// Every mimicked setting with its specimen, in file order.
    pub fn mimics(&self) -> impl Iterator<Item = (&'static str, Specimen)> + '_;
    /// A fingerprint that mimics every setting the specimens record (see decision 16).
    pub fn mimic<I>(specimens: I) -> Result<Self, FingerprintError>
    where I: IntoIterator, I::Item: Into<Specimen>;
    // accessors!: webgl_output, canvas, audio
    // to_builder: FingerprintBuilder::from_fields(self.0.declared())
}

impl FingerprintBuilder {
    /// Sets every setting `specimen` records to mimic it. A setting given afterwards replaces it,
    /// and `validate` reports a value that contradicts the specimen.
    pub fn mimic(mut self, specimen: impl Into<Specimen>) -> Self;
    /// Shortcut for `StealthPolicy::cross_engine`, applied like the other policy shortcuts.
    pub fn cross_engine(self, on: bool) -> Self;
    // setters!: webgl_output: WebGlOutputs; canvas: CanvasOutputs; audio: AudioOutputs;
}
// FingerprintBuilder gains `browser_type_given: bool`, set only by `browser_type()`.

#[non_exhaustive]
pub enum FingerprintError {
    // ... the three existing variants, then:
    /// A setting mimics a kind of specimen it cannot take, or has no mimic at all.
    #[error("{setting} cannot mimic {specimen}: {takes}")]
    MimicNotAvailable { setting: &'static str, specimen: String, takes: &'static str },
    /// Two claims name different browsers, systems, machines or places.
    #[error("{first} and {second} claim different {what}")]
    MixedClaims { first: String, second: String, what: &'static str },
    /// A mimic claims a browser engine other than Chromium, and the policy does not allow it.
    #[error("{setting} mimics {specimen}, a {engine} build, and the browser every fingerprint runs in is Chromium; StealthPolicy::cross_engine(true) claims it anyway")]
    CrossEngine { setting: &'static str, specimen: String, engine: &'static str },
    /// Recorded outputs are given while the policy noises them.
    #[error("{setting} replays recorded outputs, and the policy's {noise} would change them; set it to Natural")]
    NoiseOverRecording { setting: &'static str, noise: &'static str },
}
// `takes` texts: "it takes a build or a rendering", "it takes a machine or a rendering",
// "it takes a build, a machine or a rendering", "it takes a place", "it takes a rendering",
// "it has no mimic" (proxy, webrtc), "it has no mimic until Firefox builds exist" (oscpu, build_id).
// `first` / `second` read "user_agent (chrome-156-windows)", "os_type (Macos)".

#[non_exhaustive]
pub enum Dimension {
    // ... then:
    /// The recorded outputs of the canvas, WebGL and audio drawings.
    Rendering,
}
// is_masked: Dimension::Rendering => f.canvas.is_set() || f.webgl_output.is_set() || f.audio.is_set()
```

### `src/evasion/policy.rs`

```rust
pub struct StealthPolicy {
    // ... then:
    /// Allow a mimic to claim another engine than the one that runs it (a Safari build in
    /// Chrome). Off by default: such a claim cannot hide the running engine, and `validate`
    /// lists what it cannot hide.
    pub cross_engine: bool,
}
impl StealthPolicy { pub fn cross_engine(mut self, on: bool) -> Self; }
// Default: false; balanced(): false.
```

### `src/evasion/randomize.rs`

```rust
// Row names (decision 4):
struct Adapter { vendor, name, device_id, tier, slug: &'static str }      // + `slug`
struct MacModel { name: &'static str, chip, cores, screen }               // + `name`
struct AndroidDevice { name: &'static str, model, android, screen, gpu }  // + `name`
struct IphoneModel { name: &'static str, screen: Screen }                 // IPHONE_SCREENS becomes [IphoneModel; 6]

pub const KNOWN_FULL_VERSIONS: [KnownFullVersion; 4];   // + 157, MeasuredOnMac, "157.0.8103.39"

/// The brand of a named build.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Brand { Chrome, Edge, Safari }

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
    Pc(OsType, &'static Adapter),          // Windows or Linux
    Mac(&'static MacModel),
    Android(&'static AndroidDevice),
    Iphone(&'static IphoneModel),
}

#[derive(Debug)]
pub(super) struct NamedMachine { pub(super) name: String, pub(super) row: MachineRow }

pub(super) static BUILDS: LazyLock<Vec<NamedBuild>>;
pub(super) static MACHINES: LazyLock<Vec<NamedMachine>>;
pub(super) static PLACES: LazyLock<Vec<(String, &'static Locale)>>;

/// What a draw must take as given instead of picking it. `Default` pins nothing.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Pins<'a> {
    pub(super) major: Option<u32>,
    pub(super) place: Option<&'a Locale>,
    pub(super) build: Option<&'a NamedBuild>,
    pub(super) machine: Option<MachineRow>,
}

pub(super) fn draw(os: OsType, seed: u64, pins: &Pins<'_>) -> Draw;   // replaces (os, seed, forced_major, place)
```

Inside each OS profile the picks keep their order and count; a pin only replaces the picked value:

- `windows`, `linux`: `adapter` is the pinned PC's adapter; `cores`, `memory_gb`, `screen` are
  `spec.cores[0]`, `spec.memory_gb[0]`, `spec.{windows,linux}_screens[0]` when a PC is pinned. An
  `Edge` build pin makes `user_agent` `edge_user_agent("Windows NT 10.0; Win64; x64", major)`;
  the existing `Edg/` branch of `draw_random` then gives the Edge hints.
- `macos`: `model` is the pinned Mac.
- `android`: `device` is the pinned phone.
- `ios`: `(version, os_version)` is the pinned Safari build's; `cores` is 6 and `screen` the
  pinned iPhone's.

## The catalogue

| Kind | Rows | Name rule | Examples |
|---|---|---|---|
| Build | every major of `KNOWN_FULL_VERSIONS` (154 to 157) on windows, macos, linux, android for Chrome; on windows for Edge; every row of `MOBILE_SAFARI_VERSIONS` for Safari | `chrome-{major}-{system}`, `edge-{major}-windows`, `safari-{version with dots as hyphens}-ios` | `chrome-157-macos`, `edge-156-windows`, `safari-26-0-ios` |
| Machine, PC | every row of `WINDOWS_ADAPTERS` and `LINUX_ADAPTERS` | `windows-{slug}`, `linux-{slug}` | `windows-uhd-620`, `linux-rtx-4070` |
| Machine, Mac | `MAC_MODELS` | the `name` column | `macbook-13-m1` |
| Machine, phone | `ANDROID_DEVICES`, `IPHONE_SCREENS` | the `name` column | `pixel-8`, `iphone-15` |
| Place | `LOCALES` | `{tag lower case}-{last segment of the time zone, lower case, '_' as '-'}` | `en-us-new-york`, `de-de-berlin`, `pt-br-sao-paulo` |
| Rendering | the files in `src/evasion/mimic/renderings/` | `{build}-{machine}` | `chrome-157-macos-macbook-13-m1` |

The names to write, so that none is invented:

- **Adapter slugs, Windows** (table order): `gtx-1650`, `gtx-1660-super`, `rtx-2060`,
  `rtx-3050`, `rtx-3060`, `rtx-3070`, `rtx-4060`, `rtx-4070`, `uhd-620`, `uhd-630`, `iris-xe`,
  `uhd-770`, `radeon-graphics`, `rx-580`, `rx-6600`, `rx-6700-xt`.
- **Adapter slugs, Linux**: `uhd-630`, `uhd-620`, `iris-xe`, `uhd-770`, `rx-6600`, `rx-580`,
  `rx-7800-xt`, `gtx-1660-super`, `rtx-3060`, `rtx-4070`.
- **Macs** (table order): `macbook-13-m1` (an Air or Pro 13-inch: this Mac, `MacBookPro17,1`),
  `macbook-pro-14-m1-pro`, `macbook-pro-16-m1-max`, `macbook-air-13-m2`, `mac-mini-m2-1080p`,
  `macbook-pro-14-m2-pro`, `macbook-pro-16-m2-max`, `macbook-air-13-m3`, `imac-24-m3`,
  `macbook-pro-14-m3-pro`, `macbook-pro-16-m3-max`, `macbook-pro-14-m4`, `macbook-pro-16-m4-pro`.
- **Android**: `pixel-8`, `pixel-7`, `pixel-7-pro`, `galaxy-s23` (SM-S911B), `galaxy-s23-ultra`
  (SM-S918B), `oneplus-11` (CPH2449), `galaxy-s21` (SM-G991B).
- **iPhones** (one model with that screen each): `iphone-se-3` (375x667 @2), `iphone-13-mini`
  (375x812 @3), `iphone-14` (390x844 @3), `iphone-15` (393x852 @3), `iphone-16-pro`
  (402x874 @3), `iphone-15-pro-max` (430x932 @3).

## Which setting takes which specimen

`takes(setting)` and `settings_of(kind)` are these two lists; a test keeps them in step.

| Settings | Mimic kinds | Where the value comes from |
|---|---|---|
| `user_agent`, `app_version`, `vendor`, `product_sub`, `client_hints`, `connection` | Build, Rendering | the pinned draw's profile; `app_version` is its user agent without `Mozilla/`, even when `user_agent` is not mimicked; `connection` follows the build (Off for Safari) |
| `platform` | Build, Machine, Rendering | `os.platform()` of the pinned system |
| `hardware_concurrency`, `device_memory`, `max_touch_points`, `screen`, `avail_screen`, `pixel_ratio`, `color_depth`, `webgl`, `fonts`, `media_devices`, `speech_voices`, `battery`, `bluetooth` | Machine, Rendering | the pinned draw (profile and extras); `device_memory` of an iPhone is Natural, as Random gives today |
| `locale`, `languages`, `accept_languages`, `timezone`, `geolocation` | Place | the pinned place; the position is the seeded spot in its city |
| `webgl_output`, `canvas`, `audio` | Rendering | the rendering file |
| `proxy`, `webrtc` | none | ours, or the network's |
| `oscpu`, `build_id` | none in this lane | Firefox builds come with the Firefox lane |

`settings_of`: Build = the first row plus `platform`; Machine = the second and third rows;
Place = the fourth; Rendering = Build, Machine and `webgl_output`, `canvas`, `audio`. A
rendering does not set a place (the note's question 8).

## The checks `build()` makes

In this order, after the existing Off and proxy checks and after the policy is folded (move the
fold before the checks; it does not read the settings):

1. Every `Mimic` is of a kind its setting takes (`MimicNotAvailable`).
2. At most one build, one machine and one place among the mimics, a rendering counting as its
   build and its machine (`MixedClaims`, `what` = "builds", "machines", "places").
3. The build's system equals the machine's (`MixedClaims`, "systems").
4. A given `browser_type` (only when `browser_type_given`), `os_type` or `core_version` that
   differs from the build or the machine (`MixedClaims`). Otherwise the build sets
   `browser_type`, `os_type` and (for Chromium) `core_version`, and a machine sets `os_type`.
5. A build whose `browser()` is not `Chromium` while `policy.cross_engine` is false
   (`CrossEngine`, `engine` = "WebKit").
6. `canvas` set (Value or Mimic) with `policy.canvas_noise.adds_noise()`, or `audio` with
   `policy.audio_noise.adds_noise()` (`NoiseOverRecording`, `noise` = "canvas_noise" or
   "audio_noise").
7. `Off` on `webgl_output`, `canvas` or `audio` (`OffNotAvailable`; add them to
   `refuse_useless_off`).

Then the mimics are collected (`specimen_of` over `SETTINGS`), the pins built from them, and
`draw_random` runs when a setting is `Random` **or** `Mimic`: every `if f.x.is_random()` becomes
`if f.x.is_random() || f.x.is_mimic()`. The render settings are filled last: `Mimic(Rendering r)`
gives r's outputs; `Random` takes the rendering whose build is the drawn brand, major and system
and whose machine reports the drawn GPU, else `Incoherent` naming both and listing the recorded
renderings. `fields.mimics` is set at the end.

`validate()` adds:

- for every setting that a mimicked specimen's kind records, holds a `Value`, and is not itself a
  mimic: build `self.to_builder()` with that setting set to the specimen, and when the two values
  differ, the error `"{setting} is {value}, and {kind} {name} reports {expected}"`;
- when a mimicked build is not Chromium (`cross_engine` on): warnings that the JavaScript engine,
  CSS, workers, TLS and HTTP/2 still show Chromium, and that `navigator.userAgentData` and
  `navigator.deviceMemory` remain (Chromium-only APIs the third lane removes).

## Steps

Environment for every cargo command (one at a time, agents use one job; never set
`CARGO_TARGET_DIR` or `RUSTC_WRAPPER`; mise's `mbx` wrapper caches):

```bash
export CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0
F="mcp-server test-util testing bdd"
```

A narrow library test is `rtk cargo test -p seleniumbase-rs --features "$F" --lib <path>`; a
Chrome module is `SB_HEADLESS=true rtk cargo test -p seleniumbase-rs --features "$F" --test chrome <module>:: -- --ignored --test-threads=1`.
Write long output to the scratchpad and read the `test result` lines from the whole file.

### Step 0. Merge the consistency lane and take a baseline

- **Do:** in the mimic worktree, `rtk git merge --no-ff lane/fp-consistency`. Expected conflicts:
  none in code; if `docs/SUMMARY.md`, `docs/docs.json`, `CHANGELOG.md` or `docs/changelog.md`
  conflict, keep both sides' lines (the design entries in menu order; both changelog bullets),
  then `rtk python3 -I tools/docs7_changelog.py`. Run `tokensave sync` in the worktree.
- **Also (own commit):** the "Moved" banners of `docs/adr/003-stealth-scraping-layout.md:6` and
  `docs/design/stealth_parity_plan.md:6` still call `seleniumbase-rs-mimic` a current crate; the
  crates are now `codec`, `scrape` and `wire` (stealth #6). Reword to that. Commit
  `docs: the stealth crates are codec, scrape and wire`.
- **Check:** `rtk cargo clippy --all-targets --features "$F" -- -D warnings`;
  `rtk cargo test -p seleniumbase-rs --features "$F" --lib evasion::`;
  `rtk cargo test -p seleniumbase-rs --features "$F" --test offline docs_current::`; and the
  baseline `fingerprint_consistency_chrome::` (four tests, headless). Record which `OPEN` gaps
  each persona prints; the step 14 personas are compared with this.
- **Acceptance:** all green; if the merged lane does not build or a consistency test fails on its
  own, fix it in a separate `fix(evasion): ...` commit before step 1, or stop and report when
  the fix is not local.

### Step 1. Named rows and the catalogue

- **Files:** `src/evasion/randomize.rs`, new `src/evasion/mimic.rs`, `src/evasion/mod.rs`.
- **Tests first** (in `mimic.rs` and `randomize.rs` test modules):
  - `every_specimen_name_is_unique_across_the_four_kinds`;
  - `every_name_is_lower_case_with_hyphens` (regex `^[a-z0-9]+(-[a-z0-9]+)*$`);
  - `a_name_says_its_kind` (`macbook-13-m1` is a machine, `chrome-157-macos` a build,
    `de-de-berlin` a place);
  - `an_unknown_machine_is_refused_with_every_machine_name`;
  - `an_unknown_specimen_is_refused_with_the_names_of_each_kind` (four lines);
  - `the_builds_are_every_known_major_on_four_systems_edge_and_the_safari_releases`
    (4 x 4 Chrome + 4 Edge + 4 Safari = 24);
  - `every_mac_phone_and_iphone_row_is_a_named_machine` and
    `every_pc_adapter_is_a_named_machine_of_its_system` (26);
  - `every_place_is_a_row_of_the_locale_table`;
  - `the_full_version_of_157_is_the_one_chrome_157_reports_on_this_mac`
    (`known_full_version(157)` is `157.0.8103.39`, `MeasuredOnMac`);
  - extend `the_tables_have_no_repeated_rows` to the names.
- **Do:** the row names and slugs of [The catalogue](#the-catalogue); `KNOWN_FULL_VERSIONS` row
  157 with the comment "Chrome 157.0.8103.39 on macOS, measured on 2026-11-07"; re-measure it
  with `SB_HEADLESS=true ... --test chrome client_hints_chrome:: -- --ignored --test-threads=1`
  and write the value it prints (if it is not `157.0.8103.39`, the test reads the real one). The
  `LazyLock` statics `BUILDS`, `MACHINES`, `PLACES`; `mimic.rs` with `Specimen`, the four kinds
  (`Rendering::all()` is empty until step 8), `UnknownSpecimen`. Check every `KNOWN_FULL_VERSIONS`
  caller with `tokensave_callers` before changing its length.
- **Acceptance:** the tests above and the existing `randomize` tests pass unchanged.
- **Docs:** none yet (no setting can mimic). CHANGELOG `Added`: "`evasion::mimic`: the names of
  the real builds, machines and places the randomizer draws from (`Specimen`, `Build`,
  `Machine`, `Place`, `Rendering`)"; `Changed`: "`KNOWN_FULL_VERSIONS` holds Chrome 157, measured
  on a Mac; Random still draws 154 to 156".
- **Commit:** `feat(evasion): name the rows of the randomizer's tables`.

### Step 2. Pins in the randomizer's draw

- **Files:** `src/evasion/randomize.rs`, `src/evasion/fingerprint.rs` (the one caller).
- **Tests first:**
  - `a_pinned_machine_replaces_the_drawn_one_and_keeps_the_rest_of_the_seed` (for a seed and
    each system: the pinned draw has the machine's cores, screen and GPU; region, position and
    major equal the unpinned draw's);
  - `a_pinned_pc_takes_the_first_values_of_its_tier`;
  - `a_pinned_edge_build_draws_an_edge_user_agent` and
    `a_pinned_safari_build_draws_its_version`;
  - the existing `a_forced_major_changes_the_version_and_keeps_the_rest_of_the_seed` and
    `a_given_place_replaces_the_region_and_keeps_the_rest_of_the_seed` move to `Pins` and keep
    their assertions.
- **Do:** `Pins` and the new `draw` signature; `draw_random` passes
  `Pins { major: forced_major, place, ..Pins::default() }`.
- **Acceptance:** `--lib evasion::` green, no seeded value changes (the existing seed tests are
  the guard).
- **Docs:** none (internal). **Commit:** `refactor(evasion): a draw takes pins for a build and a machine`.

### Step 3. The fifth state

- **Files:** `src/evasion/spoof.rs`, `src/evasion/fingerprint.rs`, `src/evasion/mimic.rs`,
  `src/evasion/fingerprint/file.rs` (tests), `src/evasion/fingerprint/compare.rs`
  (`setting_values` becomes `pub(super)`), `schemas/fingerprint.schema.json`, `src/lib.rs`
  (re-export `Specimen`), `src/evasion/fingerprint.rs` re-export `pub use super::mimic::Specimen;`.
- **Tests first,** in `spoof.rs`:
  - `the_five_states_are_told_apart` (replaces `the_four_states_are_told_apart`);
  - `json_writes_a_mimic_as_an_object_with_the_name` (`{"mimic":"macbook-13-m1"}`);
  - `json_reads_a_mimic_object_and_refuses_an_unknown_name_with_the_names`;
  - `a_mimic_object_is_never_read_as_a_value_of_an_object_setting` (`Spoof<ScreenSize>`,
    `Spoof<WebGl>`);
  - `map_and_as_ref_keep_a_mimic`; `json_round_trips_every_state` covers `Mimic`.

  In `fingerprint.rs`:
  - `a_mimicked_setting_holds_the_value_of_its_specimen` (`screen_spoof(Spoof::mimic(m))` with
    `m = macbook-13-m1` gives `Value(1440x900)` and `mimic_of("screen") == Some(m)`);
  - `a_built_fingerprint_holds_no_mimic_and_keeps_every_name`;
  - `a_file_writes_the_name_of_a_mimic_and_never_its_values` (the JSON has the object, not
    `1440`);
  - `to_builder_keeps_a_mimic`;
  - `random_beside_a_mimicked_machine_is_drawn_from_that_machine` (`hardware_concurrency`
    Random, screen mimics `macbook-pro-16-m3-max`: 16 cores);
  - `app_version_mimics_the_build_without_a_user_agent`;
  - `a_mimic_persona_draws_the_same_on_every_build` (decision 17);
  - `a_setting_refuses_a_kind_it_cannot_take` (timezone of `chrome-157-macos`:
    `MimicNotAvailable { setting: "timezone", takes: "it takes a place", .. }`);
  - `proxy_webrtc_oscpu_and_build_id_have_no_mimic`.

  In `file.rs`: `a_mimic_survives_a_save_and_a_load`; `an_unknown_specimen_in_a_file_names_it`.
  The schema test fails until the schema is written again.
- **Do:** the variant and its serde; `Fields::mimics`, `declared`, `set_mimic`, `specimen_of`;
  the manual `Serialize` of `Fingerprint`; check 1 of [The checks](#the-checks-build-makes);
  the resolution through `draw_random` with pins; `mimic_of`, `mimics`; the stable seed from the
  names. Then `SB_UPDATE_SCHEMAS=1 rtk cargo test -p seleniumbase-rs --features "$F" --lib evasion::fingerprint::schema`.
- **Acceptance:** `--lib evasion::` green; `tests/offline` builds (`rtk cargo test -p seleniumbase-rs --features "$F" --test offline --no-run`).
- **Docs:** `docs/stealth/fingerprints.md`: "One setting, five states" (the table gains the row
  `Mimic(specimen)`: "What the named real build, machine, place or rendering shows; read from the
  catalogue when the fingerprint is built, and written as its name") and a first "Mimic"
  section (setting one, the file form, what a built fingerprint holds). The doc comments
  `src/evasion/spoof.rs:1-35` and `src/evasion/fingerprint.rs:7-15` say five states.
  `docs/api/stealth-settings.md`: the states paragraph. CHANGELOG `Breaking`: "`Spoof` has a fifth
  variant, `Mimic(Specimen)`: an exhaustive `match` on a `Spoof` needs a `Spoof::Mimic(_)` arm.
  A file may write `{"mimic": NAME}` for a setting." `Added`: `Spoof::mimic`, `is_mimic`,
  `specimen`, `Fingerprint::mimic_of`, `mimics`, `FingerprintError::MimicNotAvailable`.
- **Commit:** `feat(evasion)!: Spoof::Mimic, a fifth state for every setting`.

### Step 4. One claimed browser

- **Files:** `src/evasion/fingerprint.rs`, `src/evasion/policy.rs`, `schemas/fingerprint.schema.json`.
- **Tests first:**
  - `two_builds_are_refused_and_both_are_named` (`user_agent` of `chrome-156-windows`,
    `client_hints` of `chrome-155-macos`);
  - `a_build_and_a_machine_of_different_systems_are_refused`;
  - `two_machines_or_two_places_are_refused`;
  - `a_given_os_type_browser_type_or_core_version_that_disagrees_with_a_build_is_refused`;
  - `a_build_sets_the_claimed_browser_system_and_major_when_none_was_given`;
  - `a_webkit_build_is_refused_unless_the_policy_claims_across_engines`;
  - `a_cross_engine_claim_is_reported_with_what_it_cannot_hide` (warnings);
  - `a_value_that_contradicts_a_mimic_wins_and_validate_reports_it` (screen 1920x1080 beside
    `iphone-15`: build succeeds, `validate().errors` names screen and `iphone-15`);
  - `every_machine_with_a_build_of_its_system_validates_without_a_warning` (for each machine:
    the Chrome build of its system at `PRESET_CHROMIUM_MAJOR`, Safari `26.0` for an iPhone with
    `cross_engine`, place `en-us-new-york`; no error, and no warning but the cross-engine ones).
- **Do:** checks 2 to 5, `browser_type_given`, `StealthPolicy::cross_engine`, the builder
  shortcut, the two `validate` additions, `FingerprintError::MixedClaims` and `CrossEngine`.
  Rewrite the schema.
- **Acceptance:** `--lib evasion::` green.
- **Docs:** `fingerprints.md` "Mimic": "One claimed browser" (the refusals, the cross-engine
  opt-in and what it cannot hide); `docs/stealth/policies.md`: `cross_engine`;
  `docs/api/stealth-settings.md`: the policy row. CHANGELOG `Added`: `StealthPolicy::cross_engine`,
  `FingerprintError::MixedClaims`, `CrossEngine`.
- **Commit:** `feat(evasion): one claimed browser for every mimic`.

### Step 5. Mimic a whole specimen

- **Files:** `src/evasion/fingerprint.rs`, `src/evasion/mimic.rs`.
- **Tests first:**
  - `mimic_sets_every_setting_its_kind_records` (for one specimen of each of build, machine and
    place, `mimics()` lists exactly `settings_of`);
  - `takes_and_settings_of_agree` (a setting is in `settings_of(k)` iff `takes` has `k`);
  - `a_setting_given_after_mimic_wins`;
  - `fingerprint_mimic_of_a_build_a_machine_and_a_place_is_a_whole_coherent_persona`
    (`validate()` clean, `is_masked` for Navigator, Screen, Graphics, Timezone).
- **Do:** `FingerprintBuilder::mimic`, `Fingerprint::mimic` (decision 16).
- **Docs:** `fingerprints.md` "Mimic": setting all with `.mimic(..)` and `Fingerprint::mimic`.
  CHANGELOG `Added`.
- **Commit:** `feat(evasion): mimic a whole build, machine or place`.

### Step 6. Canvas, WebGL output and audio become settings

- **Files:** `src/evasion/settings.rs`, `src/evasion/fingerprint.rs`, `src/evasion/fingerprint/compare.rs`
  (`SETTINGS` 29 to 32: `webgl`, `webgl_output`, `canvas`, `audio`, ...),
  `src/evasion/fingerprint/schema.rs`, `src/evasion/providers/mod.rs` (tests), `src/lib.rs`
  (re-export the new value types beside `WebGl`), `schemas/fingerprint.schema.json`.
- **Tests first:**
  - `canvas_webgl_output_and_audio_take_a_value_random_and_a_rendering`;
  - `a_recording_beside_noise_is_refused` (canvas Value with `CanvasNoise::Persistent`; audio
    with `AudioNoise::Persistent`);
  - `a_random_rendering_with_none_recorded_is_incoherent_and_names_the_build_and_machine`;
  - `the_render_settings_mask_the_rendering_dimension` (and `EVERY_DIMENSION` gains it);
  - `off_where_it_means_nothing_names_the_setting` covers the three;
  - `json_round_trips_a_fingerprint_with_every_setting_given` gives the three.
- **Do:** the value types; the three fields in every per-setting list; `Dimension::Rendering`;
  checks 6 and 7; the render branch of the resolution (no rendering exists yet). Rewrite the
  schema.
- **Docs:** `docs/api/stealth-settings.md`: three rows (what each holds, that noise and a
  recording exclude each other); `fingerprints.md`: the settings table. CHANGELOG `Added`: the
  settings and types; `Dimension::Rendering`; `FingerprintError::NoiseOverRecording`.
- **Commit:** `feat(evasion): canvas, webgl_output and audio settings`.

### Step 7. The replay script, 2D canvas

- **Files:** new `src/evasion/js/rendering_replay.js` and `src/evasion/js/rendering_record.js`,
  `src/evasion/providers/builtin.rs` (`RenderingReplayProvider`, in `all()` after `canvas_noise`),
  `src/evasion/providers/mod.rs` (tests), `src/evasion/mimic.rs` (`recording_script`).
- **The script** (`render()` template; markers `@CANVAS@`, `@WEBGL@`, `@AUDIO@` are JSON or
  `null`, `@RECORD@` is empty in replay mode and `rendering_record.js` in record mode):
  - a per-context state `{ kind, width, height, h1, h2, calls, open }` in a `WeakMap`, made by a
    wrapped `HTMLCanvasElement.prototype.getContext` (`2d`; the WebGL kinds come in step 11) and
    reset by the wrapped `width` and `height` setters of `HTMLCanvasElement.prototype`;
  - every method of `CanvasRenderingContext2D.prototype` except the readers (`getImageData`,
    `measureText`, `isPointInPath`, `isPointInStroke`, `getTransform`, `getLineDash`,
    `getContextAttributes`, `createImageData`) is wrapped to log `name(args)`; every accessor
    with a setter logs `name=value`; getters stay native. Wrappers go through `sb.native`
    (`'set ' + name` for setters);
  - arguments: numbers (`-0` as `0`), strings (JSON), booleans, `null`/`undefined`; typed arrays
    as `ta:{type}:{length}:{hash of bytes}`; a `CanvasGradient` as its creation call and its
    stops (wrap `addColorStop`); a canvas with state as `canvas:{its key}`. Anything else (an
    image, a pattern, a `Path2D`) sets `open = false`: that drawing is never replayed;
  - the key is two running 32-bit FNV-1a hashes (`Math.imul`; offsets `0x811c9dc5` and
    `0x9e3779b9`) of `"2d|{w}x{h}|"` then each `call;`, written as 16 hex digits; after 4096
    calls `open = false`;
  - replay, when `open` and the key is in `drawings`: `toDataURL()` and `toDataURL('image/png')`
    return the recorded `png`; another type draws the recorded pixels into a scratch canvas
    (`putImageData`) and calls the native `toDataURL` with the same arguments; `toBlob` gives a
    `Blob` of the recorded PNG bytes (or the scratch canvas's for another type) in a later task;
    `getImageData(sx, sy, sw, sh)` returns a new `ImageData` from the recorded bytes of that
    rectangle; `drawImage` and `createImageBitmap` of a replayed canvas use the scratch canvas;
  - record mode keeps, for each read of an open drawing, `{ key, width, height, png, rgba }`
    read natively, in a list the recorder fetches with `Runtime.evaluate`
    (`window.__sbRecorded`, only in record mode).
- **Tests first** (offline):
  - `rendering_replay_passes_the_audit`: give `audit` a second form,
    `audit_with(name, &fingerprint)`, and run it with a fingerprint whose canvas is a two-drawing
    `CanvasOutputs` and whose noise is off;
  - `the_replay_script_holds_the_recorded_keys_and_outputs`;
  - `the_replay_script_has_no_recording_code` (no `__sbRecorded`);
  - `the_recording_script_holds_no_recorded_outputs`;
  - `rendering_replay_applies_only_when_a_render_setting_is_given`.
- **Docs:** `docs/stealth/advanced/providers.md`: the provider row. CHANGELOG `Added`.
- **Commit:** `feat(evasion): replay recorded canvas drawings`.

### Step 8. Detector copies, the recorder and the first rendering

- **Files:** new `tests/support/fixture_server.rs` (loaded once in `tests/chrome/main.rs` with
  `#[path]`, like `captcha_site`), `tests/fixtures/detectors/fingerprintjs-3/`
  (`fp.min.js`, `LICENSE`, `README.md`), optionally `tests/fixtures/detectors/creepjs/`,
  `tests/fixtures/rendering/` (pages), new `tests/chrome/rendering_record_chrome.rs`,
  `tests/chrome/main.rs`, new `src/evasion/mimic/renderings/chrome-157-macos-macbook-13-m1.json`,
  `src/evasion/mimic.rs` (the embedded list), `src/evasion/randomize.rs` or `mimic.rs`
  (`RENDERINGS: LazyLock<Vec<RenderingRow>>`).
- **The copies:** FingerprintJS `3.4.2` from npm (`@fingerprintjs/fingerprintjs@3.4.2`, its UMD
  build in `dist/`; read the file list and the `LICENSE` of that version first and confirm MIT;
  v4 is BSL and is never used). CreepJS: the built bundle and `LICENSE` at one pinned commit of
  `abrahamjuliot/creepjs` (MIT), only under decision 13. Each `README.md` names the source URL,
  version or commit, sha256, the licence and what the tests use it for. Fetch with `rtk curl` from
  the package host and GitHub's raw host only; never open the hosted pages.
- **The pages:** `fingerprintjs.html` loads `/detectors/fingerprintjs-3/fp.min.js` and stores
  `FingerprintJS.load({ monitoring: false })` → `get()` → the `canvas` and `audio` components in
  a promise the test awaits (check the option's name in the 3.4.2 source; it stops the
  monitoring request). `creepjs.html` if CreepJS is kept. `unrecorded.html` draws a drawing no
  detector draws. The recorder also runs `page.capture_fingerprint()`, so the capture probe's own
  canvas and audio are recorded.
- **The recorder test** `the_shipped_rendering_of_this_mac_is_what_its_chrome_draws`
  (`#[ignore]`, `serial(chrome)`): launch headless through `sb_cdp` with no fingerprint,
  `host_resolver_rules("MAP * ~NOTFOUND , EXCLUDE 127.0.0.1")` (confirm the rule syntax in
  Chromium's documentation first), add `mimic::recording_script()` as an init script, open each
  page on the loopback server, collect `__sbRecorded`; read the full version
  (`getHighEntropyValues`) and the unmasked WebGL renderer. Assert the full version is the row of
  `KNOWN_FULL_VERSIONS` for its major and the renderer names `Apple M1`. Build the rendering JSON
  (`name`, `build`, `machine`, `provenance`, `canvas`, `webgl_output` empty, `audio` empty). With
  `SB_UPDATE_RENDERINGS=1` write `src/evasion/mimic/renderings/{name}.json`; without it, compare
  with the committed file, ignoring `provenance.recorded`, and fail naming the env variable. A
  name with no committed file fails with the line to add to the embedded list. The fixture server
  logs each path it serves; the test asserts nothing but the expected paths was asked for.
- **Offline tests first:** `every_shipped_rendering_parses_and_names_a_known_build_and_machine`;
  `a_rendering_name_is_its_build_and_its_machine`;
  `the_replay_script_of_the_shipped_rendering_stays_under_one_and_a_half_megabytes`;
  `fingerprint_mimic_of_a_rendering_turns_the_noise_off`; and a `Random` canvas with build
  `chrome-157-macos` and machine `macbook-13-m1` takes the shipped rendering.
- **Run:** `SB_HEADLESS=true SB_UPDATE_RENDERINGS=1 ... --test chrome rendering_record_chrome:: -- --ignored --test-threads=1`,
  then the same without `SB_UPDATE_RENDERINGS` (green), then the offline tests.
- **Docs:** `fingerprints.md` "Mimic": which rendering ships, its provenance, how it is recorded
  again (`SB_UPDATE_RENDERINGS=1`). CHANGELOG `Added`: the shipped rendering.
- **Commit:** `feat(evasion): record Chrome's canvas on this Mac as the first rendering`.

### Step 9. Canvas replay in a real Chrome

- **Files:** new `tests/chrome/mimic_chrome.rs`, `tests/chrome/main.rs`.
- **Tests** (headless, the step 8 server and pages, external hosts unresolvable):
  - `a_canvas_value_reaches_fingerprintjs`: a `CanvasOutputs` with the shipped keys but other
    pixels (each recorded PNG and RGBA replaced by a solid image of the same size, made in the
    test) gives FingerprintJS exactly those images. This proves replay, because on this Mac the
    shipped rendering equals Chrome's own;
  - `a_canvas_mimic_gives_fingerprintjs_the_recorded_drawings`;
  - `todataurl_toblob_and_getimagedata_agree_on_a_replayed_drawing`;
  - `a_drawing_that_was_not_recorded_keeps_the_browsers_own_pixels` (with and without the mimic,
    `unrecorded.html` reads the same bytes);
  - `the_capture_reads_the_recorded_canvas_hash` (`unmapped["canvas_hash"]` is the FNV hash of the
    recorded PNG of the capture's drawing);
  - `a_replayed_canvas_drawn_onto_another_gives_the_recorded_pixels` (opaque pixels equal; the
    page says which pixels have alpha).
- **Acceptance:** the module green, headless, one thread.
- **Docs:** `fingerprints.md` "Render mimics": the readback paths covered and not covered
  (`OffscreenCanvas`, workers), and that any drawing not recorded keeps the browser's own pixels.
- **Commit:** `test(chrome): a canvas mimic reaches FingerprintJS and the capture`.

### Step 10. Audio replay

- **Files:** `rendering_replay.js`, `rendering_record.js`, `builtin.rs`, the rendering JSON,
  `rendering_record_chrome.rs`, `mimic_chrome.rs`.
- **The script:** wrap the `create*` methods of `BaseAudioContext.prototype` (log name and
  arguments, give the node an id and map its `AudioParam`s to `(id, name)`),
  `AudioNode.prototype.connect` and `disconnect`, the `value` setter and the scheduling methods of
  `AudioParam.prototype`, the `type` setter of `OscillatorNode.prototype`,
  `AudioScheduledSourceNode.prototype.start` and `stop`, all for `OfflineAudioContext` instances
  only. `startRendering()` computes the key from `"audio|{channels}|{length}|{sampleRate}|"` and the
  log, and marks the rendered buffer (the promise's value and the `complete` event's
  `renderedBuffer`). The first `getChannelData` or `copyFromChannel` of a marked buffer whose key is
  recorded writes the recorded samples into every channel (`set`), once; then the native methods
  answer. Record mode records `{ key, channels, length, sample_rate, samples }` on that first
  read. `AnalyserNode` of a live context is not recorded.
- **Tests first:** offline `the_replay_script_holds_the_recorded_audio`; Chrome
  `an_audio_mimic_gives_fingerprintjs_the_recorded_value`,
  `an_audio_value_reaches_fingerprintjs` (other samples, as in step 9),
  `get_channel_data_and_copy_from_channel_agree`,
  `an_audio_graph_that_was_not_recorded_renders_as_the_browser_renders`. Record again with
  `SB_UPDATE_RENDERINGS=1` and commit the file in this step.
- **Docs:** `fingerprints.md` "Render mimics": audio. CHANGELOG `Added`.
- **Commit:** `feat(evasion): replay recorded offline audio`.

### Step 11. WebGL output replay

- **Files:** as step 10; `tests/fixtures/rendering/webgl.html` (one drawn triangle, read with
  `readPixels` and `toDataURL`, as the common probes do).
- **The script:** `getContext('webgl' | 'experimental-webgl' | 'webgl2')` gets a state of kind
  `webgl` or `webgl2`; every method of the context prototype except the readers (the `get*`,
  `is*`, `check*` methods, `readPixels`, `getError`) is logged; WebGL objects are ids by creation
  (`create*`, `getUniformLocation`). Answers from `ContextOutputs`: `getParameter` (a recorded
  number answers that number; a typed array is rebuilt from `{"t": "Float32Array", "v": [...]}`),
  `getSupportedExtensions` (a copy of the list), `getExtension` (`null` for a name not on the
  list, else native), `getShaderPrecisionFormat` (the native object, with its three getters on
  `WebGLShaderPrecisionFormat.prototype` answering from a `WeakMap`), `getContextAttributes` (the
  native dictionary with the recorded values). `readPixels` with `RGBA`/`UNSIGNED_BYTE` of a
  recorded drawing copies the recorded rows; `toDataURL` of a WebGL canvas returns its recorded
  `png`. `texImage2D` and `texSubImage2D` of a replayed 2D canvas use the scratch canvas. Record
  mode records every constant of the context prototype whose `getParameter` answer is a number,
  string, boolean or typed array (not 37445 or 37446), the extensions, the twelve shader
  precisions and the context attributes, and each read drawing.
- **Tests first:** offline `the_unmasked_strings_are_never_in_the_recorded_parameters`;
  Chrome `a_webgl_output_mimic_answers_the_recorded_parameters_and_extensions`,
  `an_extension_that_was_not_recorded_is_null`,
  `a_recorded_webgl_drawing_reads_back_the_recorded_pixels`,
  `the_unmasked_vendor_and_renderer_stay_the_webgl_setting`. The recorder asserts the renderer it
  records under names `Apple M1` (see [Risks](#risks-and-how-to-see-them)). Record again.
- **Docs:** `fingerprints.md` "Render mimics": WebGL; `docs/stealth/evasions.md`: the new
  provider. CHANGELOG `Added`.
- **Commit:** `feat(evasion): replay recorded WebGL answers and drawings`.

### Step 12. `sbrs fingerprint export --mimic` and the names in `presets`

- **Files:** `crates/cli/src/fingerprint.rs`, `src/cli/fingerprint_cmd.rs`,
  `completions/sbrs.usage.kdl`, `docs/help/cli_reference.md` (generated).
- **Do:** `ExportArgs` gains
  `/// Write a fingerprint that mimics NAME: a build, machine, place or rendering (repeatable; sbrs fingerprint presets lists the names).`
  `#[usage(long, value_name = "NAME", value_hint = usage::ValueHint::Other)] mimic: Vec<String>`,
  written like the global `--plugin` (`crates/cli/src/sbrs.rs:663-664`); confirm the repeatable
  form in usage-rs's documentation on Context7 first. Exactly one of `--preset`, `--random` and
  `--mimic` (status 2 otherwise, in the existing words); `--seed` and `--os` stay `--random`
  only. `ExportSource::Mimic(Vec<String>)`; `export` reads each with `Specimen::named` (an
  `InvalidConfig` with its message) and calls `Fingerprint::mimic`. `presets_table` adds, after
  the presets, `Names for --mimic:` and one line per kind (`builds:`, `machines:`, `places:`,
  `renderings:`). The `Export` help gains `--mimic`; the `Presets` help says it also lists the
  names.
- **Tests first:** in `fingerprint_cmd.rs`: `export_of_mimics_writes_their_names` (doctest
  extended), `an_unknown_mimic_name_is_an_invalid_config_with_the_names`,
  `presets_lists_the_mimic_names_by_kind`; in `crates/cli/src/fingerprint.rs`:
  `mimic_with_preset_or_random_is_status_2`, `seed_with_mimic_is_status_2`. The `usage_spec`
  test fails until the files are written again.
- **Run:** `rtk cargo test -p seleniumbase-rs-cli --bin sbrs fingerprint`;
  `SB_UPDATE_COMPLETIONS=1 rtk cargo test -p seleniumbase-rs-cli --bin sbrs usage_spec`, then
  without the variable; `rtk cargo test -p seleniumbase-rs --features "$F" --lib mcp::dev::spec`.
- **Docs:** `fingerprints.md` and `docs/stealth/profiles.md`: the command; CHANGELOG `Added`.
- **Commit:** `feat(cli): sbrs fingerprint export --mimic`.

### Step 13. `compare_fingerprint` names the mimics

- **Files:** `src/mcp/fingerprint.rs`.
- **Do:** `comparison_output` takes `mimics: &[(&'static str, Specimen)]` (from
  `expected.mimics()`), writes `mimics: screen=macbook-13-m1, ...` after the verdict when there
  are any, and `"mimics": { setting: name }` in the data.
- **Tests first:** `the_comparison_names_the_mimicked_settings` (the existing
  `comparison_output` tests are the pattern).
- **Docs:** `docs/tutorials/mcp_server.md`: one sentence. CHANGELOG `Changed`.
- **Commit:** `feat(mcp): compare_fingerprint lists the mimicked settings`.

### Step 14. The consistency test with mimic personas

- **Files:** `tests/chrome/fingerprint_consistency_chrome.rs`.
- **Tests:** next to each preset test, a mimic persona built with `Fingerprint::mimic`:
  `a_windows_mimic_persona_agrees_with_itself_where_the_evasions_reach`
  (`chrome-157-windows`, `windows-rtx-4070`, `en-us-new-york`), `a_macos_...`
  (`chrome-157-macos`, `macbook-13-m1`, `en-us-new-york`), `a_linux_...` (`chrome-157-linux`,
  `linux-rtx-4070`), `an_android_...` (`chrome-157-android`, `pixel-8`), and
  `a_macos_rendering_persona_agrees_with_itself_where_the_evasions_reach`
  (`chrome-157-macos-macbook-13-m1` and `en-us-new-york`). Each asserts through
  `assert_only_open_gaps`; the user agent check of `contradictions_of` stays.
- **Acceptance:** green, headless, one thread. A `CLOSED` rule that fires for a mimic persona and
  not for its preset (step 0 baseline) is a bug in the catalogue's values or the resolution: fix
  it there. Never move a rule from `CLOSED` to `OPEN` to pass.
- **Docs:** `docs/design/fingerprint_consistency.md`: the mimic personas are tested too.
- **Commit:** `test(chrome): mimic personas agree with themselves`.

### Step 15. Docs, the note, the gate

- **Docs:** `docs/stealth/fingerprints.md` "Mimic": the five-states table, the four kinds with the
  catalogue table and how to list names (`sbrs fingerprint presets`), setting one and setting
  all, the file form, one claimed browser, render mimics (what is replayed, what keeps the
  browser's own output, what still betrays a claim), and the shipped rendering. Tutorials that
  teach the four states (`docs/tutorials/pure_cdp.md` near `:739`,
  `docs/tutorials/browser_profiles.md` near `:433`): five states, one `Spoof::mimic` example
  each. `docs/glossary.md`: `Spoof` (five states), new entries `Mimic`, `Specimen`, `Rendering`.
  `docs/stealth/profiles.md:36`, `docs/stealth/advanced/providers.md:79`,
  `docs/adr/001-stealth-architecture.md:19` and `docs/design/fingerprint-api-v2.md:57` (a line that
  the states grew to five). `docs/stealth/introduction.md`: a mimic, like every setting, reaches
  Chromium only. `docs/stealth/troubleshooting.md`: the refusals and what each means. In
  `docs/design/fingerprint_mimic.md`: status "built (Chromium lane)" and a section "As
  implemented" listing decisions 1 to 22 that changed the note (1, 2, 3, 5, 6, 8, 9, 10, 12, 13
  and 19), the shipped rendering, and the probes covered. Doc examples on the stealth pages use
  the page's import path; whether the stealth facade re-exports `evasion::mimic` is checked in the
  stealth repository first, and an example imports `seleniumbase_rs::evasion::mimic` when it
  does not.
- **Generated:** `tools/docs_layout_tree.py` (the module and the renderings folder), then
  `mise run layout-tree`; `rtk python3 -I tools/docs7_changelog.py`; the API index test
  (`rtk cargo test -p seleniumbase-rs --features mcp-server api_index`), written again with
  `SB_UPDATE_API_INDEX=1` only if it fails; `mise run contracts`.
- **Gate** (the commands of `CLAUDE.md`, *Build and check*): `rtk cargo fmt --all -- --check`;
  `rtk cargo clippy --all-targets --features "$F" -- -D warnings`;
  `rtk proxy cargo test --features "$F"` (count the `test result` lines of the whole output);
  `RUSTDOCFLAGS="-D warnings" rtk cargo doc --no-deps --features "$F"`; `rtk cargo deny check`;
  `mdbook build`; then the four Chrome modules of this lane once more, headless.
- **Commit:** `docs(stealth): Mimic, the fifth state` (and `chore: ...` for generated files if
  they change on their own).

## Left out, and why

| Left out | Why |
|---|---|
| Firefox: thirtyfour's `bidi` feature, preload scripts, BiDi `emulation.*`, Firefox builds (`oscpu`, `build_id` mimics) | The owner's "focused on chrome for now"; the core split puts this lane after the split (hook 4 in core, the rest in stealth). |
| Cross-engine APIs (adding `userAgentData`, `connection`, `deviceMemory` in another engine, removing them for a Safari claim in Chrome) | The note's third lane; this lane refuses a cross-engine claim unless opted in and reports what it cannot hide. |
| CanvasKit re-rendering | The note's spike; nothing may depend on it before its numbers. |
| `OffscreenCanvas`, canvases in workers, `text_metrics` (`measureText`, DOM text boxes) | No provider reaches workers in Chromium's init script path today; `text_metrics` is the note's second lane. The docs say they are not covered. |
| Renderings of Windows, Linux and phones | They need a capture on those machines; the recorder runs there unchanged and writes a file the crate embeds. |
| `sbrs fingerprint capture --rendering` | The capture command runs through `BaseCase`, which would need an init script before the first navigation, and the reference pages are test fixtures, not shipped. A user rendering is a `Value` in a file, which this lane reads. |
| The `mimic:NAME` text form, `SB_MIMIC`, a global `--mimic` | No flag or variable carries a setting today (decision 19, the note's question 11). |
| A word for mimics in profile payloads | The payload format is another tool's (the note's question 10). |
| Moving the drawn majors to 155 to 157 | A randomizer change of its own; it changes every seeded identity and every preset. |
| Live `AnalyserNode` audio | A live graph's output depends on timing; only offline renders are recorded. |
| A `#[non_exhaustive]` `Spoof` | Decision 18. |

## Risks, and how to see them

| Risk | How it shows | What to do |
|---|---|---|
| A headless recording differs from a headed one on the same Mac (2D raster on the CPU against the GPU) | Not visible to the agents (headless only) | `Provenance.headless` says so; when the owner says the Mac is free, run the recorder headed once and compare; a difference becomes a second rendering or a note. |
| Headless WebGL falls back to SwiftShader | The recorder's renderer assertion (`Apple M1`) fails | Try the Chromium flags that keep ANGLE on Metal (look them up in Chromium's documentation, never guess), else record canvas and audio only and say so in "As implemented". |
| The installed Chrome moves past 157 | The recorder fails on the full version or the missing file | Add the measured row and record a new file; the 157 rendering stays. |
| A detector copy reaches the network (FingerprintJS monitoring, CreepJS's API) | The fixture server's path log has an unexpected entry, or a request fails in the console | `monitoring: false`, the host resolver rule; drop CreepJS under decision 13. |
| Record and replay compute different keys (a font that loads late, state reset) | `a_canvas_mimic_gives_fingerprintjs_the_recorded_drawings` fails | Log both call lists in record mode and diff them; fix the logger, never the expected value. |
| A detector finds the wrappers (descriptor, `toString`, prototype checks) | The provider audit; `stealth_providers_chrome` style checks of `toString` and own property names on the patched prototypes | Patch prototypes only, `sb.native` every replacement, keep getters native. |
| The wrappers cost time or break a page (games, charts) | Step 9 pages; a page with gradients and patterns | Only installed when a render setting is set; the 4096-call cap; unknown arguments close the drawing instead of guessing. |
| `drawImage`, `createImageBitmap` and `texImage2D` of a replayed canvas lose precision at low alpha (premultiplied backing) | `a_replayed_canvas_drawn_onto_another_gives_the_recorded_pixels` on alpha pixels | Documented; the PNG and `getImageData` paths are exact. |
| Recorded WebGL limits disagree with the real GPU (a page allocates the recorded `MAX_TEXTURE_SIZE`) | Not in tests | Documented under "what can still betray it". |
| Pins change what an unpinned seed draws | The existing seeded `randomize` and `fingerprint` tests fail | Keep the picks' order and count; a pin only replaces a value. |
| `{"mimic": ...}` read as a value of an object setting | `a_mimic_object_is_never_read_as_a_value_of_an_object_setting` | `Mimic` before `Value` in `Wire`, `deny_unknown_fields` on `MimicName`. |
| The embedded renderings bloat the binary and every page script | The size test (1.5 MB) | Only the specimen's outputs go into the script, and only when a render setting is set. |
| A mimic persona fires a `CLOSED` consistency rule | Step 14 | Fix the catalogue or the resolution; the rule lists stay. |
| The stealth facade does not re-export the new module | A docs example does not import | Step 15 checks the facade first. |
| The merged consistency lane is not green | Step 0 | A `fix` commit before step 1, or stop and report. |
