# Results Database and Encrypted Profiles

Two things a team ends up needing are a history of test runs, and a safe place
for browser profiles. Both live in one embedded database,
[Turso](https://github.com/tursodatabase/turso), behind the `turso` feature. It
is off by default, runs in your process, and keeps everything in a local file;
nothing is sent over the network. The feature needs Rust 1.90 or newer, because a
Turso dependency does; without it the crate still builds on 1.89.

```toml
[dependencies]
seleniumbase-rs = { version = "0.1", features = ["turso"] }
```

## What you will learn

- Record test runs and results, and ask which tests are flaky
- Read them from the command line with `sbase report`
- Keep browser profiles encrypted at rest

## Recording test runs

A `ResultStore` keeps runs and their results. Start a run, then record each
test as it finishes:

```rust,no_run
use std::time::Instant;
use seleniumbase_rs::storage::{ResultStore, RunInfo};

# async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
let store = ResultStore::open("reports/results.db").await?;
let run = store
    .start_run(&RunInfo::new("nightly").environment("staging"))
    .await?;

let started = Instant::now();
let outcome: Result<(), seleniumbase_rs::SeleniumBaseError> = Ok(()); // your test
store
    .record_outcome(run, "login_works", started.elapsed(), &outcome)
    .await?;
# Ok(())
# }
```

- **`environment`** is the counterpart of Python SeleniumBase's `--database_env`:
  `staging` and `production` runs share a database but stay apart.
- **`record_outcome`** stores a pass, or a failure whose message is the error.
  `record` takes a `TestResult` when you build it yourself.
- The file and its directory are created on first use. A file that is not a
  results database, or that a newer release wrote, is refused rather than
  misread.
- A store is cheap to clone, and every clone shares the database.

Ask it questions:

```rust,no_run
# use seleniumbase_rs::storage::ResultStore;
# async fn demo(store: ResultStore) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
for run in store.runs(10).await? {
    println!("{} {}: {} passed, {} failed", run.id, run.label, run.passed, run.failed);
}

// Tests that passed in some of the last 20 runs and failed in others.
for test in store.flaky_tests(20).await? {
    println!("{}: failed {} of {}", test.name, test.failures, test.attempts);
}
# Ok(())
# }
```

A test that always fails is broken, not flaky, so `flaky_tests` leaves it out.
Test messages are stored as written; if an assertion message can contain a
secret, redact it before recording.

## `sbase report`

Build the CLI with the feature, and read a database without writing any code:

```bash
cargo build --features turso
sbase report --db reports/results.db            # the latest runs, newest first
sbase report --db reports/results.db --run 12   # one run, with failure messages
sbase report --db reports/results.db --flaky
sbase report --db reports/results.db --json
```

## Encrypted profiles

A browser profile holds cookies, saved logins and proxy passwords, so a
`profiles.json` is a file anyone who can read the disk can read. A
`ProfileVault` stores the same documents in a Turso database with each one
sealed by AES-256-GCM under a key derived from a passphrase (PBKDF2-HMAC-SHA256,
600,000 iterations):

```rust,no_run
use serde::{Deserialize, Serialize};
use seleniumbase_rs::storage::ProfileVault;

#[derive(Serialize, Deserialize)]
struct Profile { name: String, proxy: String }

# async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
let vault = ProfileVault::open("profiles.vault", "a long passphrase").await?;

vault
    .put("profiles", "p1", &Profile { name: "EU shop".into(), proxy: "http://u:p@eu:8080".into() })
    .await?;
let profiles: Vec<(String, Profile)> = vault.list("profiles").await?;
# let _ = profiles;
# Ok(())
# }
```

- **Collections and ids** (`profiles`, `p1`) are stored in the clear; the
  documents are not. Someone with the file learns how many documents there are
  and how large, not what is in them.
- **A document is bound to its place.** Copying a sealed body to another id or
  collection makes it fail its integrity check instead of decrypting as the
  wrong profile.
- **A wrong passphrase is refused** when the vault is opened, and the guess is
  never echoed. An empty passphrase is refused.
- **`change_passphrase`** re-encrypts everything under a new salt in a single
  transaction, so a failure part-way leaves the old passphrase working.
- **`Debug` never shows the key.**

What it does not do: it cannot protect a vault that is open in a process an
attacker controls, because the key has to be in memory to be used, and it does
not hide the number or size of documents. Keep the passphrase somewhere
better than the source tree, such as the operating system's keychain or an
environment variable your deployment injects.
