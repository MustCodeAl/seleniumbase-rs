---
title: "scrape::passport"
sidebarTitle: "Identity"
description: "StealthIdentity, the session policy and the emulation reference."
---

<Badge color="orange">Experimental</Badge> <Badge color="blue">Crate: scrape</Badge>

`passport` is the identity a scrape carries between the browser and plain HTTP: the fingerprint, the cookies, the clearances and the emulation it claims. `SessionPolicy` says when that identity should stay on HTTP and when it should go to the browser.

See [Identity and cookies](/stealth/identity).

<RequestExample>

```rust The emulation an identity claims
use seleniumbase_rs_stealth::codec::BrowserKind;
use seleniumbase_rs_stealth::evasion::fingerprint::OsType;
use seleniumbase_rs_stealth::scrape::EmulationRef;

fn chrome_on_macos(major: u32) -> EmulationRef {
    EmulationRef {
        kind: BrowserKind::Chrome,
        major,
        os: OsType::Macos,
    }
}
```

</RequestExample>

## Constants and type aliases

```rust
// passport/identity.rs
pub const IDENTITY_SCHEMA: u32 = 1;
pub const NO_EXPIRY: u64 = u64::MAX;
// passport/policy.rs
pub const DEFAULT_CLEARANCE_MARGIN_SECS: u64 = 120;
```


- `IDENTITY_SCHEMA`: The version of the identity layout. A file with another version is refused, not guessed at.
- `NO_EXPIRY`: The expiry recorded for a clearance cookie that has none (a session cookie): no known end.
- `DEFAULT_CLEARANCE_MARGIN_SECS`: How close to its end a clearance may get before an HTTP session goes back to the browser.

## `passport::identity`

### `EmulationRef`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct EmulationRef {
    pub kind: BrowserKind,
    pub major: u32,
    pub os: OsType,
}
```

The measured browser whose TLS, HTTP/2 and headers an identity presents.

### `Clearance`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct Clearance {
    pub site: String,
    pub vendor: Vendor,
    pub cookie: String,
    pub expires_at: u64,
}
```

Proof that a site let the identity through: the vendor's clearance cookie is in the jar.

### `StealthIdentity`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct StealthIdentity {
    pub schema: u32,
    pub fingerprint: Fingerprint,
    pub emulation: EmulationRef,
    pub cookies: CookieJar,
    pub accept_ch: BTreeMap<String, Vec<String>>,
    pub egress: Option<EgressKey>,
    pub clearances: Vec<Clearance>,
    pub created_at: u64,
}
```

A fingerprint, its emulation, and what the identity earned with them.

### `StealthIdentity` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn new(
        fingerprint: Fingerprint,
        emulation: EmulationRef,
        egress: Option<EgressKey>,
        now: u64,
    ) -> Result<Self, IdentityError>
pub fn verify(&self) -> Result<(), IdentityError>
pub fn refresh_clearances(&mut self, now: u64)
pub fn clearance_for(&self, site: &str) -> Option<&Clearance>
pub fn check_egress(&self, egress: Option<&EgressKey>) -> Result<(), IdentityError>
pub fn check_emulation(&self, measured: &EmulationRef) -> Result<(), IdentityError>
```


- `new`: Makes an identity from a fingerprint. See the module documentation for what it refuses.
- `verify`: Checks an identity that was read from outside: its schema, its fingerprint, and that it carries no proxy.
- `refresh_clearances`: Records a [`Clearance`] for every live clearance cookie in the jar, replacing the old list.
- `clearance_for`: The clearance for `site`, a registrable domain or any host on it. When several vendors cleared the site, the one that lasts longest. It may already be expired: compare [`Clearance::expires_at`] with the time.
- `check_egress`: Checks that a request leaves through the egress this identity belongs to.
- `check_emulation`: Checks that the browser measured now is the one the identity was made for.

### `IdentityError`

<Badge color="blue" shape="pill">enum</Badge>

```rust
pub enum IdentityError {
    EgressMismatch {
            /// The egress the identity is bound to.
            expected: Option<EgressKey>,
            /// The egress the request would leave through.
            found: Option<EgressKey>,
        },
    Schema(u32),
    Incoherent(Vec<String>),
    EmulationMismatch {
            /// The major version the identity claims.
            identity: u32,
            /// The major version that was measured.
            measured: u32,
        },
}
```

Why an identity was refused.

## `passport::policy`

### `SessionPolicy`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct SessionPolicy {
    pub require_clearance_for_http: bool,
    pub clearance_margin: Duration,
    pub demote_after_solve: bool,
    pub prefer: Mode,
}
```

When a session changes mode.

### `SessionPolicy` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn before_fetch(
        &self,
        mode: Mode,
        id: &StealthIdentity,
        site: &str,
        st: Option<&DomainState>,
        now: u64,
    ) -> Transition
pub fn after_fetch(
        &self,
        mode: Mode,
        d: &Detection,
        a: &Action,
        id: &StealthIdentity,
    ) -> Transition
```


- `before_fetch`: Whether to change mode before fetching `site`. See the module documentation for the table.
- `after_fetch`: Whether to change mode after the action chosen for `d` has been carried out. See the module documentation for the table.

### `Transition`

<Badge color="blue" shape="pill">enum</Badge>

```rust
pub enum Transition {
    Stay,
    Escalate(EscalateReason),
    Demote(DemoteReason),
}
```

A change of mode, or none.

### `Transition` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn resulting_mode(self, current: Mode) -> Mode
```


- `resulting_mode`: The mode a session in `current` is in after this transition.

### `EscalateReason`

<Badge color="blue" shape="pill">enum</Badge>

```rust
pub enum EscalateReason {
    Challenge(ChallengeKind),
    ClearanceExpiring {
            /// Seconds left; zero once it has ended.
            remaining_secs: u64,
        },
    NoClearance,
    SiteNeedsBrowser,
    Manual,
}
```

Why a session goes to the browser.

### `DemoteReason`

<Badge color="blue" shape="pill">enum</Badge>

```rust
pub enum DemoteReason {
    ChallengeSolved,
    Manual,
}
```

Why a session goes back to HTTP.

## Related

The guide is [Identity and cookies](/stealth/identity). The overview is [scrape](/stealth/api/scrape).
