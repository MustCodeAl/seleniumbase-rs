//! The interstitial markers that prove a challenge page, one named constant each, and the weaker header and widget evidence that never decides on its own.
//!
//! Every body marker is lower case, because the scan lower-cases the body first, and every one
//! names the fixture under `tests/fixtures/challenge` that proves it. The fixtures are small
//! hand-written reproductions of each vendor's page, not captures of a live site. A marker
//! without a fixture is refused by the tests.
//!
//! What is not a marker: a vendor header such as `cf-ray`, a vendor script that every protected
//! page loads (`/cdn-cgi/challenge-platform/scripts/jsd/main.js`, Kasada's `p.js`), and a CAPTCHA
//! widget a login form embeds. Those are evidence ([`HEADER_HINTS`], [`WIDGETS`]), never a
//! verdict.

use crate::sb_cdp::Captcha;

use super::detect::{ChallengeKind, Vendor};

/// Cloudflare's interstitial title. Fixtures: `cloudflare_managed.html`,
/// `cloudflare_legacy_js.html`, `cloudflare_turnstile_challenge.html`.
pub const CLOUDFLARE_TITLE: &str = "<title>just a moment";
/// Cloudflare's challenge options object, on every interstitial. Fixtures:
/// `cloudflare_managed.html`, `cloudflare_turnstile_challenge.html`.
pub const CLOUDFLARE_CHL_OPT: &str = "cf_chl_opt";
/// Cloudflare's challenge orchestration script, `b` build. Fixture: `cloudflare_managed.html`.
pub const CLOUDFLARE_ORCHESTRATE_B: &str = "/cdn-cgi/challenge-platform/h/b/orchestrate";
/// Cloudflare's challenge orchestration script, `g` build. Fixture:
/// `cloudflare_turnstile_challenge.html`.
pub const CLOUDFLARE_ORCHESTRATE_G: &str = "/cdn-cgi/challenge-platform/h/g/orchestrate";
/// The form of Cloudflare's older JavaScript challenge. Fixture: `cloudflare_legacy_js.html`.
pub const CLOUDFLARE_CHALLENGE_FORM: &str = "id=\"challenge-form\"";
/// The stage of Cloudflare's older JavaScript challenge. Fixture: `cloudflare_legacy_js.html`.
pub const CLOUDFLARE_CHALLENGE_STAGE: &str = "id=\"challenge-stage\"";
/// The title of Cloudflare's block page. Fixture: `cloudflare_blocked.html`.
pub const CLOUDFLARE_BLOCK_TITLE: &str = "<title>attention required! | cloudflare";
/// `DataDome`'s CAPTCHA host, in the frame and script of its challenge. Fixture:
/// `datadome_captcha.html`.
pub const DATADOME_HOST: &str = "captcha-delivery.com";
/// `PerimeterX`'s CAPTCHA container, which only the block page carries. Fixture:
/// `perimeterx_block.html`.
pub const PERIMETERX_CAPTCHA: &str = "id=\"px-captcha\"";
/// The token Akamai Bot Manager's verification page redirects with. Fixture:
/// `akamai_bm_verify.html`.
pub const AKAMAI_BM_VERIFY: &str = "bm-verify=";
/// The host in the reference link of Akamai's block page, matched in the 403 only, because the
/// same host serves ordinary assets. Fixture: `akamai_access_denied.html`.
pub const AKAMAI_BLOCK_HOST: &str = "edgesuite";
/// The text of Imperva's block page. Fixture: `imperva_incident.html`.
pub const IMPERVA_INCIDENT: &str = "incapsula incident id";
/// The title of Imperva's bot interstitial. Fixture: `imperva_pardon.html`.
pub const IMPERVA_PARDON_TITLE: &str = "<title>pardon our interruption";
/// Kasada's client object, matched in the 429 challenge only, because every protected page
/// loads the SDK. Fixture: `kasada_challenge.html`; the negative is
/// `kasada_sdk_on_normal_page.html`.
pub const KASADA_SDK: &str = "kpsdk";
/// AWS WAF's challenge script object, matched in the 202 and 405 answers only. Fixture:
/// `aws_waf_challenge.html`.
pub const AWS_WAF_INTEGRATION: &str = "awswafintegration";
/// AWS WAF's CAPTCHA script object, matched in the 202 and 405 answers only. Fixture:
/// `aws_waf_captcha.html`.
pub const AWS_WAF_CAPTCHA: &str = "awswafcaptcha";

/// The statuses AWS WAF answers a challenge with (202) and a CAPTCHA with (405).
const AWS_WAF_STATUSES: &[u16] = &[202, 405];
/// The status Kasada answers a challenge with.
const KASADA_STATUSES: &[u16] = &[429];
/// The status Akamai answers a block with.
const AKAMAI_BLOCK_STATUSES: &[u16] = &[403];

/// The statuses on which a marker counts.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Gate {
    /// Any status: the marker appears on challenge pages only.
    Any,
    /// Only these statuses: the marker also appears on pages that are not challenges.
    Statuses(&'static [u16]),
}

impl Gate {
    /// Whether a marker with this gate counts in a response with `status`.
    pub(crate) fn allows(self, status: u16) -> bool {
        match self {
            Self::Any => true,
            Self::Statuses(statuses) => statuses.contains(&status),
        }
    }
}

/// A text marker: a response containing `text` while `gate` allows its status is of `vendor` and
/// asks for `kind`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Marker {
    pub(crate) vendor: Vendor,
    pub(crate) text: &'static str,
    pub(crate) kind: ChallengeKind,
    pub(crate) gate: Gate,
}

impl Marker {
    const fn new(vendor: Vendor, text: &'static str, kind: ChallengeKind, gate: Gate) -> Self {
        Self {
            vendor,
            text,
            kind,
            gate,
        }
    }
}

/// Every body marker. The scan takes the strongest kind among the markers it finds.
pub(crate) const MARKERS: [Marker; 16] = [
    Marker::new(
        Vendor::Cloudflare,
        CLOUDFLARE_TITLE,
        ChallengeKind::JsChallenge,
        Gate::Any,
    ),
    Marker::new(
        Vendor::Cloudflare,
        CLOUDFLARE_CHL_OPT,
        ChallengeKind::JsChallenge,
        Gate::Any,
    ),
    Marker::new(
        Vendor::Cloudflare,
        CLOUDFLARE_ORCHESTRATE_B,
        ChallengeKind::JsChallenge,
        Gate::Any,
    ),
    Marker::new(
        Vendor::Cloudflare,
        CLOUDFLARE_ORCHESTRATE_G,
        ChallengeKind::JsChallenge,
        Gate::Any,
    ),
    Marker::new(
        Vendor::Cloudflare,
        CLOUDFLARE_CHALLENGE_FORM,
        ChallengeKind::JsChallenge,
        Gate::Any,
    ),
    Marker::new(
        Vendor::Cloudflare,
        CLOUDFLARE_CHALLENGE_STAGE,
        ChallengeKind::JsChallenge,
        Gate::Any,
    ),
    Marker::new(
        Vendor::Cloudflare,
        CLOUDFLARE_BLOCK_TITLE,
        ChallengeKind::Blocked,
        Gate::Any,
    ),
    Marker::new(
        Vendor::DataDome,
        DATADOME_HOST,
        ChallengeKind::Interactive,
        Gate::Any,
    ),
    Marker::new(
        Vendor::PerimeterX,
        PERIMETERX_CAPTCHA,
        ChallengeKind::Interactive,
        Gate::Any,
    ),
    Marker::new(
        Vendor::Akamai,
        AKAMAI_BM_VERIFY,
        ChallengeKind::JsChallenge,
        Gate::Any,
    ),
    Marker::new(
        Vendor::Akamai,
        AKAMAI_BLOCK_HOST,
        ChallengeKind::Blocked,
        Gate::Statuses(AKAMAI_BLOCK_STATUSES),
    ),
    Marker::new(
        Vendor::Imperva,
        IMPERVA_INCIDENT,
        ChallengeKind::Blocked,
        Gate::Any,
    ),
    Marker::new(
        Vendor::Imperva,
        IMPERVA_PARDON_TITLE,
        ChallengeKind::JsChallenge,
        Gate::Any,
    ),
    Marker::new(
        Vendor::Kasada,
        KASADA_SDK,
        ChallengeKind::JsChallenge,
        Gate::Statuses(KASADA_STATUSES),
    ),
    Marker::new(
        Vendor::AwsWaf,
        AWS_WAF_INTEGRATION,
        ChallengeKind::JsChallenge,
        Gate::Statuses(AWS_WAF_STATUSES),
    ),
    Marker::new(
        Vendor::AwsWaf,
        AWS_WAF_CAPTCHA,
        ChallengeKind::Interactive,
        Gate::Statuses(AWS_WAF_STATUSES),
    ),
];

/// A response header that marks a challenge when it has this value (compared without case).
#[derive(Clone, Copy, Debug)]
pub(crate) struct HeaderMarker {
    pub(crate) name: &'static str,
    pub(crate) value: &'static str,
    pub(crate) vendor: Vendor,
    pub(crate) kind: ChallengeKind,
    /// How the header is named in [`super::detect::Evidence::markers`].
    pub(crate) label: &'static str,
}

/// Every header marker. `cf-mitigated: challenge` is on every Cloudflare interstitial (fixture
/// `cloudflare_managed.html`); `x-amzn-waf-action` names what AWS WAF wants (fixtures
/// `aws_waf_challenge.html` and `aws_waf_captcha.html`).
pub(crate) const HEADER_MARKERS: [HeaderMarker; 3] = [
    HeaderMarker {
        name: "cf-mitigated",
        value: "challenge",
        vendor: Vendor::Cloudflare,
        kind: ChallengeKind::JsChallenge,
        label: "cf-mitigated: challenge",
    },
    HeaderMarker {
        name: "x-amzn-waf-action",
        value: "challenge",
        vendor: Vendor::AwsWaf,
        kind: ChallengeKind::JsChallenge,
        label: "x-amzn-waf-action: challenge",
    },
    HeaderMarker {
        name: "x-amzn-waf-action",
        value: "captcha",
        vendor: Vendor::AwsWaf,
        kind: ChallengeKind::Interactive,
        label: "x-amzn-waf-action: captcha",
    },
];

/// A response header that only says which vendor fronts the site. It is evidence, never a
/// verdict: a page served through Cloudflare is not a challenge because `cf-ray` is there.
#[derive(Clone, Copy, Debug)]
pub(crate) struct HeaderHint {
    pub(crate) name: &'static str,
    /// What the lower-cased value must contain; empty matches any value.
    pub(crate) contains: &'static str,
    pub(crate) vendor: Vendor,
    pub(crate) label: &'static str,
}

/// Every vendor header hint.
pub(crate) const HEADER_HINTS: [HeaderHint; 6] = [
    HeaderHint {
        name: "cf-ray",
        contains: "",
        vendor: Vendor::Cloudflare,
        label: "cf-ray",
    },
    HeaderHint {
        name: "server",
        contains: "cloudflare",
        vendor: Vendor::Cloudflare,
        label: "server: cloudflare",
    },
    HeaderHint {
        name: "x-datadome",
        contains: "",
        vendor: Vendor::DataDome,
        label: "x-datadome",
    },
    HeaderHint {
        name: "x-iinfo",
        contains: "",
        vendor: Vendor::Imperva,
        label: "x-iinfo",
    },
    HeaderHint {
        name: "server",
        contains: "akamaighost",
        vendor: Vendor::Akamai,
        label: "server: akamaighost",
    },
    HeaderHint {
        name: "x-amzn-waf-action",
        contains: "",
        vendor: Vendor::AwsWaf,
        label: "x-amzn-waf-action",
    },
];

/// What marks each CAPTCHA widget in the page source, lower case. A widget is evidence that a
/// solver could act; a login form with a Turnstile box is not a challenge.
pub(crate) const WIDGETS: [(Captcha, &[&str]); 5] = [
    (
        Captcha::Turnstile,
        &["challenges.cloudflare.com/turnstile", "cf-turnstile"],
    ),
    (
        Captcha::Recaptcha,
        &[
            "g-recaptcha",
            "google.com/recaptcha/",
            "recaptcha.net/recaptcha/",
        ],
    ),
    (
        Captcha::Hcaptcha,
        &["hcaptcha.com/1/api.js", "h-captcha", "hcaptcha.com/captcha"],
    ),
    (
        Captcha::FriendlyCaptcha,
        &["friendlycaptcha", "frc-captcha"],
    ),
    (Captcha::DataDome, &[DATADOME_HOST]),
];
