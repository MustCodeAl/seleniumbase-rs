---
title: "Design: fingerprint consistency"
---

# Fingerprint consistency

Status: done on `lane/fp-consistency`; what landed is listed under
`## Unreleased` in `CHANGELOG.md`, what is left is the ranked list at the end.
The steps that closed the gaps, with their decisions, are
[the implementation plan](/design/fingerprint_consistency_plan).
It builds on [Fingerprint API v2](/design/fingerprint-api-v2) and
[Fingerprint capture, export and import](/design/fingerprint-capture).

## Principle

Consistency before coverage. A detector does not need to know what the real
machine is; it only needs two surfaces that disagree. A spoof that contradicts
another surface is worse than no spoof: it says "this browser lies", which is a
stronger signal than any single honest value. So every setting a `Fingerprint`
holds must agree with every other surface a page can read, and the checker that
proves it must read the same surfaces the detectors read.

## Sources

Every detector below was read from its source (GitHub, one file at a time;
never from a hosted test page). The licence decides what may be vendored into
`tests/fixtures/`.

| Detector | Repository | Licence | What was read |
|---|---|---|---|
| fpscanner | antoinevastel/fpscanner | MIT | `src/detections/*.ts` (23 rules), `src/signals/*.ts` (media queries, codecs, WebGPU, WebGL, screen, Intl, keyboard, Client Hints, worker, iframe, browser features, plugins, CDP) |
| fp-collect | antoinevastel/fp-collect | MIT | `src/fpCollect.js` (33 attributes) |
| CreepJS | abrahamjuliot/creepjs | MIT | `src/headless/index.ts` (24 headless, stealth and like-headless checks), `src/worker/index.ts`, `src/cssmedia/index.ts`, `src/css/index.ts` |
| BotD | fingerprintjs/BotD | MIT | `src/sources/index.ts` (23 sources), `src/detectors/*.ts` (18 detectors; window size, WebGL, notification permissions, distinctive properties in full) |
| FingerprintJS v3 | fingerprintjs/fingerprintjs, branch `v3` | MIT (v4 is BSL and was not read) | `src/sources/index.ts` (35 entropy sources) |
| brotector | kaliiiiiiiiii/brotector | MIT | README (12 detections) |
| ThumbmarkJS | thumbmarkjs/thumbmarkjs | MIT | `src/components/{screen,system,hardware,locales}/index.ts` |
| ClientJS | jackspirou/clientjs | Apache-2.0 | README (method list) |
| detect-headless | infosimples/detect-headless | MIT | README (the script path has moved; the tests are the intoli six) |
| intoli headless tests (the Sannysoft page) | intoli/intoli-article-materials | MIT | both `chrome-headless-test.js` files |
| EFF Cover Your Tracks | EFForg/cover-your-tracks | AGPL-3.0 | `templates/_fingerprinting_collects.html` (the fourteen characteristics) |

Vendoring: nothing was vendored. CreepJS is MIT, but it is a TypeScript
application whose probes import a build-time module graph (`lies`, `utils`,
`errors`); the "minimal scripts" would be a build of the whole site. The
equivalent probes are written in `src/evasion/js/capture.js` instead, one
guarded reading per surface, and credited here. Cover Your Tracks is AGPL and
stays out.

## What the evasions set today

Two paths deliver a `Fingerprint` (see `src/evasion/evasions.rs` and
`src/sb_cdp/identity.rs`):

- **Protocol overrides** (native, reach workers and headers):
  `Network.setUserAgentOverride` (user agent, `platform`, `Accept-Language`,
  Client Hints metadata), `Emulation.setLocaleOverride` (Intl),
  `Emulation.setTimezoneOverride` (Intl and `Date`),
  `Emulation.setHardwareConcurrencyOverride` (the core count; step 2 of the
  [implementation plan](/design/fingerprint_consistency_plan)),
  `Emulation.setDeviceMetricsOverride` (screen size, pixel ratio, `mobile`; a
  desktop persona sends the desktop form, no viewport override and the
  persona's `screenWidth` and `screenHeight`, on both engines),
  `Emulation.setTouchEmulationEnabled` (a phone persona: touch events, the touch
  points and, through Chrome's touch emulation, `pointer: coarse` and
  `hover: none`), `Emulation.setEmulatedMedia` (the six features of
  `media_features`, which `matchMedia` and `@media` rules both obey),
  `Emulation.setGeolocationOverride`, `Browser.grantPermissions`,
  `Network.setBlockedURLs`, `Page.setBypassCSP`,
  `Emulation.setFocusEmulationEnabled`; launch flags `--lang`, `--window-size`,
  `--user-agent`, `--force-webrtc-ip-handling-policy`, `--proxy-server`,
  `--disable-blink-features=AutomationControlled`; a desktop persona's window
  is a maximized browser window (`--window-size` is the available screen less
  Chrome's tab strip and toolbar), and a phone persona's profile preference
  `plugins.always_open_pdf_externally` (`LaunchSwitch::ExternalPdf`), so it lists
  no plugins and says `pdfViewerEnabled: false`.
- **Page script providers** (JavaScript, the window only):
  `navigator_props` (platform, userAgent, appVersion, hardwareConcurrency,
  deviceMemory, maxTouchPoints, vendor, productSub, oscpu, buildID),
  `permissions` (notifications), `plugins` (bootstrap only),
  `window_geometry` (`screen.*`, `outerWidth`/`outerHeight`, `devicePixelRatio`,
  `colorDepth`), `webgl` (`UNMASKED_VENDOR_WEBGL`, `UNMASKED_RENDERER_WEBGL`,
  and for a phone GPU the extension list without S3TC, BPTC and RGTC),
  `webgpu`, `canvas_noise`, `audio_noise`, `webrtc`, `battery` (the default is a
  full battery on the charger), `connection`, `media_devices`, `fonts`
  (`document.fonts.check`/`load`, `FontFace`), `speech`, `bluetooth`, `headless`
  (`document.hidden`; it no longer touches `matchMedia`), `client_hints` (JS
  mode), `media_codecs` (`canPlayType`, `MediaSource.isTypeSupported` and
  `mediaCapabilities.decodingInfo`), `timezone` and `localization` (JS mode:
  every Intl constructor and `toLocale*String` default to the first language),
  `geolocation` (JS mode), `api_presence` (removes the operating-system APIs the
  persona's system lacks and adds none), `keyboard_layout`
  (`navigator.keyboard.getLayoutMap()`), `storage_quota`
  (`navigator.storage.estimate()` and `webkitTemporaryStorage`), `hairline`,
  `iframe`, `attach_shadow`, `tracker_block`, `webdriver`, `cdp_marker`,
  `chrome_runtime`, `native_tostring`.

The pure CDP engine never sends `Runtime.enable` or `Console.enable`
(`src/sb_cdp/trace/capture.js` relies on it), never uses `Runtime.addBinding`
(no exposed function to leak), adds no `//# sourceURL`, and can run its own
scripts in an isolated world (`Page.createIsolatedWorld`, `src/sb_cdp/frames.rs`).

## Inventory: surfaces, detectors, and what a spoofed persona shows

"Read by" counts the detectors above that read the surface. "Set today" is
what the evasions do for it. "Contradiction" is what a spoofed persona (a
Windows identity on a Mac, say) shows when only the setting is spoofed. Rank
is by how many detectors read the surface times how loud the contradiction is
(a vendor string that names the other machine is loud; a style preference is
quiet). The table is the inventory as the lane found it: "Set today" and
"Contradiction" describe the state before the lane, and
[What is left](#what-is-left-in-rank-order) says which rows are closed.

| # | Surface | Read by | Set today | Contradiction a spoofed persona shows | Rank |
|---|---|---|---|---|---|
| 1 | WebGPU `adapter.info` (`vendor`, `architecture`) vs the WebGL renderer | fpscanner `hasGPUMismatch` + `webgpu`, CreepJS, ThumbmarkJS webgl | nothing (`navigator.gpu` untouched) | the WebGL renderer says `NVIDIA ... Direct3D11` while `adapter.info.vendor` says `apple`, `architecture` `metal-3` | high |
| 2 | Worker scope (`navigator.*`, WebGL, Intl) vs the window | fpscanner 5 rules + `worker`, CreepJS worker lies, CDP consistency probes | user agent, platform and Client Hints reach workers natively; `hardwareConcurrency`, `deviceMemory`, WebGL and the JS-mode locale do not | a worker reports the real cores, memory and GPU next to the spoofed window | high |
| 3 | Screen and window geometry | CreepJS `noTaskbar`, `hasVvpScreenRes`, fpscanner 800x600, BotD `windowSize`, FingerprintJS `screenFrame`, fp-collect `screen`/`screenMediaQuery` | `screen.*` and `outer*` from the persona (JS), `screenWidth`/`screenHeight` natively | `availHeight == height` on a desktop (no taskbar or menu bar), `outerWidth > screen.width`, the headless 800x600 | high |
| 4 | CSS media features (`pointer`, `hover`, `any-pointer`, `any-hover`, `color-gamut`, `prefers-*`, `forced-colors`, `inverted-colors`, `monochrome`, `dynamic-range`) | fpscanner `mediaQueries`, CreepJS cssmedia (via `@media` and `matchMedia`), FingerprintJS 7 sources, ThumbmarkJS screen, fp-collect | `headless` forces `any-pointer: fine`, `prefers-reduced-motion: no-preference`, `prefers-color-scheme: light` through `matchMedia` only; `@media` rules in stylesheets see the real values | an Android persona with `pointer: fine`, `hover: hover` and `any-pointer: fine`; a Windows persona with `color-gamut: p3` (a Retina Mac) | high |
| 5 | Codecs: `canPlayType`, `MediaSource.isTypeSupported`, `mediaCapabilities.decodingInfo`, `RTCRtpReceiver.getCapabilities` | fpscanner `mediaCodecs` (6 hashes), fp-collect, CreepJS media | `canPlayType` for MP4/AAC/MP3 only | `canPlayType` says `probably` for H.264 while `isTypeSupported` and `decodingInfo` say no; an iOS persona plays Ogg Vorbis and WebM VP8 | medium-high |
| 6 | Fonts vs the operating system | FingerprintJS `fonts`/`fontPreferences`, CreepJS fonts (OS inference), ThumbmarkJS, EFF | `document.fonts.check`/`load` and `FontFace` only; width measurement sees the host fonts | a Windows persona on a Mac has Menlo and Helvetica Neue and no Segoe UI or Calibri | medium-high |
| 7 | API presence by operating system (`navigator.share`/`canShare`, `hid`, `serial`, `usb`, `xr`, `contacts`, `windowControlsOverlay`, `EyeDropper`, `documentPictureInPicture`, `BarcodeDetector`, `ApplePaySession`, `audioSession`, `Notification`) | CreepJS `noWebShare`, `noContactsManager`, fpscanner `browserFeatures` (30-bit mask), ThumbmarkJS `applePayVersion` | nothing | a Linux persona with `navigator.share` (Chrome on Linux has none), an Android persona with `hid`, `serial`, `EyeDropper` and `documentPictureInPicture` (desktop only), a Windows persona with `BarcodeDetector` (macOS, ChromeOS and Android only). MDN compatibility data, read 2026-11-06 | medium |
| 8 | Plugins and `pdfViewerEnabled` vs the browser and OS | BotD `pluginsLengthInconsistency`, CreepJS `noPlugins`/`pdfIsDisabled`, fpscanner `plugins`, fp-collect, intoli | left to Chrome (`plugins` provider in the bootstrap script only) | an Android persona with five PDF plugins (Chrome on Android lists none) | medium |
| 9 | Touch presence (`ontouchstart`, `TouchEvent`) vs `maxTouchPoints` | FingerprintJS `touchSupport`, fp-collect `touchScreen`, touch emulation probes | `maxTouchPoints` in JS; `Emulation.setTouchEmulationEnabled` only through the launch option `mobile` | an Android persona with `maxTouchPoints` 5 and no `ontouchstart` | medium |
| 10 | Intl default locale vs `navigator.language` and `languages[0]` | CreepJS `localeIntlEntropyIsTrusty`, fpscanner `hasMismatchLanguages` + `internationalization`, ThumbmarkJS locales | native `Emulation.setLocaleOverride` keeps them together; the JS mode changes `navigator.language` only | JS mode: `navigator.language` `ja-JP`, `Intl.DateTimeFormat().resolvedOptions().locale` `en-US` | medium (high in JS mode) |
| 11 | Time zone offset and DST vs the IANA zone | CreepJS timezone (offset vs Intl), fpscanner `hasUTCTimezone`, FingerprintJS `timezone` | native `Emulation.setTimezoneOverride` moves both; the JS mode leaves `Date` setters and `Date.parse` | JS mode: `getTimezoneOffset()` of a constructed date disagrees with the zone | medium (JS mode only) |
| 12 | User agent vs `navigator.platform` vs Client Hints (`platform`, `platformVersion`, `architecture`, `bitness`, `mobile`, brands) vs `oscpu` | fpscanner `hasPlatformMismatch`, CreepJS `uaDataIsBlank` and the platform-version lie, brotector `UAOverride`, BotD `appVersion` | all native through `Network.setUserAgentOverride` | a hand-written identity with `Win32` and a Mac user agent; empty high-entropy values when the metadata is left out | medium |
| 13 | Speech voices vs the operating system | CreepJS speech, ThumbmarkJS speech | `speech` provider when set | Apple voices (`Samantha`) on a Windows persona, `Microsoft ...` voices on a Mac | medium |
| 14 | Keyboard layout (`navigator.keyboard.getLayoutMap`) vs locale and browser | fpscanner `keyboard` | nothing | a Safari or Firefox persona with the API (Chromium only, desktop and Android); a `de-DE` locale with a QWERTY map | low-medium |
| 15 | WebGL parameters and extensions vs the renderer | CreepJS webgl, FingerprintJS `videoCard`, fp-collect `videoCard` (extensions) | the two unmasked strings only | a `Mali-G715` renderer with `WEBGL_compressed_texture_s3tc` and `MAX_TEXTURE_SIZE` 16384 (a desktop GPU) | low-medium |
| 16 | `navigator.storage.estimate().quota` vs the claimed machine | CreepJS (storage), fpscanner no | nothing | a desktop persona in a fresh context with a quota of a few hundred megabytes (an incognito or tiny-disk shape) | low |
| 17 | `doNotTrack`, `globalPrivacyControl` | FingerprintJS no, EFF (DNT) | captured only; a user setting, not a machine fact, so there is nothing to make consistent | none; recorded for the person reading the capture | none |
| 18 | `eval.toString().length` vs the engine the user agent claims | fpscanner `hasInconsistentEtsl`, fp-collect `etsl`, BotD `evalLength` | nothing can be set: the engine is V8 | a Safari or Firefox persona on Chrome (37 expected, 33 found); the only cure is not to claim another engine | persona rule |
| 19 | Battery shape | CreepJS (battery), fp-collect `battery` presence | `battery` provider | `BatteryProfile::default()` charges at 94 % with `chargingTime` 0, a shape Chrome gives only at 100 % | low |
| 20 | Automation and headless markers (`webdriver`, `window.chrome`, `Notification.permission` vs `permissions.query`, `HeadlessChrome` in the user agent or the brands, SwiftShader, `outerWidth` 0) | every headless test above | `webdriver`, `chrome_runtime`, `permissions`, `headless` providers; `--disable-blink-features=AutomationControlled` | none when the providers run; the capture reports them as `headless_markers` | covered |

CDP-level tells, which matter most for the pure CDP engine (brotector,
fpscanner `cdp`, and DevTools leak probes):

| Tell | What it reads | Where this engine stands |
|---|---|---|
| `Runtime.enable` leak | a console serialization or `Error.prepareStackTrace` getter fires while `Runtime` is enabled | never sent (`Runtime.evaluate` works without it); `evasion::cdp::is_detectable_enable` refuses it on the WebDriver path |
| exposed functions (`exposeFunctionLeak`, `cdpBindingGlobals`) | `toString()` of a bound function, native functions with empty names on `window` | `Runtime.addBinding` is never used |
| `sourceURL` leak | the stack of an error names a `//# sourceURL` or a bare file name | no script carries one; the init script runs as `Page.addScriptToEvaluateOnNewDocument` without a URL |
| main-world execution | the page notices its own `document.querySelector` called by automation | the engine evaluates in the main world; `Page.createIsolatedWorld` exists (`frames.rs`) and could carry the helper, left for the next lane |
| `Input.untrusted`, `Input.cordinatesLeak` | `isTrusted` of mouse events, click coordinates (crbug 1477537) | `Input.dispatchMouseEvent` gives trusted events; the coordinate bug is Chromium's |
| pointer behaviour | straight-line moves, zero-duration clicks | `sb_cdp::human` curves and timing |
| `navigator.webdriver` | the flag | `--disable-blink-features=AutomationControlled` and the `webdriver` provider |

## The checker

`CapturedFingerprint::contradictions()` (in `src/evasion/fingerprint/compare.rs`)
returns every pair of surfaces of one capture that disagree, as a
`Contradiction`: the rule, the two surfaces, the two values, the persona rule
they break, a short reason and how loudly a detector reacts (`high`, `medium`,
`low`). The persona is the one the capture's own user agent names (the
operating system, the browser family, mobile or not), so the checker needs no
expected fingerprint: it asks whether the browser agrees with itself. The
readings it needs are new to `capture.js` (a Web Worker, an `about:blank`
frame, WebGPU, the CSS media features, the codecs, the keyboard map, the
storage quota, the API presence map, the Intl locales and offsets, the fonts,
touch) and are kept under `CapturedFingerprint::unmapped`, because no setting
holds them.

The compare path reports them: `compare_fingerprint` and `explain_fingerprint`
on the MCP servers add a `contradictions` field, and `sbrs fingerprint capture`
lists them in its summary and writes them with `--readings`.

A headless Chrome test per persona (`tests/chrome/fingerprint_consistency_chrome.rs`)
launches the preset of each operating system, captures a local page and fails on
any contradiction the evasions can avoid; the ones a later step closes are
listed as expected, each with an assertion that flips when it is fixed.

## Identity linking (supercookies)

A tracker that cannot fingerprint a browser can still recognise it by what it
stored on the first visit. `tests/chrome/identity_linking_chrome.rs` visits a
fixture page (`tests/fixtures/identity/evercookie.html`) that writes one mark
into every store a page can reach, and reports what each store held before. A
second identity that finds the first one's mark anywhere is linked to it. All of
it runs headless against the test's own loopback server.

What the tests prove:

- **The isolation units core gives hold.** A second `BrowserContext` of one
  browser, and a second browser with its own profile folder, start with the
  cookie, `localStorage`, `sessionStorage`, IndexedDB, the Cache API, service
  workers and the HTTP cache (an `ETag` and a `Last-Modified` that name the
  visitor, so a revalidation would show) all empty, and no request carries both
  identities' cookies or a validator of the other's cache entry.
- **The harness can see a link** (the control). Two launches that share one
  profile folder are linked: the second visit finds the first one's mark in the
  cookie, `localStorage`, IndexedDB and the Cache API, one service worker, and the
  server is asked to revalidate the first one's cache entry. The control found one
  thing the plan had not: the fixture's cookie was a session cookie, which dies
  with the browser, so it now has an expiry, as a tracker's has.
- **Sockets are not shared.** With HTTP/1.1 keep-alive, the first identity's two
  visits share a connection, and the page of a second browser context comes on a
  connection of its own (HTTP/2 needs TLS and is left out).
- **A saved session carries what it holds.** A pool lease saves its cookies and
  the local storage of the origin it is on; a second lease that loads it gets
  those two and nothing else (`sessionStorage`, IndexedDB, the Cache API, service
  workers and the HTTP cache stay empty), and a lease that loads nothing starts
  empty.
- **WebRTC.** With `shield_webrtc`, a page in the default context and one in a
  new context gather no candidate. Without it, the host candidates of both are
  `.local` names and never a private address; in the run these notes come from
  the two contexts were handed different names (two each), so the names do not
  link them. The test prints the comparison on every run. If a Chrome ever
  hands both contexts one name, `shield_webrtc` or `webrtc(Spoof::Off)` is the
  cure for identities that share one browser.

What the tests do not cover, and why:

- HSTS. Chrome keeps no HSTS entry for an origin with a certificate error and
  the tests have no trusted certificate for a loopback server; a fresh profile
  folder has no dynamic HSTS state by construction.
- TLS session tickets and HTTP/2 connection reuse: the network layer of the
  stealth repository.
- The DNS cache: only a timing side channel reads it, and a loopback server
  cannot show it reliably.
- The favicon cache: headless Chrome asks for no favicon, so the test checks it
  only when one was fetched.
- `rotate_profile` and `rotate_egress`: they are not in this crate.

## What is left, in rank order

The rows of the inventory, by rank, with what the lane did. The rules still in
`OPEN` in `tests/chrome/fingerprint_consistency_chrome.rs` are the four marked
open; every other rule is in `CLOSED`, and the test fails if a preset shows one.

| # | Surface | State |
|---|---|---|
| 1 | WebGPU against WebGL | closed: the `webgpu` provider |
| 2 | Worker scope | **open** (`worker_vs_window`): the core count is sent through the protocol, but Chrome applies it to the page and not to a worker; a worker still reports the real platform, memory and GPU. A worker bootstrap changes target handling in `src/sb_cdp/browser.rs`, and a worker that never starts is worse than the tell, so it is its own lane |
| 3 | Screen and window geometry | closed: a desktop persona's window is a maximized browser window, the desktop device-metrics form goes to both engines (`no_taskbar`, `viewport_is_screen`, `window_vs_screen`, `headless_screen`, `pixel_ratio_vs_device`). Under `StealthPolicy::native_only()` no script runs and `availHeight` is the screen's: documented, not fixed |
| 4 | CSS media features | closed through `Emulation.setEmulatedMedia` (`media_features`; `media_query_lie`, `color_gamut_vs_os`, `forced_colors_vs_os`); a phone's touch emulation gives `pointer: coarse` and `hover: none`. **Open** on a host with a mouse: `any-pointer: fine` stays true (`media_pointer_vs_device`) |
| 5 | Codecs | closed: `media_codecs` answers all three. A Safari persona's other-engine codecs are not spoofed |
| 6 | Fonts | **open for good** (`fonts_vs_os`): a page measures the host's fonts, and no flag or script gives a Mac the metrics of Segoe UI. The cure is a host of the persona's system, or a persona of the host's |
| 7 | API presence | closed for an API the persona never has (`api_present_vs_persona`: the provider removes, it never adds). **Open for good** for an API the host's Chrome lacks (`api_missing_vs_persona`): a stub a page can call is a lie the call reveals |
| 8 | Plugins and the PDF viewer | closed: a phone persona opens PDFs outside the browser (a profile preference at launch), so it lists no plugin and says `pdfViewerEnabled: false` |
| 9 | Touch | closed: a phone persona turns touch emulation on |
| 10 | Intl against the language | closed in the script mode too: `localization` makes every Intl constructor and `toLocale*String` default to the first language |
| 11 | Time zone offsets | closed on the protocol path; the script mode is as the inventory describes it |
| 12 | User agent, platform, Client Hints | closed (unchanged: native) |
| 13 | Speech voices | closed (`voices_vs_os`) |
| 14 | Keyboard layout | closed: the `keyboard_layout` setting, filled from the locale, and its provider. The three maps are Windows's (KBDUS, KBDGR, KBDFR); a Mac's punctuation keys differ and no rule reads them |
| 15 | WebGL extensions | closed for the texture compression of a phone GPU (S3TC, BPTC, RGTC). WebGL parameters beyond the list (`MAX_TEXTURE_SIZE` and the like) have no rule yet |
| 16 | Storage quota | closed: the `storage_quota` setting and provider answer `estimate()` and `webkitTemporaryStorage`, also in a new browser context. The 60 % of the disk the setting draws is the share web.dev gives one origin; it was not confirmed through Context7 |
| 17 | `doNotTrack` | none |
| 18 | `eval.toString().length` | persona rule: nothing can be set, the engine is V8 |
| 19 | Battery shape | closed: the default battery is a full one on the charger |
| 20 | Automation markers | covered |

Not done, and why: a vendored detector page (fetching one is a download nobody
approved; the capture reads the same surfaces); an iOS persona test (a Safari
persona on Chrome breaks the engine rule by design, so the four Chromium presets
are the tests); `rotate_profile` and `rotate_egress` (the stealth repository's).
