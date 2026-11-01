---
title: "scrape::challenge"
sidebarTitle: "Challenges"
description: "Challenge detection, interstitial markers and the mitigation policy."
---

<Badge color="orange">Experimental</Badge> <Badge color="blue">Crate: scrape</Badge>

`challenge` decides what a response is and what to do next. `detect` reads a `ResponseView` and returns a `Detection`. `markers` holds the interstitial strings. `mitigation` turns a detection and an attempt into an `Action`.

`BrowserScraper` calls this on every load. See [BrowserScraper](/stealth/browser-scraper).

<RequestExample>

```rust Classify a response
use seleniumbase_rs_stealth::scrape::challenge::detect::{detect, ResponseView};

fn main() {
    let headers: [(String, String); 0] = [];
    let found = detect(&ResponseView {
        url: "https://example.com/",
        status: 200,
        headers: &headers,
        body: "hello",
    });
    println!("{:?}", found.kind);
}
```

</RequestExample>

## Constants and type aliases

```rust
// challenge/detect.rs
pub const MAX_SCAN_BYTES: usize = 256 * 1024;
// challenge/markers.rs
pub const CLOUDFLARE_TITLE: &str = "<title>just a moment";
pub const CLOUDFLARE_CHL_OPT: &str = "cf_chl_opt";
pub const CLOUDFLARE_ORCHESTRATE_B: &str = "/cdn-cgi/challenge-platform/h/b/orchestrate";
pub const CLOUDFLARE_ORCHESTRATE_G: &str = "/cdn-cgi/challenge-platform/h/g/orchestrate";
pub const CLOUDFLARE_CHALLENGE_FORM: &str = "id=\"challenge-form\"";
pub const CLOUDFLARE_CHALLENGE_STAGE: &str = "id=\"challenge-stage\"";
pub const CLOUDFLARE_BLOCK_TITLE: &str = "<title>attention required! | cloudflare";
pub const DATADOME_HOST: &str = "captcha-delivery.com";
pub const PERIMETERX_CAPTCHA: &str = "id=\"px-captcha\"";
pub const AKAMAI_BM_VERIFY: &str = "bm-verify=";
pub const AKAMAI_BLOCK_HOST: &str = "edgesuite";
pub const IMPERVA_INCIDENT: &str = "incapsula incident id";
pub const IMPERVA_PARDON_TITLE: &str = "<title>pardon our interruption";
pub const KASADA_SDK: &str = "kpsdk";
pub const AWS_WAF_INTEGRATION: &str = "awswafintegration";
pub const AWS_WAF_CAPTCHA: &str = "awswafcaptcha";
// challenge/mitigation.rs
pub const DEFAULT_MAX_CHALLENGE_ATTEMPTS: u32 = 3;
pub const DEFAULT_ROTATE_EGRESS_AFTER: u32 = 2;
pub const DEFAULT_ROTATE_PROFILE_AFTER: u32 = 3;
pub const DEFAULT_BLOCK_COOLDOWN_SECS: u64 = 900;
pub const DEFAULT_RATE_LIMIT_COOLDOWN_SECS: u64 = 60;
pub const MAX_RETRY_AFTER_SECS: u64 = 3600;
```


- `MAX_SCAN_BYTES`: How much of a body is scanned. Anything past this is ignored, so a huge response costs a bounded scan and a marker buried deep in it cannot be used as a verdict.
- `CLOUDFLARE_TITLE`: Cloudflare's interstitial title. Fixtures: `cloudflare_managed.html`, `cloudflare_legacy_js.html`, `cloudflare_turnstile_challenge.html`.
- `CLOUDFLARE_CHL_OPT`: Cloudflare's challenge options object, on every interstitial. Fixtures: `cloudflare_managed.html`, `cloudflare_turnstile_challenge.html`.
- `CLOUDFLARE_ORCHESTRATE_B`: Cloudflare's challenge orchestration script, `b` build. Fixture: `cloudflare_managed.html`.
- `CLOUDFLARE_ORCHESTRATE_G`: Cloudflare's challenge orchestration script, `g` build. Fixture: `cloudflare_turnstile_challenge.html`.
- `CLOUDFLARE_CHALLENGE_FORM`: The form of Cloudflare's older JavaScript challenge. Fixture: `cloudflare_legacy_js.html`.
- `CLOUDFLARE_CHALLENGE_STAGE`: The stage of Cloudflare's older JavaScript challenge. Fixture: `cloudflare_legacy_js.html`.
- `CLOUDFLARE_BLOCK_TITLE`: The title of Cloudflare's block page. Fixture: `cloudflare_blocked.html`.
- `DATADOME_HOST`: `DataDome`'s CAPTCHA host, in the frame and script of its challenge. Fixture: `datadome_captcha.html`.
- `PERIMETERX_CAPTCHA`: `PerimeterX`'s CAPTCHA container, which only the block page carries. Fixture: `perimeterx_block.html`.
- `AKAMAI_BM_VERIFY`: The token Akamai Bot Manager's verification page redirects with. Fixture: `akamai_bm_verify.html`.
- `AKAMAI_BLOCK_HOST`: The host in the reference link of Akamai's block page, matched in the 403 only, because the same host serves ordinary assets. Fixture: `akamai_access_denied.html`.
- `IMPERVA_INCIDENT`: The text of Imperva's block page. Fixture: `imperva_incident.html`.
- `IMPERVA_PARDON_TITLE`: The title of Imperva's bot interstitial. Fixture: `imperva_pardon.html`.
- `KASADA_SDK`: Kasada's client object, matched in the 429 challenge only, because every protected page loads the SDK. Fixture: `kasada_challenge.html`; the negative is `kasada_sdk_on_normal_page.html`.
- `AWS_WAF_INTEGRATION`: AWS WAF's challenge script object, matched in the 202 and 405 answers only. Fixture: `aws_waf_challenge.html`.
- `AWS_WAF_CAPTCHA`: AWS WAF's CAPTCHA script object, matched in the 202 and 405 answers only. Fixture: `aws_waf_captcha.html`.
- `DEFAULT_MAX_CHALLENGE_ATTEMPTS`: How many attempts at one URL the default policy makes before giving up.
- `DEFAULT_ROTATE_EGRESS_AFTER`: Challenges in a row after which the default policy changes the egress.
- `DEFAULT_ROTATE_PROFILE_AFTER`: Challenges in a row after which the default policy changes the browser profile.
- `DEFAULT_BLOCK_COOLDOWN_SECS`: Seconds the default policy leaves a site alone after an outright block.
- `DEFAULT_RATE_LIMIT_COOLDOWN_SECS`: Seconds to leave a site alone after a rate limit that named no `Retry-After`.
- `MAX_RETRY_AFTER_SECS`: The longest `Retry-After` honored, in seconds. A server cannot park a scraper for longer.

## `challenge::detect`

### `Vendor`

<Badge color="blue" shape="pill">enum</Badge>

```rust
pub enum Vendor {
    Cloudflare,
    DataDome,
    PerimeterX,
    Akamai,
    Imperva,
    Kasada,
    AwsWaf,
}
```

A bot-protection vendor.

### `ChallengeKind`

<Badge color="blue" shape="pill">enum</Badge>

```rust
pub enum ChallengeKind {
    None,
    JsChallenge,
    Interactive,
    Blocked,
    RateLimited {
            /// Seconds to wait, from a `Retry-After` header that is a plain number of seconds. A
            /// date in that header is not read.
            retry_after_secs: Option<u64>,
        },
}
```

What a response asks of the client.

### `Evidence`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct Evidence {
    pub vendor: Option<Vendor>,
    pub markers: Vec<&'static str>,
    pub widget: Option<Captcha>,
}
```

What the scan found, kept with the verdict so a caller can log why.

### `Detection`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct Detection {
    pub kind: ChallengeKind,
    pub evidence: Evidence,
}
```

A verdict and the evidence behind it.

### `ResponseView`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct ResponseView<'a> {
    pub url: &'a str,
    pub status: u16,
    pub headers: &'a [(String, String)],
    pub body: &'a str,
}
```

The parts of a response that detection reads.

## `challenge::mitigation`

### `Mode`

<Badge color="blue" shape="pill">enum</Badge>

```rust
pub enum Mode {
    Browser,
    Http,
}
```

How a request is being made.

### `AttemptContext`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct AttemptContext {
    pub attempt: u32,
    pub mode: Mode,
    pub consecutive_challenges: u32,
    pub can_rotate_egress: bool,
    pub can_escalate: bool,
    pub now: u64,
}
```

Where the fetch stands when a detection comes back.

### `Action`

<Badge color="blue" shape="pill">enum</Badge>

```rust
pub enum Action {
    Proceed,
    Retry {
            /// How long to wait first.
            after: Duration,
        },
    Escalate,
    Solve,
    RotateEgress,
    RotateProfile,
    Cooldown {
            /// When the site may be tried again.
            until: u64,
        },
    GiveUp(GiveUpReason),
}
```

What to do next.

### `GiveUpReason`

<Badge color="blue" shape="pill">enum</Badge>

```rust
pub enum GiveUpReason {
    AttemptsExhausted,
    Blocked,
    NoEgressLeft,
}
```

Why the policy stopped.

### `MitigationPolicy`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct MitigationPolicy {
    pub max_challenge_attempts: u32,
    pub backoff: RetryPolicy,
    pub rotate_egress_after: u32,
    pub rotate_profile_after: u32,
    pub block_cooldown_secs: u64,
}
```

The thresholds of the decision table.

### `MitigationPolicy` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn decide(&self, detection: &Detection, context: &AttemptContext) -> Action
```


- `decide`: Decides the next step for `detection` in `context`. See the module documentation for the table.

## Functions

### `detect`

<Badge color="green" shape="pill">function</Badge>

```rust
pub fn detect(view: &ResponseView<'_>) -> Detection
```

Decides what `view` asks of the client.

### `clearance_cookie`

<Badge color="green" shape="pill">function</Badge>

```rust
pub fn clearance_cookie(vendor: Vendor) -> Option<&'static str>
```

The cookie that proves a vendor's challenge was passed, once it is set.

## Related

The guide is [BrowserScraper](/stealth/browser-scraper). The overview is [scrape](/stealth/api/scrape).
