---
title: "Troubleshooting stealth"
sidebarTitle: "Troubleshooting"
---

When a site still tells the browser is automated, work through these. UC mode
has its own list in [Still detected?](/tutorials/uc_mode#still-detected), and
the scraping tools have theirs in
[Limits and troubleshooting](/scraping/advanced-stealthy/limits).


<AccordionGroup>

<Accordion title="Still flagged by bot detection" icon="flag">

**Likely cause.** The evasion script was never installed (`DriverMode::WebDriver`), the setting is `Natural`, or the site checks what this page does not change: the browser's own TLS and HTTP/2 fingerprint (see [Stealth Scraping](/scraping/stealthy/first-fetch)), the IP address, the browser's real version.

**Fix.** Use `DriverMode::Uc` or the Pure CDP engine, build the fingerprint from a preset or `Fingerprint::randomized`, check `fp.is_masked(Dimension::...)` for the dimension, and match the egress IP to the identity's place.

</Accordion>

<Accordion title="A value was given and the page does not show it" icon="eye-slash">

**Likely cause.** A later call for the same setting replaced it (`Natural` included), or the profile payload's flag for it is a `natural` word, which beats the values the payload holds.

**Fix.** Read the setting back (`fp.timezone()`), and remove the later call or the `natural` word.

</Accordion>

<Accordion title='`build()` fails with "cannot be Off" or "cannot be Random"' icon="circle-xmark">

**Likely cause.** `Off` was given to a setting with no off state, or `Random` to one with nothing to draw (`proxy`, `oscpu`, `build_id`).

**Fix.** Give a value or leave it `Natural`; the message names the setting.

</Accordion>

<Accordion title='`build()` fails with "no coherent random"' icon="circle-xmark">

**Likely cause.** A random locale, language or position next to a time zone or locale the table of places lacks.

**Fix.** Give the locale and the position as values, or use a zone the table holds (`randomize::locale_of_timezone`).

</Accordion>

<Accordion title="`navigator.webdriver` still `true`" icon="robot">

**Likely cause.** The bootstrap script did not run: `DriverMode::WebDriver`, or no fingerprint on the config. The `webdriver` provider has no flag that turns it off.

**Fix.** Use `DriverMode::Uc` or `Cdp`, or `Page::apply_fingerprint` on the Pure CDP engine. For driver-level markers, see binary patching.

</Accordion>

<Accordion title="Geolocation is wrong or never answers" icon="location-dot">

**Likely cause.** The `geolocation` setting is `Natural` or `Off`, or there is no CDP session.

**Fix.** Set `.geolocation((lat, lon))` and use `DriverMode::Uc` or `Cdp`. The position grants the permission itself; `Off` refuses it on purpose.

</Accordion>

<Accordion title="Inconsistent locale and time zone" icon="clock">

**Likely cause.** `locale` and `timezone` come from different countries. `validate()` warns about this pair.

**Fix.** Align both, or use `Fingerprint::randomized`, which draws them from one place.

</Accordion>

<Accordion title="WebRTC leaks local IP" icon="network-wired">

**Likely cause.** The policy has no `webrtc_policy` or a looser one, and the `webrtc` setting is `Natural`.

**Fix.** Use `StealthPolicy::balanced()` (the presets do), which keeps `WebRtcPolicy::DisableNonProxiedUdp`, or turn WebRTC `Off`.

</Accordion>

<Accordion title="Two surfaces of the page disagree" icon="code-compare">

**Likely cause.** A setting reaches one surface and not another that reports the same fact: the WebGL renderer was replaced and the WebGPU adapter was not, `navigator.hardwareConcurrency` in the window and not in a worker, `matchMedia` and not the `@media` rules, a phone persona whose pointer is `fine`. A detector needs no fingerprint to see this, only the two readings.

**Fix.** Capture and read `CapturedFingerprint::contradictions()` (the `contradictions` field of `compare_fingerprint` and `explain_fingerprint`, the summary of `sbrs fingerprint capture`), loudest first. Each names the rule, the two surfaces, the two values and the persona rule they break. For a surface the evasions reach (`webgl_vs_os`, `client_hints_vs_user_agent`, `window_vs_screen`, ...), fix the fingerprint the message points at; for one they do not yet (a worker's `navigator`, the host's fonts and APIs), the ranked list in [Fingerprint consistency](/design/fingerprint_consistency) says what is open, and `tests/chrome/fingerprint_consistency_chrome.rs` which rules a preset may still show.

</Accordion>


</AccordionGroup>

---

This page was brought up to date with the change that makes a setting one `Spoof`
and moves the behaviour into the `StealthPolicy` by reading `src/evasion/**`,
`src/sb_cdp/{identity,launch}.rs`, `src/browser/{config,session,playwright}.rs`,
`src/profile_payloads/**`, `src/mcp/stealth.rs` and `src/macros.rs` again. The code examples were compiled
against that source as a test of the crate; the tables were checked against the
`applies` conditions of the providers.
