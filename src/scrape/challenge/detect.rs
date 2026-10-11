//! Detects bot-protection challenges in a response, with the evidence that supports each verdict.
//!
//! A response is a challenge only when it carries an interstitial marker (see
//! [`markers`](super::markers)). A vendor header, a CAPTCHA widget, or a bare 403 is evidence and
//! nothing more: [`detect`] returns [`ChallengeKind::None`] with that evidence attached.

use serde::{Deserialize, Serialize};

use super::markers::{HEADER_HINTS, HEADER_MARKERS, MARKERS, WIDGETS};
use crate::sb_cdp::Captcha;

/// How much of a body is scanned. Anything past this is ignored, so a huge response costs a
/// bounded scan and a marker buried deep in it cannot be used as a verdict.
pub const MAX_SCAN_BYTES: usize = 256 * 1024;

/// The status of a response that asks the client to slow down.
const RATE_LIMITED_STATUS: u16 = 429;

/// A bot-protection vendor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Vendor {
    /// Cloudflare (Turnstile and the JavaScript challenge).
    Cloudflare,
    /// `DataDome`.
    DataDome,
    /// `PerimeterX` (HUMAN).
    PerimeterX,
    /// Akamai Bot Manager.
    Akamai,
    /// Imperva (Incapsula).
    Imperva,
    /// Kasada.
    Kasada,
    /// AWS WAF.
    AwsWaf,
}

/// What a response asks of the client.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ChallengeKind {
    /// The page is not a challenge.
    None,
    /// A script must run and answer before the page is real.
    JsChallenge,
    /// A widget the user, or a solver, must complete.
    Interactive,
    /// The site refuses the client outright.
    Blocked,
    /// The site asks the client to slow down.
    RateLimited {
        /// Seconds to wait, from a `Retry-After` header that is a plain number of seconds. A
        /// date in that header is not read.
        retry_after_secs: Option<u64>,
    },
}

impl ChallengeKind {
    /// How strong a verdict is, for choosing between markers: an interactive page outranks a
    /// script challenge, which outranks a plain block.
    const fn strength(self) -> u8 {
        match self {
            Self::Interactive => 3,
            Self::JsChallenge => 2,
            Self::Blocked => 1,
            Self::None | Self::RateLimited { .. } => 0,
        }
    }
}

/// What the scan found, kept with the verdict so a caller can log why.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Evidence {
    /// The vendor behind the verdict, or the vendor a header says fronts the site when there is
    /// no verdict.
    pub vendor: Option<Vendor>,
    /// The markers and header names that matched, in table order. A header such as `cf-ray`
    /// appears here too, though it never decides anything.
    pub markers: Vec<&'static str>,
    /// The CAPTCHA widget on the page, when one is present, whether or not the page is a challenge.
    pub widget: Option<Captcha>,
}

/// A verdict and the evidence behind it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Detection {
    /// What the response asks of the client.
    pub kind: ChallengeKind,
    /// What the scan saw.
    pub evidence: Evidence,
}

/// The parts of a response that detection reads.
#[derive(Clone, Copy, Debug)]
pub struct ResponseView<'a> {
    /// The URL that was requested. Detection does not read it today; it is part of the view so
    /// that a marker in the URL can be added without changing any caller.
    pub url: &'a str,
    /// The HTTP status code.
    pub status: u16,
    /// Response headers as (name, value) pairs. Names match in any case.
    pub headers: &'a [(String, String)],
    /// The response body, as text.
    pub body: &'a str,
}

/// The strongest marker seen so far.
#[derive(Clone, Copy)]
struct Finding {
    kind: ChallengeKind,
    vendor: Vendor,
}

/// Decides what `view` asks of the client.
///
/// Markers are matched in the first [`MAX_SCAN_BYTES`] of the body and in the headers, without
/// regard to case. A vendor header or a widget alone is evidence, not a verdict; so is a status
/// such as 403 with no marker. Status 429 without a marker is [`ChallengeKind::RateLimited`].
#[must_use]
pub fn detect(view: &ResponseView<'_>) -> Detection {
    let body = scan_window(view.body).to_ascii_lowercase();
    let mut evidence = Evidence {
        widget: find_widget(&body),
        ..Evidence::default()
    };
    let mut strongest: Option<Finding> = None;

    for marker in &MARKERS {
        if marker.gate.allows(view.status) && body.contains(marker.text) {
            evidence.markers.push(marker.text);
            strongest = Some(stronger(strongest, marker.kind, marker.vendor));
        }
    }
    for marker in &HEADER_MARKERS {
        let value = marker.value;
        if header_values(view.headers, marker.name).any(|v| v.trim().eq_ignore_ascii_case(value)) {
            evidence.markers.push(marker.label);
            strongest = Some(stronger(strongest, marker.kind, marker.vendor));
        }
    }
    let mut hinted = None;
    for hint in &HEADER_HINTS {
        let seen = header_values(view.headers, hint.name)
            .any(|v| hint.contains.is_empty() || v.to_ascii_lowercase().contains(hint.contains));
        if seen {
            evidence.markers.push(hint.label);
            hinted.get_or_insert(hint.vendor);
        }
    }

    let kind = if let Some(found) = strongest {
        evidence.vendor = Some(found.vendor);
        // A challenge or block page with a widget on it is one a solver can act on.
        match found.kind {
            ChallengeKind::JsChallenge | ChallengeKind::Blocked if evidence.widget.is_some() => {
                ChallengeKind::Interactive
            }
            kind => kind,
        }
    } else {
        evidence.vendor = hinted;
        if view.status == RATE_LIMITED_STATUS {
            ChallengeKind::RateLimited {
                retry_after_secs: retry_after(view.headers),
            }
        } else {
            ChallengeKind::None
        }
    };
    Detection { kind, evidence }
}

/// The cookie that proves a vendor's challenge was passed, once it is set.
#[must_use]
pub fn clearance_cookie(vendor: Vendor) -> Option<&'static str> {
    match vendor {
        Vendor::Cloudflare => Some("cf_clearance"),
        Vendor::DataDome => Some("datadome"),
        Vendor::PerimeterX => Some("_px3"),
        Vendor::Akamai => Some("_abck"),
        Vendor::AwsWaf => Some("aws-waf-token"),
        Vendor::Imperva => Some("reese84"),
        Vendor::Kasada => None,
    }
}

/// The part of `body` that is scanned: at most [`MAX_SCAN_BYTES`], cut on a character boundary.
fn scan_window(body: &str) -> &str {
    let mut end = body.len().min(MAX_SCAN_BYTES);
    while !body.is_char_boundary(end) {
        end -= 1;
    }
    &body[..end]
}

/// The widget whose mark is in `body` (already lower case), in the order of [`WIDGETS`].
fn find_widget(body: &str) -> Option<Captcha> {
    WIDGETS
        .iter()
        .find(|(_, marks)| marks.iter().any(|mark| body.contains(mark)))
        .map(|(widget, _)| *widget)
}

/// Every value of the headers named `name`, compared without case.
fn header_values<'a>(
    headers: &'a [(String, String)],
    name: &'a str,
) -> impl Iterator<Item = &'a str> {
    headers
        .iter()
        .filter(move |(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

/// The `Retry-After` wait in seconds, when the header is a plain whole number.
fn retry_after(headers: &[(String, String)]) -> Option<u64> {
    let value = header_values(headers, "retry-after").next()?.trim();
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    value.parse().ok()
}

/// The stronger of the finding so far and one more marker. The first of equals is kept.
fn stronger(so_far: Option<Finding>, kind: ChallengeKind, vendor: Vendor) -> Finding {
    match so_far {
        Some(current) if current.kind.strength() >= kind.strength() => current,
        _ => Finding { kind, vendor },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scrape::challenge::markers::{Gate, Marker};
    use crate::stealth::humanize::Rng;

    macro_rules! fixture {
        ($name:literal) => {
            include_str!(concat!("../../../tests/fixtures/challenge/", $name))
        };
    }

    type Headers = &'static [(&'static str, &'static str)];

    struct Case {
        name: &'static str,
        body: &'static str,
        status: u16,
        headers: Headers,
        kind: ChallengeKind,
        vendor: Option<Vendor>,
    }

    const CF_HEADERS: Headers = &[
        ("Server", "cloudflare"),
        ("CF-RAY", "8a1b2c3d4e5f6071-AMS"),
        ("cf-mitigated", "challenge"),
    ];
    const CF_PLAIN: Headers = &[("server", "cloudflare"), ("cf-ray", "8a1b2c3d4e5f6071-AMS")];

    fn case(
        name: &'static str,
        body: &'static str,
        status: u16,
        headers: Headers,
        kind: ChallengeKind,
        vendor: Option<Vendor>,
    ) -> Case {
        Case {
            name,
            body,
            status,
            headers,
            kind,
            vendor,
        }
    }

    const JS: ChallengeKind = ChallengeKind::JsChallenge;
    const NONE: ChallengeKind = ChallengeKind::None;

    /// The Cloudflare fixtures, with the response each was served in.
    fn cloudflare_cases() -> Vec<Case> {
        let cloudflare = Some(Vendor::Cloudflare);
        vec![
            case(
                "cloudflare_managed",
                fixture!("cloudflare_managed.html"),
                403,
                CF_HEADERS,
                JS,
                cloudflare,
            ),
            case(
                "cloudflare_legacy_js",
                fixture!("cloudflare_legacy_js.html"),
                503,
                CF_PLAIN,
                JS,
                cloudflare,
            ),
            case(
                "cloudflare_turnstile_challenge",
                fixture!("cloudflare_turnstile_challenge.html"),
                403,
                CF_PLAIN,
                ChallengeKind::Interactive,
                cloudflare,
            ),
            case(
                "cloudflare_blocked",
                fixture!("cloudflare_blocked.html"),
                403,
                CF_PLAIN,
                ChallengeKind::Blocked,
                cloudflare,
            ),
            case(
                "cloudflare_normal_page",
                fixture!("cloudflare_normal_page.html"),
                200,
                CF_PLAIN,
                NONE,
                cloudflare,
            ),
        ]
    }

    /// The fixtures of the other vendors' challenge and block pages.
    fn vendor_cases() -> Vec<Case> {
        vec![
            case(
                "datadome_captcha",
                fixture!("datadome_captcha.html"),
                403,
                &[("x-datadome", "protected")],
                ChallengeKind::Interactive,
                Some(Vendor::DataDome),
            ),
            case(
                "perimeterx_block",
                fixture!("perimeterx_block.html"),
                403,
                &[],
                ChallengeKind::Interactive,
                Some(Vendor::PerimeterX),
            ),
            case(
                "akamai_bm_verify",
                fixture!("akamai_bm_verify.html"),
                200,
                &[("server", "AkamaiGHost")],
                JS,
                Some(Vendor::Akamai),
            ),
            case(
                "akamai_access_denied",
                fixture!("akamai_access_denied.html"),
                403,
                &[("Server", "AkamaiGHost")],
                ChallengeKind::Blocked,
                Some(Vendor::Akamai),
            ),
            case(
                "imperva_incident",
                fixture!("imperva_incident.html"),
                403,
                &[("x-iinfo", "12-34-56 NNNN CT(0 0 0) RT(1 2)")],
                ChallengeKind::Blocked,
                Some(Vendor::Imperva),
            ),
            case(
                "imperva_pardon",
                fixture!("imperva_pardon.html"),
                200,
                &[],
                JS,
                Some(Vendor::Imperva),
            ),
        ]
    }

    /// Kasada and AWS WAF, a rate limit, and the pages that are not challenges.
    fn other_cases() -> Vec<Case> {
        vec![
            case(
                "kasada_challenge",
                fixture!("kasada_challenge.html"),
                429,
                &[],
                JS,
                Some(Vendor::Kasada),
            ),
            case(
                "aws_waf_challenge",
                fixture!("aws_waf_challenge.html"),
                202,
                &[("x-amzn-waf-action", "challenge")],
                JS,
                Some(Vendor::AwsWaf),
            ),
            case(
                "aws_waf_captcha",
                fixture!("aws_waf_captcha.html"),
                405,
                &[("x-amzn-waf-action", "captcha")],
                ChallengeKind::Interactive,
                Some(Vendor::AwsWaf),
            ),
            case(
                "rate_limited",
                fixture!("rate_limited.html"),
                429,
                &[("Retry-After", "30")],
                ChallengeKind::RateLimited {
                    retry_after_secs: Some(30),
                },
                None,
            ),
            case(
                "turnstile_login_form",
                fixture!("turnstile_login_form.html"),
                200,
                &[],
                NONE,
                None,
            ),
            case(
                "kasada_sdk_on_normal_page",
                fixture!("kasada_sdk_on_normal_page.html"),
                200,
                &[],
                NONE,
                None,
            ),
            case(
                "plain_article",
                fixture!("plain_article.html"),
                200,
                &[],
                NONE,
                None,
            ),
        ]
    }

    /// Every fixture, with the response it was served in and the verdict it must get.
    fn cases() -> Vec<Case> {
        let mut all = cloudflare_cases();
        all.extend(vendor_cases());
        all.extend(other_cases());
        all
    }

    fn owned(headers: &[(&str, &str)]) -> Vec<(String, String)> {
        headers
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    fn detect_with(body: &str, status: u16, headers: &[(&str, &str)]) -> Detection {
        let headers = owned(headers);
        detect(&ResponseView {
            url: "https://example.test/",
            status,
            headers: &headers,
            body,
        })
    }

    #[test]
    fn every_fixture_gets_its_kind_and_vendor() {
        for case in cases() {
            let found = detect_with(case.body, case.status, case.headers);
            assert_eq!(found.kind, case.kind, "{}: {:?}", case.name, found.evidence);
            assert_eq!(
                found.evidence.vendor, case.vendor,
                "{}: {:?}",
                case.name, found.evidence
            );
        }
    }

    #[test]
    fn every_marker_is_proven_by_a_fixture() {
        let mut seen: Vec<&'static str> = Vec::new();
        for case in cases() {
            seen.extend(
                detect_with(case.body, case.status, case.headers)
                    .evidence
                    .markers,
            );
        }
        for marker in &MARKERS {
            assert!(
                seen.contains(&marker.text),
                "no fixture matches {:?}",
                marker.text
            );
        }
        for marker in &HEADER_MARKERS {
            assert!(
                seen.contains(&marker.label),
                "no fixture sets {:?}",
                marker.label
            );
        }
    }

    #[test]
    fn every_marker_is_lower_case_and_listed_once() {
        let mut texts: Vec<&str> = MARKERS.iter().map(|m| m.text).collect();
        for text in &texts {
            assert_eq!(
                *text,
                text.to_ascii_lowercase(),
                "{text} must be lower case"
            );
            assert!(text.len() >= 5, "{text} is too short to be a marker");
        }
        texts.sort_unstable();
        let total = texts.len();
        texts.dedup();
        assert_eq!(texts.len(), total, "a marker is listed twice");
    }

    #[test]
    fn a_bare_cf_ray_is_not_a_challenge() {
        let found = detect_with("", 200, &[("cf-ray", "8a1b2c3d4e5f6071-AMS")]);
        assert_eq!(found.kind, ChallengeKind::None);
        assert_eq!(found.evidence.vendor, Some(Vendor::Cloudflare));
        assert_eq!(found.evidence.markers, ["cf-ray"]);
        assert_eq!(found.evidence.widget, None);
    }

    #[test]
    fn a_cloudflare_page_with_its_beacon_script_is_not_a_challenge() {
        let found = detect_with(fixture!("cloudflare_normal_page.html"), 200, CF_PLAIN);
        assert_eq!(found.kind, ChallengeKind::None);
        assert_eq!(found.evidence.markers, ["cf-ray", "server: cloudflare"]);
    }

    #[test]
    fn a_bare_turnstile_widget_is_not_a_challenge() {
        let found = detect_with(fixture!("turnstile_login_form.html"), 200, &[]);
        assert_eq!(found.kind, ChallengeKind::None);
        assert_eq!(found.evidence.widget, Some(Captcha::Turnstile));
        assert_eq!(found.evidence.markers, Vec::<&str>::new());
        assert_eq!(found.evidence.vendor, None);
    }

    #[test]
    fn a_forbidden_status_alone_is_not_a_challenge() {
        for status in [401, 403, 404, 500, 503] {
            let found = detect_with("Forbidden", status, &[("server", "cloudflare")]);
            assert_eq!(found.kind, ChallengeKind::None, "{status}");
            assert_eq!(found.evidence.vendor, Some(Vendor::Cloudflare));
        }
    }

    #[test]
    fn words_in_an_article_are_not_markers() {
        let found = detect_with(fixture!("plain_article.html"), 200, &[]);
        assert_eq!(found.kind, ChallengeKind::None);
        assert_eq!(found.evidence.markers, Vec::<&str>::new());
    }

    #[test]
    fn a_vendor_sdk_on_a_normal_page_is_not_a_challenge() {
        let normal = detect_with(fixture!("kasada_sdk_on_normal_page.html"), 200, &[]);
        assert_eq!(normal.kind, ChallengeKind::None);
        // The same text in a 429 is Kasada's challenge, not a plain rate limit.
        let challenge = detect_with(fixture!("kasada_sdk_on_normal_page.html"), 429, &[]);
        assert_eq!(challenge.kind, ChallengeKind::JsChallenge);
        assert_eq!(challenge.evidence.vendor, Some(Vendor::Kasada));
    }

    #[test]
    fn a_header_marker_decides_without_any_body() {
        let found = detect_with("", 403, &[("CF-Mitigated", " Challenge ")]);
        assert_eq!(found.kind, ChallengeKind::JsChallenge);
        assert_eq!(found.evidence.markers, ["cf-mitigated: challenge"]);
        let other = detect_with("", 403, &[("cf-mitigated", "block")]);
        assert_eq!(other.kind, ChallengeKind::None);
    }

    #[test]
    fn markers_and_headers_match_in_any_case() {
        let body = "<HTML><TITLE>JUST A MOMENT...</TITLE></HTML>";
        let found = detect_with(body, 503, &[("SERVER", "CloudFlare")]);
        assert_eq!(found.kind, ChallengeKind::JsChallenge);
        assert_eq!(
            found.evidence.markers,
            ["<title>just a moment", "server: cloudflare"]
        );
        let waf = detect_with("", 202, &[("X-Amzn-Waf-Action", "CHALLENGE")]);
        assert_eq!(waf.kind, ChallengeKind::JsChallenge);
    }

    #[test]
    fn a_repeated_header_is_read_on_every_line() {
        let found = detect_with(
            "",
            403,
            &[("cf-mitigated", "block"), ("cf-mitigated", "challenge")],
        );
        assert_eq!(found.kind, ChallengeKind::JsChallenge);
    }

    #[test]
    fn a_widget_turns_a_script_challenge_or_a_block_into_an_interactive_one() {
        let found = detect_with(fixture!("cloudflare_turnstile_challenge.html"), 403, &[]);
        assert_eq!(found.kind, ChallengeKind::Interactive);
        assert_eq!(found.evidence.widget, Some(Captcha::Turnstile));
        let block = format!(
            "{}<div class=\"g-recaptcha\" data-sitekey=\"x\"></div>",
            fixture!("cloudflare_blocked.html")
        );
        let found = detect_with(&block, 403, &[]);
        assert_eq!(found.kind, ChallengeKind::Interactive);
        assert_eq!(found.evidence.widget, Some(Captcha::Recaptcha));
    }

    #[test]
    fn the_strongest_marker_names_the_vendor() {
        let body = format!(
            "{}{}",
            fixture!("imperva_incident.html"),
            fixture!("cloudflare_managed.html")
        );
        let found = detect_with(&body, 403, &[]);
        assert_eq!(found.kind, ChallengeKind::JsChallenge);
        assert_eq!(found.evidence.vendor, Some(Vendor::Cloudflare));
        assert!(found.evidence.markers.contains(&"incapsula incident id"));
    }

    #[test]
    fn a_challenge_wins_over_a_rate_limit_status() {
        let found = detect_with(
            fixture!("cloudflare_managed.html"),
            429,
            &[("Retry-After", "5")],
        );
        assert_eq!(found.kind, ChallengeKind::JsChallenge);
    }

    #[test]
    fn a_rate_limit_reads_only_a_plain_number_of_seconds() {
        let wait = |value: &'static str| match detect_with("", 429, &[("retry-after", value)]).kind
        {
            ChallengeKind::RateLimited { retry_after_secs } => retry_after_secs,
            other => panic!("expected a rate limit, got {other:?}"),
        };
        assert_eq!(wait("30"), Some(30));
        assert_eq!(wait(" 120 "), Some(120));
        assert_eq!(wait("0"), Some(0));
        for bad in [
            "",
            "-1",
            "+5",
            "abc",
            "1.5",
            "1e3",
            "Wed, 21 Oct 2026 07:28:00 GMT",
            "99999999999999999999999",
        ] {
            assert_eq!(wait(bad), None, "{bad:?}");
        }
        let missing = detect_with("", 429, &[]);
        assert_eq!(
            missing.kind,
            ChallengeKind::RateLimited {
                retry_after_secs: None
            }
        );
    }

    #[test]
    fn retry_after_means_nothing_without_a_429() {
        let found = detect_with("", 200, &[("Retry-After", "30")]);
        assert_eq!(found.kind, ChallengeKind::None);
        let unavailable = detect_with("", 503, &[("Retry-After", "30")]);
        assert_eq!(unavailable.kind, ChallengeKind::None);
    }

    #[test]
    fn a_rate_limit_keeps_the_vendor_evidence() {
        let found = detect_with("", 429, &[("cf-ray", "x"), ("Retry-After", "9")]);
        assert_eq!(found.evidence.vendor, Some(Vendor::Cloudflare));
        assert_eq!(
            found.kind,
            ChallengeKind::RateLimited {
                retry_after_secs: Some(9)
            }
        );
    }

    #[test]
    fn the_status_gate_admits_only_its_listed_statuses() {
        assert!(Gate::Any.allows(0) && Gate::Any.allows(599));
        let gate = Gate::Statuses(&[202, 405]);
        assert!(gate.allows(202) && gate.allows(405));
        assert!(!gate.allows(200) && !gate.allows(403));
        let marker: &Marker = &MARKERS[0];
        assert!(
            marker.gate.allows(200),
            "a Cloudflare marker counts at any status"
        );
    }

    #[test]
    fn a_marker_past_the_scan_window_is_ignored() {
        let marker = "cf_chl_opt";
        let after = format!("{}{marker}", "a".repeat(MAX_SCAN_BYTES));
        assert_eq!(detect_with(&after, 403, &[]).kind, ChallengeKind::None);
        let before = format!("{marker}{}", "a".repeat(MAX_SCAN_BYTES * 4));
        assert_eq!(
            detect_with(&before, 403, &[]).kind,
            ChallengeKind::JsChallenge
        );
        let last_fit = format!("{}{marker}", "a".repeat(MAX_SCAN_BYTES - marker.len()));
        assert_eq!(
            detect_with(&last_fit, 403, &[]).kind,
            ChallengeKind::JsChallenge
        );
        let straddling = format!(
            "{}cf_chl{}_opt",
            "a".repeat(MAX_SCAN_BYTES - 6),
            "a".repeat(10)
        );
        assert_eq!(detect_with(&straddling, 403, &[]).kind, ChallengeKind::None);
    }

    #[test]
    fn a_widget_past_the_scan_window_is_ignored_too() {
        let body = format!("{}cf-turnstile", "a".repeat(MAX_SCAN_BYTES));
        assert_eq!(detect_with(&body, 200, &[]).evidence.widget, None);
    }

    #[test]
    fn the_window_never_cuts_a_character_in_two() {
        // One two-byte character straddles the limit, in every alignment.
        for lead in [
            MAX_SCAN_BYTES - 3,
            MAX_SCAN_BYTES - 2,
            MAX_SCAN_BYTES - 1,
            MAX_SCAN_BYTES,
        ] {
            let body = format!("{}é€😀cf_chl_opt", "a".repeat(lead));
            let found = detect_with(&body, 403, &[]);
            assert_eq!(found.kind, ChallengeKind::None, "lead {lead}");
        }
        let body = format!("{}é", "a".repeat(MAX_SCAN_BYTES - 1));
        assert_eq!(scan_window(&body).len(), MAX_SCAN_BYTES - 1);
    }

    #[test]
    fn a_huge_body_is_scanned_in_bounded_time_and_memory() {
        let body = "x".repeat(16 * 1024 * 1024);
        let found = detect_with(&body, 200, &[]);
        assert_eq!(found.kind, ChallengeKind::None);
    }

    #[test]
    fn binary_junk_and_control_characters_never_panic_or_match() {
        let mut rng = Rng::new(0x00C0_FFEE);
        for _ in 0..64 {
            let bytes: Vec<u8> = (0..4096).map(|_| rng.next_u64().to_le_bytes()[0]).collect();
            let junk = String::from_utf8_lossy(&bytes).into_owned();
            let found = detect_with(&junk, 200, &[]);
            assert_eq!(found.kind, ChallengeKind::None);
            assert_eq!(found.evidence.markers, Vec::<&str>::new());
        }
        let nul = "\u{0}".repeat(2 * MAX_SCAN_BYTES);
        assert_eq!(detect_with(&nul, 403, &[]).kind, ChallengeKind::None);
        let controls = "\u{1}\u{7}\u{8}\u{1b}[0m\u{feff}\u{202e}cf_chl_opt";
        assert_eq!(
            detect_with(controls, 403, &[]).kind,
            ChallengeKind::JsChallenge
        );
    }

    #[test]
    fn hostile_headers_never_panic() {
        let weird: Vec<(String, String)> = vec![
            (String::new(), String::new()),
            ("cf-ray".to_owned(), "\u{0}".to_owned()),
            ("SERVER".to_owned(), "é".repeat(100_000)),
            ("retry-after".to_owned(), "9".repeat(100_000)),
            ("cf-mitigated\n".to_owned(), "challenge".to_owned()),
        ];
        let found = detect(&ResponseView {
            url: "",
            status: 429,
            headers: &weird,
            body: "",
        });
        assert_eq!(
            found.kind,
            ChallengeKind::RateLimited {
                retry_after_secs: None
            }
        );
        assert_eq!(found.evidence.vendor, Some(Vendor::Cloudflare));
        let many: Vec<(String, String)> = (0..20_000)
            .map(|i| (format!("x-h-{i}"), "v".to_owned()))
            .collect();
        let found = detect(&ResponseView {
            url: "",
            status: 200,
            headers: &many,
            body: "",
        });
        assert_eq!(found.kind, ChallengeKind::None);
    }

    #[test]
    fn detection_is_a_pure_function_of_the_response() {
        for case in cases() {
            let first = detect_with(case.body, case.status, case.headers);
            assert_eq!(
                first,
                detect_with(case.body, case.status, case.headers),
                "{}",
                case.name
            );
        }
    }

    #[test]
    fn each_vendor_has_the_cookie_that_proves_clearance() {
        let table = [
            (Vendor::Cloudflare, Some("cf_clearance")),
            (Vendor::DataDome, Some("datadome")),
            (Vendor::PerimeterX, Some("_px3")),
            (Vendor::Akamai, Some("_abck")),
            (Vendor::AwsWaf, Some("aws-waf-token")),
            (Vendor::Imperva, Some("reese84")),
            (Vendor::Kasada, None),
        ];
        for (vendor, cookie) in table {
            assert_eq!(clearance_cookie(vendor), cookie, "{vendor:?}");
        }
    }

    #[test]
    fn kinds_and_vendors_survive_serde() {
        let kinds = [
            ChallengeKind::None,
            ChallengeKind::JsChallenge,
            ChallengeKind::Interactive,
            ChallengeKind::Blocked,
            ChallengeKind::RateLimited {
                retry_after_secs: Some(7),
            },
            ChallengeKind::RateLimited {
                retry_after_secs: None,
            },
        ];
        for kind in kinds {
            let json = serde_json::to_string(&kind).expect("serializes");
            assert_eq!(
                serde_json::from_str::<ChallengeKind>(&json).expect("parses"),
                kind
            );
        }
        let json = serde_json::to_string(&Vendor::AwsWaf).expect("serializes");
        assert_eq!(
            serde_json::from_str::<Vendor>(&json).expect("parses"),
            Vendor::AwsWaf
        );
    }
}
