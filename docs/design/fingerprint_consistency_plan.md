---
title: "Design: fingerprint consistency, the implementation plan"
---

# Fingerprint consistency: the implementation plan

Status: the plan the implementer of `lane/fp-consistency` follows, step by step.
It builds on the inventory in [Fingerprint consistency](/design/fingerprint_consistency)
and resolves its open choices. One step is one commit, in the order given; a
step starts only when the one before it is committed. Every step names the
files it touches, the Rust it adds or changes, the tests to write first, the
commands, the acceptance check, and the docs and `CHANGELOG.md` lines.

The principle stays the note's: consistency before coverage. A spoof that
contradicts another surface is worse than no spoof, so a step that cannot make
two surfaces agree leaves both natural and says so.

## What is on the branch

`batch/friend-2026-11-07..HEAD`, judged:

| Commit | What it did | Verdict |
|---|---|---|
| `89d9cb4e` | The inventory note, `docs/design/fingerprint_consistency.md` | Keep. Its "Identity linking" and "What is left" sections are placeholders that step 23 fills. |
| `40095037` | `capture.js`, `capture.rs`, `schema.rs` read the new surfaces into `CapturedFingerprint::unmapped` | Keep. |
| `c77cb8ec` | `CapturedFingerprint::contradictions`, the 34 rules in `src/evasion/fingerprint/consistency.rs` | Keep. One row is suspect: `api_expected` says Chrome on Linux has `navigator.share`, the note says it has not. Step 10 settles it from MDN's data. |
| `661d3a13` | The `contradictions` field of `compare_fingerprint` and `explain_fingerprint`, `sbrs fingerprint capture` | Keep. |
| `fc4d89ea` | The `webgpu` provider (rank 1 closed) and the phone window fix | Keep. Rank 1 is done. |
| `3237e3a2` | An Android user agent override sends no architecture | Keep. |
| `e495b575` | `Contradiction` and `Severity` at the crate root | Keep. |
| `9ebb8262` | `tests/chrome/fingerprint_consistency_chrome.rs` (four presets, `CLOSED` and `OPEN` lists) and `tests/chrome/identity_linking_chrome.rs` (two isolation tests, the evercookie fixture) | Keep. Nothing records that these Chrome tests were ever run; step 0 runs them. The `OPEN` list does not flip by itself when a gap closes; step 1 adds the guard and the per-persona lists that make it flip. |
| `dc2aef40` (wip) | Docs only: `CHANGELOG.md` and `docs/changelog.md`, the Consistency section of `docs/stealth/fingerprints.md`, a troubleshooting accordion, `cdp-page.md`, `mcp_server.md`, `fingerprint-capture.md`, `tools/docs_layout_tree.py` | Keep; the text matches the code. Tidy in step 0: a blank line splits the list under `### Added` after the `webgpu` entry and another the list under `### Fixed`; `fingerprints.md` and `troubleshooting.md` each have a doubled blank line. |

Rule names today (34): `webgpu_vs_webgl`, `webgpu_vs_os`, `webgl_vs_os`,
`worker_vs_window`, `frame_vs_window`, `headless_screen`, `window_vs_screen`,
`no_taskbar`, `viewport_is_screen`, `pixel_ratio_vs_device`,
`media_pointer_vs_device`, `color_gamut_vs_os`, `forced_colors_vs_os`,
`media_query_lie`, `codecs_disagree`, `codecs_vs_browser`, `fonts_vs_os`,
`api_presence_vs_persona`, `plugins_vs_device`, `pdf_viewer_vs_plugins`,
`touch_vs_device`, `language_vs_languages`, `intl_vs_language`,
`offset_vs_zone`, `platform_vs_user_agent`, `client_hints_vs_user_agent`,
`voices_vs_os`, `keyboard_vs_locale`, `webgl_extensions_vs_renderer`,
`storage_quota_vs_device`, `battery_shape`, `vendor_vs_user_agent`,
`product_sub_vs_user_agent`, `engine_vs_user_agent`. `CLOSED` holds 18 of them
and `OPEN` 16.

## How every step works

- Test first: write the test, run it, see it fail (a compile error of a test
  that names a missing item counts), then change the code until it passes.
- The worktree is `/Users/notlaggy/Documents/GitFolder/seleniumbase-rs/.claude/worktrees/fp-consistency`;
  every shell command starts with `cd` there. TokenSave reads and edits, with
  `graph_root` set to it; `rtk` in front of every command.
- Environment for every cargo command:
  `CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0`. One
  cargo command at a time. Long output goes to a file in the scratchpad, and
  failures are counted from the `test result:` lines of the whole file.
- Narrow runs while a step is in progress:
  `rtk proxy cargo test -p seleniumbase-rs --lib <module path>` with the
  module of the step (`evasion::fingerprint::consistency`, `evasion::evasions`,
  `evasion::providers`, `evasion::randomize`, `evasion::capture`,
  `sb_cdp::identity`, `sb_cdp::launch`, `browser::session`), and
  `--features mcp-server --lib mcp::fingerprint` for the MCP tools.
- A Chrome module, headless, one at a time:
  `SB_HEADLESS=true rtk proxy cargo test -p seleniumbase-rs --features "mcp-server test-util testing bdd" --test chrome <module>:: -- --ignored --test-threads=1 --nocapture`.
  Pages are the tests' own loopback servers and `data:` URLs only.
- Generated files: the fingerprint schema with
  `SB_UPDATE_SCHEMAS=1 rtk proxy cargo test -p seleniumbase-rs --lib evasion::fingerprint::schema`;
  the dev server's index with
  `SB_UPDATE_API_INDEX=1 rtk proxy cargo test -p seleniumbase-rs --features mcp-server api_index`
  whenever a public doc comment of `sb_cdp` or `BaseCase` changed; the docs
  changelog with `rtk python3 -I tools/docs7_changelog.py` after every
  `CHANGELOG.md` edit.
- The lists of `tests/chrome/fingerprint_consistency_chrome.rs`: a step that
  closes a gap moves its rule from `OPEN` to `CLOSED` (or into the persona's
  `closed_here`, see step 1) in the same commit. That move is the flip: the
  test then fails if the surface ever disagrees again.
- Docs change in the same commit as the code: the settings table and "What is
  spoofed" in `docs/stealth/fingerprints.md`, the provider table in
  `docs/stealth/advanced/providers.md`, the builder and accessor rows in
  `docs/api/stealth-settings.md`, the capture table in
  `docs/design/fingerprint-capture.md`, and one `CHANGELOG.md` entry under
  `## Unreleased`. To find every page that names a provider or a setting, search
  the docs with `tokensave_search` (`literal: true`, `path_include: ["docs/"]`).
- No Python-parity API changes in this lane (Python SeleniumBase has none of
  these settings), so `parity/api.toml` is not touched.

## Decisions

1. **Order.** The note's rank order, which the task names: worker scope (rank 2,
   the protocol part only), geometry (3), media features and the phone's pointer
   (4), codecs (5), API presence (7), the phone's plugins (8), Intl in the script
   mode (10), keyboard (14), WebGL extensions (15), storage quota (16), battery
   (19). The task's own list puts keyboard before storage before the APIs; the
   note's ranking wins where the two differ, as the task says to follow it.
2. **A setting only for a fact nothing else decides.** A new `Spoof` field is
   added for a fact of the machine or of its user that no other setting
   implies: `media_features` (the display's gamut and the user's preferences),
   `keyboard_layout`, `storage_quota`. A fact another setting decides is derived
   by the evasion and is never settable, so nobody can write a contradiction:
   pointer, hover and touch from a phone persona; API presence from browser,
   system and Chromium major; codecs from the browser; plugins from a phone
   persona; the Intl default locale from `locale`; the WebGPU adapter from
   `webgl` (already so).
3. **Media features go through the protocol only.** `Emulation.setEmulatedMedia`
   changes `matchMedia` and `@media` rules alike. A script can change only
   `matchMedia`, which is the `media_query_lie`, so the `headless` provider's
   `matchMedia` patch is removed and there is no script fallback: where no
   protocol is available, the features stay the browser's.
4. **Which media features.** The six Puppeteer documents for this command:
   `prefers-color-scheme`, `prefers-reduced-motion`,
   `prefers-reduced-transparency`, `prefers-contrast`, `forced-colors`,
   `color-gamut` (Context7, `/puppeteer/puppeteer`, `Page.emulateMediaFeatures`).
   `dynamic-range`, `inverted-colors`, `monochrome` and the pointer features are
   not in it and are not emulated.
5. **One `ColorScheme`.** `sb_cdp::ColorScheme` (in `src/sb_cdp/state.rs`) moves
   to `src/evasion/settings.rs`, gains serde, and `sb_cdp` re-exports it, so
   `sb_cdp::ColorScheme` keeps working and one name means one thing.
6. **A phone's pointer is touch emulation.** `Emulation.setTouchEmulationEnabled`
   for a phone persona gives `ontouchstart`, the touch points and, through
   Chrome's touch emulation, `pointer: coarse` and `hover: none`. Step 5's
   Chrome test proves the media part; if it does not move, `media_pointer_vs_device`
   stays open for phones.
7. **API presence removes, never adds.** A stub of `navigator.share` that a page
   can call is a lie the call reveals. The provider deletes only the
   operating-system APIs of Chromium (`hid`, `serial`, `windowControlsOverlay`,
   `contacts`, `share`/`canShare`, `EyeDropper`, `documentPictureInPicture`,
   `BarcodeDetector`). Engine APIs (`window.chrome`, `userAgentData`, ...) are
   left: a Firefox or Safari persona on Chrome already breaks
   `engine_vs_user_agent`, which nothing can fix.
8. **The API rule splits by direction.** `api_presence_vs_persona` becomes
   `api_present_vs_persona` (the page has an API the persona never has: the
   provider removes it, so it closes) and `api_missing_vs_persona` (the persona
   always has it and the host's Chrome lacks it: a host limit, open for good).
   Severities stay as they are.
9. **The rule list is public.** `Contradiction::RULES` lists every rule by name;
   an offline test keeps it equal to the rules the checker fires, and a test in
   the Chrome module keeps `CLOSED` and `OPEN` a partition of it. MCP clients and
   the docs can list the rules from it.
10. **A phone has no PDF viewer.** Chrome on Android lists no plugins and says
    `pdfViewerEnabled: false`. The fix is the profile preference
    `plugins.always_open_pdf_externally` at launch (native, every context of
    the profile), through the existing `LaunchSwitch::ExternalPdf`. A browser
    that is already running keeps its viewer, and no script fakes an empty
    `PluginArray` (its indexed properties would still show the real entries).
11. **A desktop window is a maximized browser window.** The launch window is the
    available screen less the browser's frame (tab strip and toolbar), so the
    viewport is never the whole screen (`viewport_is_screen`). The desktop form
    of the device-metrics override (no viewport override, the persona's
    `screenWidth` and `screenHeight`) moves from `StealthPlan` into
    `raw_overrides`, so the WebDriver session gets it too; today it forces the
    WebDriver page's viewport to the size of the screen.
12. **Keyboard maps from Windows.** The three maps are Windows's unshifted
    layouts (KBDUS, KBDGR, KBDFR), 48 keys each. `KeyboardLayout::for_locale`
    is the one table: `build()` fills `keyboard_layout` from a given locale (as
    it fills `languages`), the provider uses the value, and the checker's
    `keyboard_vs_locale` uses the same function.
13. **The storage quota in the page.** The provider answers
    `StorageManager.estimate()` and `navigator.webkitTemporaryStorage.queryUsageAndQuota`
    with the persona's quota and the real usage. `Storage.overrideQuotaForOrigin`
    is not used: it needs every origin before the page loads it and changes the
    quota Chrome enforces. The value is 60 % of the volume, the share Chrome
    gives one origin (web.dev, "Storage for the web"; not confirmed through
    Context7, whose quota was exhausted, so it is a risk below).
14. **Workers: the protocol part now, the rest in its own lane.**
    `Emulation.setHardwareConcurrencyOverride` is added (it may reach workers;
    step 2's test says). A worker bootstrap (auto-attach to worker targets with
    `waitForDebuggerOnStart`, a worker-scope script, `Runtime.runIfWaitingForDebugger`)
    is left out: it changes target handling in `src/sb_cdp/browser.rs` that
    `ensure_proxy_auth` already owns, and its failure mode, a worker that never
    starts, breaks pages, which is worse than the tell.
15. **Codecs agree; no other engine's codecs.** `media_codecs` answers
    `MediaSource.isTypeSupported` and `mediaCapabilities.decodingInfo` the way it
    answers `canPlayType`, for a Chromium build without the proprietary codecs.
    A Safari persona's codecs are not spoofed (decision 7's reason).
16. **No detector is vendored.** Fetching one (intoli's `chrome-headless-test.js`
    is MIT and a plain script) is a download the user has not approved; the
    capture reads the same surfaces. A later lane may add it under
    `tests/fixtures/intoli/` with its `LICENSE` once the user says so.
17. **The default battery is a full one on the charger**: `level` 1.0,
    `charging` true, `chargingTime` 0, which is also what Chrome reports on a
    desktop with no battery. The 94 % with no time to full is a shape Chrome
    never gives.
18. **Intl in the script mode follows the language.** The `localization`
    provider makes every Intl constructor and `toLocale*String` default to the
    persona's first language when the caller names no locale, as
    `Emulation.setLocaleOverride` does natively.
19. **Identity linking.** The isolation units core gives are a fresh profile
    folder and a `BrowserContext`. The lane adds a positive control (a kept
    profile folder is linked, so the harness can see a link), connection pools,
    the scope of a saved `Session`, and WebRTC across contexts. HSTS, TLS session
    tickets, the DNS cache and the headed favicon cache are left out (below).
    `rotate_profile` and `rotate_egress` are not in core (they are the stealth
    repository's) and are not tested here.
20. **No iOS persona test.** A Safari persona on Chrome breaks
    `engine_vs_user_agent` by design; the four Chromium presets are the tests.
21. **`native_only` keeps its promise.** Under `StealthPolicy::native_only()`
    no script runs, so `availHeight` is the screen's and `no_taskbar` shows.
    That stays documented, not fixed; the presets use `balanced()`.

## Steps

### Step 0: baseline

- **Files:** `CHANGELOG.md`, `docs/changelog.md` (regenerated),
  `docs/stealth/fingerprints.md`, `docs/stealth/troubleshooting.md`.
- **Change:** remove the four stray blank lines named in the table above. No
  code.
- **Run, before the edit, and record exit codes and counts in the report:**
  the lib tests of `evasion::`, `--features mcp-server --lib mcp::fingerprint`,
  the `api_index` test (without the update variable: it must pass, since the wip
  changed no rustdoc of `sb_cdp` or `BaseCase`), then the Chrome modules
  `fingerprint_consistency_chrome` and `identity_linking_chrome`.
- **Acceptance:** all pass; the four persona tests print which `OPEN` rules this
  host shows. If a `CLOSED` rule fires, stop: write a regression test for it,
  fix it, commit `fix(evasion): ...`, and only then go on.
- **Commit:** `docs: tidy the lists and blank lines of the consistency pages`.

### Step 1: the rule list, the split API rule, per-persona lists

- **Files:** `src/evasion/fingerprint/consistency.rs`,
  `tests/chrome/fingerprint_consistency_chrome.rs`, `CHANGELOG.md`,
  `docs/stealth/troubleshooting.md`.
- **Change:**

  ```rust
  impl Contradiction {
      /// Every rule [`CapturedFingerprint::contradictions`] applies, by the name
      /// a `Contradiction` carries in [`rule`](Self::rule), in the order the
      /// checks run.
      pub const RULES: [&'static str; 35] = [ /* the 34 above, with
          "api_presence_vs_persona" replaced by "api_present_vs_persona" and
          "api_missing_vs_persona" */ ];
  }
  ```

  In `apis`, push `"api_present_vs_persona"` when `found` is true and
  `"api_missing_vs_persona"` when it is false. In the Chrome module:
  `assert_only_open_gaps(name: &str, found: &[Contradiction], closed_here: &[&str])`,
  where a rule in `closed_here` fails the test like a `CLOSED` one; the four
  tests pass `&[]` for now. `OPEN` keeps its `(rule, why)` shape; every `why`
  starts with the step that closes it (`"step 7: ..."`) or with `"host: ..."`.
  Replace the `api_presence_vs_persona` entry by
  `("api_present_vs_persona", "step 10: ...")` and
  `("api_missing_vs_persona", "host: an API the host's Chrome lacks cannot be added honestly")`.
  Move `voices_vs_os` to `CLOSED` (every preset sets its system's voices and the
  `speech` provider answers them), unless step 0 saw it fire; then it is a bug of
  the `speech` provider, fixed first with its own test.
- **Tests first:**
  - `consistency.rs`: `every_rule_fires_somewhere_and_every_name_is_listed`:
    the union of the rules fired by `probe()`, by `android_persona(probe())`, by
    the Safari probe and by each case of `each_rule_fires_on_the_reading_it_watches`
    equals the set of `Contradiction::RULES`. Add cases to
    `each_rule_fires_on_the_reading_it_watches` for any rule no probe fires
    (both API rules among them: the Android persona on the desktop probe shows
    `hid` present and `contacts` missing).
  - Chrome module, not ignored (it needs no browser):
    `every_rule_is_closed_or_open_exactly_once`: `CLOSED` and the rules of
    `OPEN` are disjoint and together equal `Contradiction::RULES`.
- **Run:** `--lib evasion::fingerprint::consistency`; the Chrome module's
  non-ignored test with `--test chrome fingerprint_consistency_chrome::every_rule`.
- **Acceptance:** both tests green; the four persona tests still pass headless.
- **Docs:** the troubleshooting accordion names the two API rules and which
  one is a host limit. `CHANGELOG.md`, in the consistency entry: "the API rule
  is two, `api_present_vs_persona` and `api_missing_vs_persona`;
  `Contradiction::RULES` lists every rule".
- **Commit:** `feat(evasion)!: Contradiction::RULES, and the API rule splits into present and missing`.

### Step 2: the core count through the protocol (rank 2, part)

- **Files:** `src/evasion/evasions.rs`, new `tests/chrome/fingerprint_surfaces_chrome.rs`,
  `tests/chrome/main.rs`, docs.
- **Change:** in `raw_overrides`:

  ```rust
  if let Some(cores) = fp.hardware_concurrency().value() {
      map.insert(
          "Emulation.setHardwareConcurrencyOverride".to_owned(),
          serde_json::json!({ "hardwareConcurrency": cores }),
      );
  }
  ```

  The new Chrome module holds the tests that need the whole identity applied
  the way a user applies it (`LaunchOptions::builder().fingerprint(..)`): a
  loopback server like the consistency module's, and helpers
  `async fn open(persona: &Fingerprint) -> (Browser, Page)` (headless, the page
  loaded) and `async fn in_worker(page: &Page, expression: &str) -> Value`
  (evaluates `expression` in a dedicated worker made from a Blob, as
  `capture.js` does, and returns what it posts). Register
  `mod fingerprint_surfaces_chrome;` in `tests/chrome/main.rs` after
  `fingerprint_consistency_chrome`.
- **Tests first:**
  - `evasions.rs`: `the_core_count_is_also_a_protocol_override`: the Windows
    preset's overrides hold `Emulation.setHardwareConcurrencyOverride` with 8;
    a fingerprint without the setting has none.
  - Chrome: `the_core_count_reaches_the_window_and_a_worker`: the Windows preset
    with `hardware_concurrency(3)`; `navigator.hardwareConcurrency` is 3 in the
    window and in the worker. If the worker still reports the host's count, the
    override does not reach workers in this Chrome: keep the override, turn the
    worker assertion into `assert_ne!` with the message "flips when Chrome
    applies the override to workers", and record it in the report.
- **Acceptance:** green; `worker_vs_window` stays `OPEN` (memory and WebGL in a
  worker are still the host's) with its `why` updated to name step 2 and the
  worker lane.
- **Docs:** the Navigator row of the settings table names the override; the
  design note's "What the evasions set today" too. `CHANGELOG.md` Added:
  "A fingerprint's `hardware_concurrency` is also sent as
  `Emulation.setHardwareConcurrencyOverride`."
- **Commit:** `feat(evasion): the core count is also a protocol override`.

### Step 3: a desktop persona's window is a maximized browser window (rank 3)

- **Files:** `src/evasion/evasions.rs`, `src/evasion/settings.rs`,
  `src/evasion/fingerprint.rs` (the draw of `avail_screen`),
  `src/evasion/providers/builtin.rs` (`window_geometry`),
  `src/sb_cdp/launch/layers.rs` (`of_fingerprint`), the tests that assert a
  launch window, docs.
- **Change:**

  ```rust
  // settings.rs
  /// What a desktop's taskbar or menu bar takes from the height of the screen
  /// when a fingerprint gives no available screen.
  pub(crate) const TASKBAR_HEIGHT: u32 = 40;

  // evasions.rs
  /// The height Chrome's tab strip and toolbar take from a maximized window on
  /// `os`, in CSS pixels, without a bookmarks bar. A phone has none.
  pub(crate) const fn browser_frame_height(os: OsType) -> u32 {
      match os {
          OsType::Windows => 87,
          OsType::Macos => 80,
          OsType::Linux => 85,
          OsType::Android | OsType::Ios => 0,
      }
  }

  /// The window a launch gives `fp`: a phone's is its screen; a desktop's is a
  /// maximized browser window, the available screen less the browser's frame,
  /// so the page never fills the whole screen.
  pub fn window_size(fp: &Fingerprint) -> Option<(u32, u32)>
  ```

  Desktop: `(screen.width, avail.height - browser_frame_height(os))`, where
  `avail` is the `avail_screen` value or the screen less `TASKBAR_HEIGHT`. The
  three places that write 40 today (the draw of `avail_screen` in
  `fingerprint.rs`, the `window_geometry` default, any other `rtk`-found literal
  in the evasion layer, searched with `tokensave_search`) use `TASKBAR_HEIGHT`.
  `launch_args` writes `--window-size` from `window_size(fp)`;
  `of_fingerprint` sets `window_size: window_size(fingerprint)`.
- **Tests first:** `evasions.rs`:
  `a_desktop_window_is_the_available_screen_less_the_browser_frame` (Windows
  preset gives `(1920, 953)`; the macOS preset its width and its available
  height less 80); `a_phone_window_is_its_screen`; the existing
  `launch_args_include_stealth_flags` and the layers test that reads the window
  of a fingerprint (find it with `tokensave_test_map` on `layers.rs`) get the new
  numbers. Chrome, surfaces module:
  `a_desktop_persona_has_a_frame_around_its_viewport`: Windows preset;
  `innerHeight < outerHeight`, `outerHeight <= screen.availHeight`,
  `innerWidth <= outerWidth`.
- **Acceptance:** green; move `viewport_is_screen` and `no_taskbar` to `CLOSED`
  and the persona tests pass. Every other test that asserted a window or a
  viewport equal to the screen is updated, and the report lists them.
- **Docs:** the `window_geometry` row of `providers.md`, the Screen row of the
  settings table (`--window-size` is the maximized window), the
  `native_only` note of decision 21 in the Consistency section. `CHANGELOG.md`
  Fixed: "A desktop persona's launch window is a maximized browser window, the
  available screen less the tab strip and toolbar; it was the whole screen,
  which no headed browser shows (`viewport_is_screen`)."
- **Commit:** `fix(evasion): a desktop persona's window is a maximized browser window`.

### Step 4: the desktop device-metrics form for both engines

- **Files:** `src/evasion/evasions.rs`, `src/sb_cdp/identity.rs`,
  `src/browser/session/fingerprint.rs` tests, docs.
- **Change:** `raw_overrides` writes `Emulation.setDeviceMetricsOverride` in its
  final form: a desktop persona
  `{ "width": 0, "height": 0, "screenWidth": w, "screenHeight": h, "deviceScaleFactor": r, "mobile": false }`,
  a phone `{ "width": w, "height": h, "screenWidth": w, "screenHeight": h, "deviceScaleFactor": r, "mobile": true }`.
  `session_metrics` and the `DEVICE_METRICS` branch of `StealthPlan::new` go;
  the WebDriver session then sends the same form.
- **Tests first:** `evasions.rs`: `a_desktop_screen_is_reported_without_a_viewport_override`
  and `a_phone_screen_is_its_viewport`; the evasions tests that read the
  override's `width` (lines near 912, 939 and 979 today) and the identity tests
  are updated; `browser::session` gains
  `a_webdriver_session_keeps_its_window_viewport` (its override has width 0).
- **Acceptance:** `--lib evasion::evasions`, `--lib sb_cdp::identity`,
  `--lib browser::session` green; the persona tests still pass.
- **Docs:** the `apply_fingerprint` doc comment keeps its sentence about the
  `0` by `0` viewport, now true of both engines; `CHANGELOG.md` Fixed: "A
  WebDriver session given a desktop fingerprint keeps its window's viewport; the
  device-metrics override made it the size of the screen."
- **Commit:** `fix(evasion): both engines send the desktop screen without a viewport override`.

### Step 5: a phone persona turns touch emulation on (ranks 4 and 9)

- **Files:** `src/evasion/evasions.rs`, the surfaces module, the consistency
  module, docs.
- **Change:** in `raw_overrides`:

  ```rust
  if fp.os_type().is_mobile() && fp.is_masked(Dimension::Navigator) {
      let points = fp.max_touch_points().value().copied().filter(|n| *n > 0).unwrap_or(5);
      map.insert(
          "Emulation.setTouchEmulationEnabled".to_owned(),
          serde_json::json!({ "enabled": true, "maxTouchPoints": points }),
      );
  }
  ```

- **Tests first:** `evasions.rs`:
  `a_phone_persona_turns_touch_emulation_on_with_its_touch_points` (Android
  preset) and `a_desktop_persona_leaves_touch_alone`. Chrome, surfaces module:
  `a_phone_persona_has_a_coarse_pointer_that_cannot_hover_and_fires_touch_events`:
  Android preset; `'ontouchstart' in window`, `navigator.maxTouchPoints` equals
  the preset's, `matchMedia` answers `pointer: coarse`, `any-pointer: coarse`,
  `hover: none`, `any-hover: none`, and the capture's `css_media` agrees for
  the four.
- **Acceptance:** green. Move `touch_vs_device` to `CLOSED`; the Android test
  passes `closed_here: &["media_pointer_vs_device"]`. If the pointer features do
  not follow touch emulation, leave them out of `closed_here`, keep the
  assertion as `assert_ne!` with "flips when touch emulation reaches the media
  features", and record it.
- **Docs:** the Navigator row ("a phone persona: `Emulation.setTouchEmulationEnabled`");
  "What is spoofed"; `CHANGELOG.md` Added: "A phone persona turns touch
  emulation on, so touch events, the touch points and a coarse pointer that
  cannot hover agree."
- **Commit:** `feat(evasion): a phone persona's touch and pointer come from touch emulation`.

### Step 6: the `media_features` setting

- **Files:** `src/evasion/settings.rs`, `src/sb_cdp/state.rs` (the move of
  `ColorScheme`), `src/evasion/fingerprint.rs`, `src/evasion/fingerprint/compare.rs`,
  `src/evasion/fingerprint/schema.rs` and `schemas/fingerprint.schema.json`,
  `src/evasion/randomize.rs`, `src/evasion/providers/mod.rs` (the `setters` table),
  docs.
- **Change:**

  ```rust
  // settings.rs
  /// What `matchMedia` and `@media` rules report about the display and the
  /// user's preferences. The pointer and hover features belong to the device:
  /// a phone persona's touch emulation gives them.
  #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
  #[cfg_attr(test, derive(schemars::JsonSchema))]
  #[serde(default, deny_unknown_fields)]
  pub struct MediaFeatures {
      /// `color-gamut`: the widest gamut the display covers.
      pub color_gamut: ColorGamut,
      /// `prefers-color-scheme`.
      pub color_scheme: ColorScheme,
      /// `prefers-reduced-motion: reduce`.
      pub reduced_motion: bool,
      /// `prefers-reduced-transparency: reduce`.
      pub reduced_transparency: bool,
      /// `prefers-contrast`.
      pub contrast: Contrast,
      /// `forced-colors: active`, Windows high contrast.
      pub forced_colors: bool,
  }

  impl Default for MediaFeatures { /* Srgb, Light, false, false, NoPreference, false */ }

  impl MediaFeatures {
      /// The features of a display that covers `color_gamut`, with no preference set.
      #[must_use]
      pub const fn new(color_gamut: ColorGamut) -> Self;
      /// The `features` array of `Emulation.setEmulatedMedia` for these values,
      /// one `{ "name", "value" }` per feature, in the order of the fields.
      pub(crate) fn emulated(&self) -> serde_json::Value;
  }

  /// The `color-gamut` a display covers.
  #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
  #[cfg_attr(test, derive(schemars::JsonSchema))]
  #[serde(rename_all = "lowercase")]
  pub enum ColorGamut { #[default] Srgb, P3, Rec2020 }
  impl ColorGamut { pub(crate) const fn media_value(self) -> &'static str; } // "srgb", "p3", "rec2020"

  /// `prefers-contrast`.
  #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
  #[cfg_attr(test, derive(schemars::JsonSchema))]
  #[serde(rename_all = "kebab-case")]
  pub enum Contrast { #[default] NoPreference, More, Less, Custom }
  impl Contrast { pub(crate) const fn media_value(self) -> &'static str; }
  ```

  `ColorScheme` moves here unchanged in its variants (`Light`, `Dark`,
  `NoPreference`, `#[non_exhaustive]`), with
  `Serialize, Deserialize` (`rename_all = "kebab-case"`) and the schemars derive
  under `cfg(test)`; `media_value` moves with it. `src/sb_cdp/state.rs` says
  `pub use crate::evasion::settings::ColorScheme;` where the enum was, so every
  `sb_cdp::ColorScheme` path still resolves. Re-export the four types wherever
  `ScreenSize` is re-exported (find it with `tokensave_search`), nowhere more.

  `fingerprint.rs`: `media_features: Spoof<MediaFeatures>` after `color_depth`
  in `Fields`, `Default` (`Natural`), the `accessors!` and `setters!` rows
  ("`matchMedia` and `@media` for the display and the preferences. It has no
  `Off`."), `without_random`, `has_random`, a new
  `Dimension::MediaFeatures` ("The colour gamut and the preferences `matchMedia`
  and `@media` report") answered by `f.media_features.is_set()`, the draw
  `if f.media_features.is_random() { f.media_features = Spoof::Value(draw.media_features()); }`,
  `.media_features(Spoof::Random)` in `randomized_builder`, and
  `.media_features(randomize::media_features_for(os))` in `with_machine_extras`.
  `validate` warns for a gamut other than `Srgb` on Windows or Linux and for
  `forced_colors` off Windows, with the checker's wording. `compare.rs`:
  `"media_features"` after `"color_depth"` in `SETTINGS` (now 30).

  `randomize.rs`:

  ```rust
  /// The display and preferences a preset of `os` claims: a P3 display on a
  /// Mac and an iPhone, sRGB elsewhere, and the light scheme.
  pub(super) const fn media_features_for(os: OsType) -> MediaFeatures;
  impl Draw {
      /// The display of the machine and its user's preferences: the gamut of
      /// `media_features_for`, the dark scheme three times in ten, reduced
      /// motion two times in a hundred, nothing else. Stream 7.
      pub(super) fn media_features(&self) -> MediaFeatures;
  }
  ```

- **Tests first:** `settings.rs`:
  `media_features_serialize_with_their_css_words` (`"p3"`, `"no-preference"`,
  `"dark"`); `fingerprint.rs`:
  `media_features_round_trip_through_the_builder_and_the_file`,
  `a_wide_gamut_on_a_windows_persona_is_a_coherence_warning`;
  `randomize.rs`: `a_drawn_display_follows_the_system` (seeds 0 to 99: macOS and
  iOS `P3`, the rest `Srgb`, never `forced_colors`), and the draw of every other
  setting is unchanged for a fixed seed (the existing seed tests stay green);
  the preset test that every preset validates without a warning stays green.
  Then regenerate the schema.
- **Run:** `--lib evasion::`; the schema update.
- **Acceptance:** green; `SETTINGS` equals the `Spoof` fields (the existing
  test); no delivery yet, so the Chrome tests are unchanged.
- **Docs:** `stealth-settings.md` (accessor list, builder row, the new
  `Dimension`), the settings table row ("`media_features` | The display's gamut
  (P3 on a Mac or an iPhone, sRGB elsewhere) and a light or dark scheme. | no
  `Off` | `MediaFeatures` | `Emulation.setEmulatedMedia`."). `CHANGELOG.md`
  Added: "The `media_features` setting (`MediaFeatures`: `color_gamut`,
  `color_scheme`, `reduced_motion`, `reduced_transparency`, `contrast`,
  `forced_colors`); `ColorScheme` moved to `evasion::settings` and is still
  `sb_cdp::ColorScheme`."
- **Commit:** `feat(evasion)!: the media_features setting`.

### Step 7: media features reach `matchMedia` and `@media` alike

- **Files:** `src/evasion/evasions.rs`, `src/evasion/providers/builtin.rs`
  (`HeadlessProvider`), `src/sb_cdp/state.rs` (doc of `Emulation::color_scheme`),
  the surfaces and consistency modules, docs, `src/mcp/dev/api_index.json`.
- **Change:** in `raw_overrides`:

  ```rust
  if let Some(features) = fp.media_features().value() {
      map.insert(
          "Emulation.setEmulatedMedia".to_owned(),
          serde_json::json!({ "features": features.emulated() }),
      );
  }
  ```

  `HeadlessProvider` keeps the visibility getters and drops its `matchMedia`
  patch; its doc says why (a patched `matchMedia` next to an unpatched `@media`
  is `media_query_lie`). `Emulation::color_scheme` documents that it replaces
  every media feature a fingerprint set, since one `setEmulatedMedia` holds them
  all, and points to `media_features`.
- **Tests first:** `evasions.rs`: `media_features_are_one_emulated_media_override`
  (macOS preset: the six features with `color-gamut` `p3`);
  `builtin.rs`: `the_headless_fixes_leave_match_media_to_the_browser` (the
  script does not mention `matchMedia`); the old headless test that asserted the
  patch is removed. Chrome, surfaces module:
  `the_media_features_of_a_persona_reach_match_media_and_style_sheets`: the
  Windows preset with `MediaFeatures { color_scheme: Dark, reduced_motion: true, ..MediaFeatures::new(ColorGamut::Srgb) }`;
  the capture's `media_features` and `css_media` both say `srgb`, `dark` and
  `reduce`, and `matchMedia('(color-gamut: p3)')` is false.
- **Acceptance:** green. Move `media_query_lie` and `color_gamut_vs_os` to
  `CLOSED`. If one feature does not take in this Chrome, the test names it: drop
  it from `emulated()`, say so in the doc of `MediaFeatures`, and keep its rule
  open.
- **Docs:** the `headless` row of `providers.md`; "What is spoofed" lists the
  six features; regenerate `api_index.json` (the `color_scheme` doc changed).
  `CHANGELOG.md` Changed: "The `headless` provider no longer patches
  `matchMedia`; `media_features` goes through `Emulation.setEmulatedMedia`,
  which `@media` rules see too."
- **Commit:** `feat(evasion): media features go through Emulation.setEmulatedMedia`.

### Step 8: the capture reads `media_features` back

- **Files:** `src/evasion/capture.rs`, docs.
- **Change:** in `RawProbe::fingerprint_builder` (or a new `map_media` beside
  `map_graphics`), when the six readings of the probe's `mediaFeatures` are all
  present and known words, set `builder.media_features(...)`; otherwise
  `note_unanswered(unanswered, "media_features", ...)`. The raw `media_features`
  and `css_media` readings stay under `unmapped`: the rules read pointer, hover
  and the style-sheet answers there. The doc of `CapturedFingerprint::unmapped`
  says "readings no setting holds, and the raw readings the consistency rules
  compare".
- **Tests first:** `capture.rs`:
  `the_media_features_reading_becomes_the_setting` (the Windows-on-Mac fixture
  gives the values it holds) and `a_partial_media_reading_is_unanswered`; a
  `compare_to` test: a fingerprint whose `media_features` equals the capture's
  lists it under `matched`.
- **Acceptance:** `--lib evasion::capture`, `--lib evasion::fingerprint` green.
- **Docs:** the capture table of `fingerprint-capture.md`; fold into step 7's
  `CHANGELOG.md` entry: "and `capture_fingerprint` reads it back".
- **Commit:** `feat(evasion): the capture reads media_features`.

### Step 9: codecs, the three answers agree (rank 5)

- **Files:** `src/evasion/providers/builtin.rs` (`MediaCodecsProvider`),
  `tests/chrome/stealth_providers_chrome.rs`, docs.
- **Change:** the script also wraps `MediaSource.isTypeSupported` (and
  `ManagedMediaSource.isTypeSupported` when it exists) and
  `MediaCapabilities.prototype.decodingInfo` with the same `container` and
  `known` rule: only when the browser's own answer is no, for a type the rule
  accepts, the answer is yes. For `decodingInfo`, take the object the original
  resolves and set `supported`, `smooth` and `powerEfficient` to true (every
  machine the presets claim decodes H.264 and AAC in hardware), keeping its
  other keys. Each wrapper through `sb.native` with the original.
- **Tests first:** `builtin.rs`:
  `media_codecs_answers_is_type_supported_and_decoding_info_too`. Chrome:
  `a_build_without_the_proprietary_codecs_answers_alike_once_patched`: stub
  `canPlayType` to `''`, `MediaSource.isTypeSupported` to false and
  `decodingInfo` to `{ supported: false, ... }` for MP4 types, install
  `media_codecs`, then all three say yes for H.264 and AAC; and
  `the_codecs_of_a_real_chrome_are_left_alone`: unstubbed, the answers are the
  same before and after the install.
- **Acceptance:** green; no list changes (`codecs_disagree` is already
  `CLOSED`).
- **Docs:** the `media_codecs` row of `providers.md`. `CHANGELOG.md` Changed:
  "`media_codecs` answers `MediaSource.isTypeSupported` and
  `mediaCapabilities.decodingInfo` as it answers `canPlayType`."
- **Commit:** `feat(evasion): media_codecs makes the three codec answers agree`.

### Step 10: APIs the persona's system lacks are removed (rank 7)

- **Files:** `src/evasion/fingerprint/consistency.rs`, `src/evasion/fingerprint.rs`
  (a `pub(crate) use`), `src/evasion/providers/builtin.rs`,
  `src/evasion/providers/mod.rs`, `tests/chrome/stealth_providers_chrome.rs`, docs.
- **First, the data:** fetch MDN's compatibility file into the scratchpad
  (`rtk curl -sL https://raw.githubusercontent.com/mdn/browser-compat-data/main/api/Navigator.json -o <scratchpad>/Navigator.json`,
  a JSON data file, read with `/opt/homebrew/bin/jq '.api.Navigator.share.__compat.support.chrome'`),
  and set the `share` row of `api_expected` for Linux from it (expected: Chrome
  on Linux has no Web Share). Quote the support block in the commit message.
- **Change:**

  ```rust
  // consistency.rs, made crate-visible for the provider
  pub(crate) fn api_expected(
      browser: BrowserType,
      os: OsType,
      chrome_major: Option<u32>,
      api: &str,
  ) -> Option<bool>;
  pub(crate) const APIS: [(&str, &str); 22];

  // builtin.rs
  /// Removes the APIs Chrome on the persona's system does not have
  /// (`navigator.hid` for a phone, `BarcodeDetector` for Windows and Linux, ...),
  /// from the object that owns each one. It never adds an API: a stub a page can
  /// call is a lie the call reveals.
  #[derive(Clone, Copy, Debug, Default)]
  pub struct ApiPresenceProvider;
  // name "api_presence", priority 42 (between plugins 40 and window_geometry 45),
  // applies: fp.browser_type() == BrowserType::Chromium && fp.user_agent().is_value()

  /// The operating-system APIs the provider may remove: the key of `APIS`, and
  /// the global name a page reads.
  const REMOVABLE: [(&str, &str); 9] = [
      ("hid", "navigator.hid"), ("serial", "navigator.serial"),
      ("windowControlsOverlay", "navigator.windowControlsOverlay"),
      ("contacts", "navigator.contacts"), ("share", "navigator.share"),
      ("share", "navigator.canShare"), ("eyeDropper", "window.EyeDropper"),
      ("documentPictureInPicture", "window.documentPictureInPicture"),
      ("barcodeDetector", "window.BarcodeDetector"),
  ];
  ```

  The script deletes each name whose `api_expected` is `Some(false)`, from the
  first object on the prototype chain of `navigator` or `window` that owns it
  (`Object.prototype.hasOwnProperty`), inside `try`. Register it in `all()`
  after `PluginsProvider`; the provider count in
  `all_providers_are_priority_ordered` becomes 31; add
  `api_presence_passes_the_audit`; the `user_agent` row of `setters()` lists
  `"api_presence"`.
- **Tests first:** `consistency.rs`: the Linux `share` row; `builtin.rs`:
  `a_phone_persona_loses_the_desktop_only_apis` (Android preset: `hid`,
  `EyeDropper`, `documentPictureInPicture`, `windowControlsOverlay`; not
  `share`), `a_windows_persona_loses_the_barcode_detector`,
  `a_firefox_persona_is_left_alone`. Chrome:
  `the_apis_a_persona_lacks_are_gone_and_nothing_is_added`: Android preset
  installed; the four are absent (`'hid' in navigator` false, ...), and
  `Object.getOwnPropertyNames(Navigator.prototype)` lost only names of
  `REMOVABLE`.
- **Acceptance:** green; move `api_present_vs_persona` to `CLOSED`;
  `api_missing_vs_persona` stays `OPEN` as a host limit.
- **Docs:** a row in `providers.md`; "What is spoofed"; `CHANGELOG.md` Added:
  "The `api_presence` provider removes the APIs Chrome on the persona's system
  does not have, and adds none."
- **Commit:** `feat(evasion): the api_presence provider removes what the persona's system lacks`.

### Step 11: a phone persona has no PDF viewer and no plugins (rank 8)

- **Files:** `src/evasion/evasions.rs`, `src/sb_cdp/launch/layers.rs`, the
  WebDriver preference builder (the caller of `chrome_preferences` in
  `src/browser/`, found with `tokensave_callers`), the surfaces module, docs.
- **Change:**

  ```rust
  /// Whether the persona's browser has a PDF viewer: `navigator.pdfViewerEnabled`
  /// and the five PDF entries of `navigator.plugins`. A phone's has not.
  #[must_use]
  pub fn pdf_viewer_enabled(fp: &Fingerprint) -> bool {
      !fp.os_type().is_mobile()
  }
  ```

  `of_fingerprint` turns `LaunchSwitch::ExternalPdf` on when it is false. Read
  `set_switch` first: if the pure CDP engine does not act on `ExternalPdf`, put
  `"plugins.always_open_pdf_externally": true` in the layer's preferences
  instead. The WebDriver path adds the same switch when the config's fingerprint
  has no PDF viewer.
- **Tests first:** `evasions.rs`: `a_phone_persona_has_no_pdf_viewer`;
  `sb_cdp::launch`: `a_phone_fingerprint_opens_pdfs_outside_the_browser`
  (the written `Default/Preferences` holds the preference, as the existing
  launch tests read that file); a WebDriver capabilities test with the same
  check. Chrome, surfaces module:
  `a_phone_persona_lists_no_plugins_and_no_pdf_viewer`: Android preset;
  `navigator.plugins.length` and `navigator.mimeTypes.length` are 0 and
  `pdfViewerEnabled` false, in the default context and in a page of
  `browser.new_context()`.
- **Acceptance:** green; move `plugins_vs_device` and `pdf_viewer_vs_plugins`
  to `CLOSED`. If Chrome still lists the plugins, keep both open, turn the test
  into the `assert_ne!` flip, and record it.
- **Docs:** "What is spoofed"; `CHANGELOG.md` Added: "A phone persona's
  browser opens PDFs outside it, so it lists no plugins and says
  `pdfViewerEnabled: false`, as Chrome on Android."
- **Commit:** `feat(evasion): a phone persona has no PDF viewer`.

### Step 12: Intl follows `navigator.language` in the script mode (rank 10)

- **Files:** `src/evasion/providers/builtin.rs` (`LocalizationProvider`),
  `tests/chrome/stealth_providers_chrome.rs`, docs.
- **Change:** the script wraps `Intl.Collator`, `NumberFormat`, `PluralRules`,
  `RelativeTimeFormat`, `ListFormat`, `DateTimeFormat`, `DisplayNames` and
  `Segmenter` (each when it exists) in a `Proxy` whose `apply` and `construct`
  put the first language in front when `locales` is `undefined`, made
  native-looking with `sb.ctor` and with `prototype.constructor` set as
  `TimezoneProvider` does; and `Date.prototype.toLocaleString`,
  `toLocaleDateString`, `toLocaleTimeString`, `Number.prototype.toLocaleString`,
  `Array.prototype.toLocaleString`, `String.prototype.localeCompare` get the same
  default through `sb.native`. It runs after `timezone` (92 after 90), so it
  wraps the time zone's `DateTimeFormat`.
- **Tests first:** `builtin.rs`:
  `localization_makes_every_intl_constructor_default_to_the_language`. Chrome:
  `the_language_reaches_intl_in_the_script_mode`: the Windows preset with
  `locale("ja-JP")`, `languages("ja-JP,ja")` and `StealthPolicy::default()`
  (no native spoofing; check its `native_spoofing` is false with
  `tokensave_body`), providers `localization` and `timezone`;
  `Intl.DateTimeFormat().resolvedOptions().locale` and the other constructors
  say `ja-JP`, `(1234.5).toLocaleString()` equals
  `(1234.5).toLocaleString('ja-JP')`, `new Intl.NumberFormat() instanceof Intl.NumberFormat`,
  `Intl.NumberFormat.toString()` reads native, and a capture of the page shows
  no `intl_vs_language`.
- **Acceptance:** green; no list change (`intl_vs_language` is `CLOSED` for
  the protocol path already).
- **Docs:** the `localization` row of `providers.md`. `CHANGELOG.md` Fixed:
  "Without native spoofing, Intl followed the machine's locale while
  `navigator.language` said the persona's; every Intl constructor and
  `toLocale*String` now default to the persona's language."
- **Commit:** `fix(evasion): Intl follows the persona's language without native spoofing`.

### Step 13: the `keyboard_layout` setting

- **Files:** `src/evasion/settings.rs`, `src/evasion/fingerprint.rs`,
  `compare.rs`, `schema.rs` and the schema file, `randomize.rs`,
  `consistency.rs`, docs.
- **Change:**

  ```rust
  /// The layout `navigator.keyboard.getLayoutMap()` reports: the character each
  /// writing-system key types, unshifted. The maps are Windows's own layouts
  /// (KBDUS, KBDGR, KBDFR); a Mac's punctuation keys differ a little.
  #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
  #[cfg_attr(test, derive(schemars::JsonSchema))]
  #[serde(rename_all = "lowercase")]
  #[non_exhaustive]
  pub enum KeyboardLayout { Qwerty, Qwertz, Azerty }

  impl KeyboardLayout {
      /// The layout a machine set up for `locale` has, when its language has one
      /// that everybody uses: German and the languages of central Europe
      /// (`de`, `cs`, `sk`, `hu`, `sl`, `hr`) QWERTZ, French AZERTY except in
      /// Canada and Switzerland, and `en`, `es`, `it`, `pt`, `nl`, `pl`, `sv`,
      /// `da`, `nb`, `fi`, `ja`, `ko` and `zh` QWERTY; `None` otherwise.
      #[must_use]
      pub fn for_locale(locale: &str) -> Option<Self>;
      /// The 48 keys of the map, `(code, key)`.
      #[must_use]
      pub const fn keys(self) -> &'static [(&'static str, &'static str); 48];
      /// The layout a sample of a map is, read from `KeyQ`, `KeyY`, `KeyZ` and `KeyA`.
      pub(crate) fn of_sample(sample: &serde_json::Map<String, serde_json::Value>) -> Option<Self>;
  }
  ```

  The maps, unshifted, codes in this order: `Backquote`, `Digit1` to `Digit9`,
  `Digit0`, `Minus`, `Equal`, `KeyQ` to `KeyP`, `BracketLeft`, `BracketRight`,
  `Backslash`, `KeyA` to `KeyL`, `Semicolon`, `Quote`, `IntlBackslash`, `KeyZ`
  to `KeyM`, `Comma`, `Period`, `Slash`:
  - QWERTY (KBDUS): `` ` `` `1`...`0` `-` `=` `q w e r t y u i o p` `[` `]` `\`
    `a s d f g h j k l` `;` `'` `\` `z x c v b n m` `,` `.` `/`.
  - QWERTZ (KBDGR): `^` `1`...`0` `ß` `´` `q w e r t z u i o p` `ü` `+` `#`
    `a s d f g h j k l` `ö` `ä` `<` `y x c v b n m` `,` `.` `-`.
  - AZERTY (KBDFR): `²` `&` `é` `"` `'` `(` `-` `è` `_` `ç` `à` `)` `=`
    `a z e r t y u i o p` `^` `$` `*` `q s d f g h j k l` `m` `ù` `<`
    `w x c v b n ,` `;` `:` `!`.

  `fingerprint.rs`: `keyboard_layout: Spoof<KeyboardLayout>` after `timezone`,
  every place step 6 listed, `Dimension::Keyboard` ("`navigator.keyboard.getLayoutMap()`"),
  no `Off`. `build()` fills it, as it fills `languages`: when it is `Natural`
  and `locale` is a value with a `for_locale` layout, it becomes that value; a
  value given is never replaced. A `Random` layout is `for_locale` of the
  resolved locale (the given one, else the drawn region's tag), and `Natural`
  when that is `None`. `validate` warns when a value disagrees with
  `for_locale(locale)`. In `consistency.rs`, `keyboard` uses
  `KeyboardLayout::of_sample` and `for_locale` instead of its own tables
  (`layout_family` goes). `compare.rs` `SETTINGS` gains `"keyboard_layout"`
  after `"timezone"`.
- **Tests first:** `settings.rs`: `each_layout_has_48_keys_and_types_its_letters`
  (QWERTZ `KeyY` is `z`, AZERTY `KeyQ` is `a`, codes unique),
  `for_locale_picks_the_layout_of_the_language` (`de-DE`, `fr-FR`, `fr-CA`,
  `en-GB`, `ru-RU`); `fingerprint.rs`: `a_locale_fills_the_keyboard_layout`,
  `a_given_layout_is_never_replaced`, `a_random_layout_follows_the_locale`;
  `consistency.rs`: the existing `keyboard_vs_locale` case stays green on the
  shared table. Regenerate the schema.
- **Acceptance:** `--lib evasion::` green; the presets validate without a
  warning (they are `en-US`, so `Qwerty`). Fingerprint files with a locale now
  read back with a layout; the file tests that compare JSON are updated, and the
  report lists them.
- **Docs:** `stealth-settings.md`; the settings table ("`keyboard_layout` |
  The locale's layout. | no `Off` | `Keyboard` | the `keyboard_layout`
  provider."); the paragraph "`build()` also fills two values" becomes three.
  `CHANGELOG.md` Added: "The `keyboard_layout` setting (`KeyboardLayout`:
  `Qwerty`, `Qwertz`, `Azerty`), filled from a given locale."
- **Commit:** `feat(evasion)!: the keyboard_layout setting`.

### Step 14: the `keyboard_layout` provider and its capture

- **Files:** `src/evasion/providers/builtin.rs`, `providers/mod.rs`,
  `src/evasion/capture.rs`, `tests/chrome/stealth_providers_chrome.rs`, docs.
- **Change:** `KeyboardLayoutProvider`, name `"keyboard_layout"`, priority 100
  (after `geolocation` 95, before `hairline` 110), applies when
  `fp.keyboard_layout().is_value()` and the browser is Chromium. The script
  builds a `Map` of the 48 keys and replaces, on `KeyboardLayoutMap.prototype`,
  `get`, `has`, `keys`, `values`, `entries`, `forEach`, the `size` getter and
  `Symbol.iterator` (the same function as `entries`, as on a real maplike), each
  through `sb.native`, answering from the map; no instance gets an own
  property. The count of `all()` becomes 32; add the audit entry and a
  `setters()` row for `keyboard_layout`. `capture.rs` maps the probe's
  `keyboard.sample` through `KeyboardLayout::of_sample` into the setting when the
  API answered, and notes it unanswered otherwise.
- **Tests first:** `builtin.rs`: `the_keyboard_script_answers_the_persona_map`;
  `capture.rs`: `the_keyboard_sample_becomes_the_layout`. Chrome:
  `the_layout_map_is_the_persona_s`: the Windows preset with `locale("de-DE")`;
  after the install `map.get('KeyY')` is `z`, `map.size` is 48,
  `[...map].length` is 48, `map instanceof KeyboardLayoutMap`,
  `Object.getOwnPropertyNames(map)` is empty, and
  `KeyboardLayoutMap.prototype.get.toString()` reads native. If
  `getLayoutMap()` rejects in headless Chrome, the test prints why and returns,
  as the WebGPU test does.
- **Acceptance:** green; move `keyboard_vs_locale` to `CLOSED`.
- **Docs:** a row in `providers.md`; the capture table; "What is spoofed".
  `CHANGELOG.md`: fold into step 13's entry ("answered by the `keyboard_layout`
  provider and read back by the capture").
- **Commit:** `feat(evasion): the keyboard_layout provider`.

### Step 15: a phone GPU lists no desktop texture compression (rank 15)

- **Files:** `src/evasion/fingerprint/consistency.rs`,
  `src/evasion/providers/builtin.rs` (`WebglProvider`),
  `tests/chrome/stealth_providers_chrome.rs`, docs.
- **Change:** `pub(crate) fn is_mobile_gpu(renderer: &str) -> bool` in
  `consistency.rs` (the `Adreno`, `Mali`, `PowerVR` list `webgl_parameters`
  uses, which then calls it). When the `WebGl` renderer is a mobile GPU,
  `WebglProvider` also filters `getSupportedExtensions()` of WebGL1 and WebGL2
  to drop `WEBGL_compressed_texture_s3tc`, `WEBGL_compressed_texture_s3tc_srgb`,
  `EXT_texture_compression_bptc` and `EXT_texture_compression_rgtc`, and makes
  `getExtension` return `null` for them.
- **Tests first:** `builtin.rs`: `a_phone_gpu_drops_the_desktop_compression_extensions`
  and `a_desktop_gpu_keeps_them`. Chrome: `a_phone_gpu_has_no_s3tc`: Android
  preset, `webgl` installed; the list lacks the four and `getExtension` answers
  `null`.
- **Acceptance:** green; move `webgl_extensions_vs_renderer` to `CLOSED`.
- **Docs:** the `webgl` row of `providers.md`. `CHANGELOG.md` Added: "A phone
  GPU's WebGL lists no S3TC, BPTC or RGTC texture compression."
- **Commit:** `feat(evasion): a phone GPU lists no desktop texture compression`.

### Step 16: the `storage_quota` setting

- **Files:** `src/evasion/fingerprint.rs`, `compare.rs`, `schema.rs` and the
  schema file, `randomize.rs`, docs.
- **Change:** `storage_quota: Spoof<u64>` after `connection`: the quota
  `navigator.storage.estimate()` reports, in bytes; no `Off`;
  `Dimension::Storage` ("The storage quota a page is told"); every place step 6
  listed; `.storage_quota(Spoof::Random)` in `randomized_builder` and
  `.storage_quota(randomize::storage_quota_for(os))` in `with_machine_extras`.
  `SETTINGS` gains `"storage_quota"` after `"connection"`.

  ```rust
  // randomize.rs
  /// The quota Chrome gives one origin on a volume of `gigabytes` (decimal)
  /// less the `reserved` bytes the system keeps: 60 % of it.
  pub(super) const fn quota_for_disk(gigabytes: u64, reserved: u64) -> u64 {
      (gigabytes * 1_000_000_000 - reserved) / 5 * 3
  }
  /// The disks of a desktop and of a phone, in decimal gigabytes.
  const DESKTOP_DISKS: [u64; 4] = [256, 512, 1000, 2000];
  const PHONE_DISKS: [u64; 3] = [128, 256, 512];
  /// The quota a preset claims: a 512 GB desktop disk, a 128 GB phone, each
  /// less 20 GiB the system keeps.
  pub(super) const fn storage_quota_for(os: OsType) -> u64;
  impl Draw {
      /// A disk of the machine's kind, less between 8 and 40 GiB the system
      /// keeps (in whole MiB), as the quota of one origin. Stream 8.
      pub(super) fn storage_quota(&self) -> u64;
  }
  ```

- **Tests first:** `randomize.rs`:
  `a_drawn_quota_is_sixty_percent_of_a_disk_of_the_machine_s_kind` (seeds 0 to
  99: desktop quotas between 0.6 × 216 GB and 0.6 × 2000 GB, phones between
  0.6 × 88 GB and 0.6 × 512 GB, never a round number of GB);
  `fingerprint.rs`: `the_storage_quota_round_trips_through_the_builder_and_the_file`.
  Regenerate the schema.
- **Acceptance:** `--lib evasion::` green.
- **Docs:** `stealth-settings.md`; the settings table ("`storage_quota` | 60 %
  of a disk of the machine's kind. | no `Off` | `Storage` | the
  `storage_quota` provider."). `CHANGELOG.md` Added: "The `storage_quota`
  setting."
- **Commit:** `feat(evasion)!: the storage_quota setting`.

### Step 17: the `storage_quota` provider and its capture

- **Files:** `src/evasion/providers/builtin.rs`, `providers/mod.rs`,
  `src/evasion/capture.rs`, `tests/chrome/stealth_providers_chrome.rs`, the
  surfaces module, docs.
- **Change:** `StorageQuotaProvider`, name `"storage_quota"`, priority 105,
  applies when the setting is a value. `StorageManager.prototype.estimate`
  resolves the original's object with `quota` replaced (usage and
  `usageDetails` kept); `DeprecatedStorageQuota.prototype.queryUsageAndQuota`,
  for `navigator.webkitTemporaryStorage` only (the persistent one keeps its
  own), calls back with the real usage and the persona's quota. Both through
  `sb.native`. `all()` count 33; the audit entry; a `setters()` row.
  `capture.rs` maps a finite `storage.quota` into the setting.
- **Tests first:** `builtin.rs`: `the_storage_script_carries_the_quota`;
  `capture.rs`: `the_quota_reading_becomes_the_setting`. Chrome, providers
  module: `the_quota_is_the_persona_s_in_both_apis` (300 000 000 000 in
  `estimate()` and in `queryUsageAndQuota`, the usage unchanged). Surfaces
  module: `a_new_browser_context_reports_the_persona_s_quota`: the Windows
  preset; a page of `browser.new_context()` (an in-memory context, whose real
  quota is small) shows no `storage_quota_vs_device` in its capture.
- **Acceptance:** green; move `storage_quota_vs_device` to `CLOSED`.
- **Docs:** a row in `providers.md`; the capture table; "What is spoofed".
  Fold into step 16's `CHANGELOG.md` entry.
- **Commit:** `feat(evasion): the storage_quota provider`.

### Step 18: the default battery is a full one on the charger (rank 19)

- **Files:** `src/evasion/fingerprint.rs` (`BatteryProfile::default`), docs.
- **Change:** `level: 1.0`; the rest stays (`charging: true`,
  `charging_time: 0.0`, `discharging_time: f64::INFINITY`).
- **Tests first:** `fingerprint.rs`:
  `the_default_battery_is_the_shape_chrome_gives_a_full_one`; a consistency
  test: a capture of a preset's battery shows no `battery_shape`.
- **Acceptance:** green; move `battery_shape` to `CLOSED`. Search the docs for
  `94` near "battery" and correct them.
- **Docs:** `CHANGELOG.md` Fixed: "`BatteryProfile::default()` is a full
  battery on the charger; it charged at 94 % with no time to full, a shape
  Chrome never reports."
- **Commit:** `fix(evasion): the default battery is a full one on the charger`.

### Step 19: identity linking, a kept profile is linked (the control)

- **Files:** `tests/chrome/identity_linking_chrome.rs`.
- **Change:** the server answers `/mark` with a `Last-Modified` that names the
  visitor (a fixed date plus the visitor's number in seconds) beside the
  `ETag`, and records `If-Modified-Since` in `Seen::if_modified_since`.
- **Tests first:** `a_kept_profile_folder_links_two_visits`: two launches with
  `.user_data_dir(dir)` of one `tempfile::tempdir()`, closed in between; the
  second visit finds `alpha` in the cookie, `localStorage`, IndexedDB and the
  Cache API, one service worker, and the server saw `If-None-Match` or
  `If-Modified-Since`. It proves the harness sees a link. The two existing
  tests also assert `if_modified_since` is empty.
- **Acceptance:** the three tests green, headless. If the second launch misses
  a store because the first did not flush it, wait for the close to finish
  (`Browser::close` awaited) before the second launch, and say so.
- **Commit:** `test(chrome): a kept profile folder links two visits, so the harness can see a link`.

### Step 20: identity linking, each browser context opens its own connections

- **Files:** `tests/chrome/identity_linking_chrome.rs`.
- **Change:** a second server, `serve_keep_alive(log)`, that keeps
  connections open (`Connection: keep-alive`, `Content-Length`), reads requests
  in a loop on each accepted stream, numbers the streams, and records
  `(stream, phase)` for each request, where `phase` is an `Arc<AtomicU8>` the
  test sets before each visit.
- **Tests first:** `a_second_browser_context_opens_its_own_connections`: visit
  as `alpha` in the default context (phase 1), then as `beta` in a new context
  (phase 2); the streams of phase 1 and of phase 2 are disjoint. This is HTTP/1.1
  socket reuse; HTTP/2 needs TLS and is left out.
- **Acceptance:** green, headless.
- **Commit:** `test(chrome): two browser contexts share no connection`.

### Step 21: identity linking, a saved session restores only what it holds

- **Files:** `tests/chrome/identity_linking_chrome.rs`.
- **Change:** the server answers `/blank` with an empty page (it writes no
  mark).
- **Tests first:** `a_saved_session_restores_cookies_and_local_storage_and_nothing_else`:
  a `BrowserPool` (`PoolOptions::builder().max_browsers(1).contexts_per_browser(2)`);
  lease A visits as `alpha`, `save_session("alpha")`, `release()`; lease B
  opens `/blank`, `load_session("alpha")` returns true, then visits as `beta`:
  the cookie holds `alpha` and `localStorage` is `alpha`, while
  `sessionStorage`, IndexedDB, the Cache API and the service workers are empty
  and the `ETag` body is a new visitor's. A lease C that loads nothing finds
  every store empty.
- **Acceptance:** green, headless.
- **Commit:** `test(chrome): a saved session carries its cookies and local storage, nothing else`.

### Step 22: identity linking, WebRTC across two contexts

- **Files:** `tests/chrome/identity_linking_chrome.rs`.
- **Tests first:**
  `two_shielded_contexts_gather_no_candidate`: a page in the default context
  and one in a new context, each `shield_webrtc()`, then `webrtc_report()`:
  both `is_clean()` with no candidate. And
  `two_contexts_without_a_shield_show_only_mdns_host_names`: no shield; every
  host candidate's address is an `.local` name (`AddressKind::Mdns`), no raw
  private address; the test prints whether the two contexts' names are equal.
  Equal names link the two identities: the report and the design note then
  say so, and that `shield_webrtc` or `webrtc(Spoof::Off)` is the cure for
  identities that share one browser.
- **Acceptance:** green, headless; the finding is written down.
- **Commit:** `test(chrome): WebRTC across two browser contexts`.

### Step 23: the design note's findings, what is left, the final gate

- **Files:** `docs/design/fingerprint_consistency.md`, `docs/stealth/fingerprints.md`,
  `docs/stealth/troubleshooting.md`, `docs/stealth/advanced/providers.md`,
  `docs/api/stealth-settings.md`, `CHANGELOG.md`, `docs/changelog.md`,
  `src/mcp/dev/api_index.json` if stale.
- **Change:** the note's "What the evasions set today" lists the new overrides
  (`setHardwareConcurrencyOverride`, `setTouchEmulationEnabled`,
  `setEmulatedMedia`, the desktop device-metrics form, the maximized window, the
  PDF preference) and providers (`api_presence`, `keyboard_layout`,
  `storage_quota`, the changed `headless`, `media_codecs`, `localization`,
  `webgl`); its "Identity linking" section states what steps 19 to 22 proved and
  what they found; its "What is left, in rank order" is the list below with the
  final `OPEN` rules (`worker_vs_window`, `media_pointer_vs_device` on hosts
  without a pointer, `fonts_vs_os`, `api_missing_vs_persona`). The Consistency
  section of `fingerprints.md` says the same in two short lists, closed and
  open; troubleshooting points at it. The status line of the note says the lane
  is done.
- **Final gate, in order, recording exit codes and the `test result` counts:**
  `rtk cargo fmt --all -- --check`;
  `rtk cargo clippy --all-targets --features "mcp-server test-util testing bdd" -- -D warnings`;
  `rtk proxy cargo test --features "mcp-server test-util testing bdd"` into a
  scratchpad file; `RUSTDOCFLAGS="-D warnings" rtk cargo doc --no-deps --features "mcp-server test-util testing bdd"`;
  then the Chrome modules one at a time, headless:
  `fingerprint_consistency_chrome`, `fingerprint_surfaces_chrome`,
  `identity_linking_chrome`, `stealth_providers_chrome`, `client_hints_chrome`,
  `sb_cdp_capture_chrome`, `mcp_fingerprint_chrome`.
- **Commit:** `docs(stealth): what the consistency lane closed, what it found, and what is left`.

## Left out, and why

- **The worker bootstrap** (rank 2, the rest of it: `deviceMemory`, WebGL and
  the script-mode Intl of a dedicated worker). Its own lane: it changes target
  handling in `src/sb_cdp/browser.rs` and has no WebDriver equivalent;
  decision 14.
- **Fonts** (rank 6). A page measures the host's fonts and no flag or script can
  give a Mac the metrics of Segoe UI. The cure is a host of the persona's
  system, or a persona of the host's; documented, `fonts_vs_os` open for good.
- **APIs the host lacks** (`api_missing_vs_persona`). Adding one is a stub a
  call reveals; decision 7.
- **WebGL parameters beyond the extension list** (`MAX_TEXTURE_SIZE` and the
  like for a phone GPU). No rule reads them yet; a later step with its own rule.
- **Codecs of another engine** (a Safari persona's Ogg). Decision 15.
- **HSTS.** Chrome keeps no HSTS entry for an origin with a certificate error,
  and the tests have no trusted certificate for a loopback server; a fresh
  profile folder has no dynamic HSTS state by construction.
- **TLS session tickets and HTTP/2 reuse.** The stealth repository's network
  layer; core proves the socket pools are per context (step 20).
- **The DNS cache.** Only a timing side channel reads it; not testable reliably
  on a loopback server.
- **The favicon cache in a headed browser.** Headless Chrome asks for no
  favicon, and the lane runs no headed Chrome.
- **`rotate_profile` and `rotate_egress`.** Not in core.
- **A vendored detector page.** Decision 16.
- **Mac punctuation of the keyboard maps.** The maps are Windows's; no rule
  reads the punctuation keys.
- **`native_only` and the taskbar.** Decision 21.

## Risks and how to see them

| Risk | How it shows | What to do |
|---|---|---|
| Chrome refuses a new override (`setHardwareConcurrencyOverride`, a feature of `setEmulatedMedia`) and `Page::apply_fingerprint` stops at it | a Chrome test fails with "applying the fingerprint failed at ..." | drop that override or feature, keep its rule open, record it |
| `setHardwareConcurrencyOverride` does not reach workers | step 2's worker assertion | the `assert_ne!` flip; `worker_vs_window` stays open |
| Touch emulation does not move `pointer` and `hover` | step 5's media assertions | keep `media_pointer_vs_device` out of the phone's `closed_here` |
| `always_open_pdf_externally` does not empty `navigator.plugins`, or a new context does not inherit it | step 11's two pages | keep the plugin rules open, record which page |
| One `setEmulatedMedia` call replaces every feature, so a later `Emulation::color_scheme` drops the gamut | not by a test | documented on `color_scheme` (step 7) |
| The maximized window shrinks the viewport other tests expected to be the screen | failures in the full run | update those expectations in step 3, list them in the report |
| New settings change every preset and randomized fingerprint (golden files, JSON round trips, snapshot tests) | failures in the full run | regenerate only with the documented update variables, and only where the diff is the new setting |
| The keyboard fill changes fingerprint files that have a locale | file tests in step 13 | update the expectations; the fill is the intent |
| MDN's data on `navigator.share` differs from both the code and the note | step 10's read of the JSON | follow the data, quote it in the commit |
| The 60 % quota rule is not current Chrome behaviour | not by a test (Context7 could not confirm it) | print the host's real `estimate().quota` in step 17's test before the install, and compare the order of magnitude by hand |
| `getLayoutMap`, WebGPU or the storage APIs behave differently headless | the test prints and returns | run the test headed only when the user allows it |
| A kept profile's stores are not flushed when the first browser closes | step 19 misses a store | await the close; the report says so |
| Two contexts share one mDNS name | step 22 prints it | a finding, documented with its cure |
| `CLOSED` and `OPEN` drift from the checker | step 1's two guard tests | they fail on any new, renamed or dropped rule |

## The order at a glance

0 baseline; 1 rule list; 2 core count; 3 maximized window; 4 device-metrics
form; 5 touch; 6 to 8 media features; 9 codecs; 10 API presence; 11 PDF viewer;
12 Intl; 13 and 14 keyboard; 15 WebGL extensions; 16 and 17 storage quota;
18 battery; 19 to 22 identity linking; 23 docs and the gate.
