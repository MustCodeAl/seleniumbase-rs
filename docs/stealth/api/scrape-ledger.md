---
title: "scrape::ledger"
sidebarTitle: "Ledger"
description: "Per-site state, event sinks and the events a scraper reports."
---

<Badge color="orange">Experimental</Badge> <Badge color="blue">Crate: scrape</Badge>

`ledger` is the memory of a scrape. `state` keeps what each site has done behind `StateStore`. `events` is what the scraper reports, with URLs and proxies already redacted, behind `EventSink`.

See [BrowserScraper](/stealth/browser-scraper).

<RequestExample>

```rust The in-memory store
use seleniumbase_rs_stealth::scrape::InMemoryStateStore;

fn main() {
    let _store = InMemoryStateStore::new();
}
```

</RequestExample>

## Constants and type aliases

```rust
// ledger/events.rs
pub const MAX_COLLECTED_EVENTS: usize = 10_000;
// ledger/state.rs
pub const STATE_SCHEMA: u32 = 1;
pub const MAX_STATE_SITES: usize = 10_000;
```


- `MAX_COLLECTED_EVENTS`: The most events a [`CollectingEventSink`] keeps. Past it, the oldest is dropped for each new one.
- `STATE_SCHEMA`: The version of the state file layout. A file with another version is refused, not guessed at.
- `MAX_STATE_SITES`: How many sites a store keeps. A scrape can touch an unbounded number of sites, so a store that is full forgets the site it saw longest ago to make room for a new one.

## `ledger::events`

### `ScraperEvent`

<Badge color="blue" shape="pill">enum</Badge>

```rust
pub enum ScraperEvent {
    Request {
            /// The URL requested, redacted.
            url: RedactedUrl,
            /// The HTTP status of the answer.
            status: u16,
            /// How the request was made.
            mode: Mode,
            /// How long it took, in milliseconds.
            elapsed_ms: u64,
        },
    ChallengeDetected {
            /// The URL requested, redacted.
            url: RedactedUrl,
            /// What the answer asked for.
            kind: ChallengeKind,
            /// The vendor behind it, when known.
            vendor: Option<Vendor>,
            /// How the request was made.
            mode: Mode,
        },
    ChallengeSolved {
            /// The URL requested, redacted.
            url: RedactedUrl,
            /// What the challenge was.
            kind: ChallengeKind,
            /// How many attempts it took.
            attempts: u32,
            /// How long it took, in milliseconds.
            elapsed_ms: u64,
        },
    ActionChosen {
            /// The site the decision is about.
            site: String,
            /// What was chosen.
            action: Action,
        },
    ModeChanged {
            /// The mode it left.
            from: Mode,
            /// The mode it entered.
            to: Mode,
            /// Why.
            reason: Transition,
        },
    EgressRotated {
            /// The egress it left, `None` for the direct connection.
            from: Option<EgressKey>,
            /// The egress it entered, `None` for the direct connection.
            to: Option<EgressKey>,
        },
    ProfileRotated {
            /// The noise seed of the profile it left.
            from_seed: u64,
            /// The noise seed of the profile it entered.
            to_seed: u64,
        },
    ClearanceExpiring {
            /// The site the clearance is for.
            site: String,
            /// Seconds left.
            remaining_secs: u64,
        },
    CooldownStarted {
            /// The site.
            site: String,
            /// When it may be tried again, in seconds since the Unix epoch.
            until: u64,
        },
    IdentityExported {
            /// How many cookies it holds.
            cookies: usize,
        },
    IdentityRestored {
            /// How many cookies it holds.
            cookies: usize,
        },
}
```

Something a scrape did or decided, with nothing secret in it.

### `ScraperEvent` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub const fn name(&self) -> &'static str
```


- `name`: The name of the variant in `snake_case`, the value of the `event` field when serialized.

### `EventSink`

<Badge color="blue" shape="pill">trait</Badge>

```rust
pub trait EventSink
```

Where scraper events go.

### `EventSink` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
fn emit(&self, event: &ScraperEvent)
```


- `emit`: Takes in one event.

### `NoopEventSink`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct NoopEventSink;
```

A sink that drops every event.

### `CollectingEventSink`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct CollectingEventSink
```

A sink that keeps the newest [`MAX_COLLECTED_EVENTS`] events, for tests and short runs.

### `CollectingEventSink` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn new() -> Self
pub fn events(&self) -> Vec<ScraperEvent>
pub fn take(&self) -> Vec<ScraperEvent>
pub fn len(&self) -> usize
pub fn is_empty(&self) -> bool
```


- `new`: A sink with no events.
- `events`: The events so far, oldest first.
- `take`: The events so far, oldest first, removed from the sink.
- `len`: How many events the sink holds.
- `is_empty`: Whether the sink holds no event.

## `ledger::state`

### `DomainState`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct DomainState {
    pub site: String,
    pub successes: u64,
    pub challenges: u64,
    pub blocks: u64,
    pub consecutive_challenges: u32,
    pub last_challenge: Option<ChallengeKind>,
    pub last_seen: u64,
    pub cooldown_until: Option<u64>,
    pub preferred_mode: Option<Mode>,
    pub last_egress: Option<EgressKey>,
}
```

What is known about one site: how it has answered, and how it should be approached next. `site` is the key; callers pass the registrable domain.

### `DomainState` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn new(site: impl Into<String>) -> Self
pub fn record(&mut self, detection: &Detection, mode: Mode, now: u64)
pub fn start_cooldown(&mut self, until: u64)
pub fn in_cooldown(&self, now: u64) -> bool
```


- `new`: A site that has not been seen.
- `record`: Takes in one response, fetched in `mode` at time `now` (seconds since the Unix epoch).
- `start_cooldown`: Leaves the site alone until `until`. A cooldown that is already longer stays.
- `in_cooldown`: Whether the site is to be left alone at time `now`.

### `StateError`

<Badge color="blue" shape="pill">enum</Badge>

```rust
pub enum StateError {
    Io {
            /// The state file.
            path: PathBuf,
            /// What the operating system said.
            #[source]
            source: io::Error,
        },
    Corrupt {
            /// The state file.
            path: PathBuf,
            /// Where the parse failed.
            #[source]
            source: serde_json::Error,
        },
    Schema(u32),
    TooLarge {
            /// The state file.
            path: PathBuf,
            /// The most it may hold.
            limit: u64,
        },
}
```

Why a state store failed.

### `StateStore`

<Badge color="blue" shape="pill">trait</Badge>

```rust
pub trait StateStore
```

Where per-site state lives between requests.

### `StateStore` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
async fn get(&self, site: &str) -> Result<Option<DomainState>, StateError>
async fn put(&self, state: DomainState) -> Result<(), StateError>
async fn remove(&self, site: &str) -> Result<(), StateError>
async fn sites(&self) -> Result<Vec<String>, StateError>
```


- `get`: The state of `site`, when there is any.
- `put`: Saves `state`, replacing what was there for its site.
- `remove`: Forgets `site`. Forgetting a site that is not there is not an error.
- `sites`: Every site that has state, in order.

### `InMemoryStateStore`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct InMemoryStateStore
```

A store that keeps everything in memory, behind an `Arc`, so that clones share it.

### `InMemoryStateStore` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn new() -> Self
```


- `new`: An empty store.

## Related

The guide is [BrowserScraper](/stealth/browser-scraper). The overview is [scrape](/stealth/api/scrape).
