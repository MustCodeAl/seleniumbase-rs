# rstest-bdd 0.6 spike: how a scenario's browser is closed

Written for lane RUN-7 (`bdd-core`), 2026-10-23. It answers the first question of
the runner spec (section 4.5): teardown mechanism A (a harness that finalizes the
fixtures) or B (a helper thread that re-attaches), and what the spike showed
instead. Crates: `rstest-bdd`, `-macros`, `-harness`, `-harness-tokio`, all 0.6.0
(the newest on crates.io), rstest 0.26.

## What was measured

A test crate with a `Probe` fixture whose `Drop` prints the thread's panic state
and whether a Tokio runtime is current, async `#[given]`/`#[when]`/`#[then]`
steps that sleep (so they yield `Pending`), and four scenarios:

| Scenario | Shape | Result |
|---|---|---|
| S1 | `async fn`, `#[tokio::test(flavor = "current_thread")]`, sync fixture, no harness | Runs. Steps are awaited in the test's own runtime. The fixture drops at the end of the test function, **inside the runtime** (`Handle::try_current()` is `Ok`), after the scenario function's body; `thread::panicking()` is `true` when a step panicked. |
| S2 | as S1 with an async fixture | Runs with `#[future(awt)] probe: Probe`. Plain `#[future] probe: Probe` does **not** compile (`E0310`: the macro needs the parameter type to be `'static`). |
| S3 | sync `fn` with `harness = TokioHarness` | The first step that yields `Pending` fails the scenario: `async step yielded Pending inside a harness-provided runtime; multi-poll async steps are not supported`. |
| S4 | the same under a harness of ours wrapping `TokioHarness` | The same failure. Our `run` is called with no Tokio runtime current; the fixture is still dropped by the test function after `run` returned. |

Two compile-time facts: `harness = ...` on an `async fn` scenario is refused
(`combining harness with async fn scenarios is not supported; use a synchronous
scenario function with TokioHarness instead`), and a fixture requested with
`#[from(other_name)]` is stored under `other_name`, so a step must ask for it by
that name.

## Decision: neither A nor B

* **A (a harness finalizes the fixtures) is not possible in 0.6.0.** The harness
  path cannot run a step that waits, so no browser step can run under it, and a
  harness cannot be combined with the `async fn` scenarios that can.
* **B (re-attach from `Drop` with a new client) is possible but not needed.**
  It would need a second client for each engine, and its evidence would come
  from a different connection than the steps used.
* **Chosen: C, a thread per scenario.** `Scenario::start`
  (`src/testing/bdd/session.rs`) starts a thread with a current-thread runtime
  of its own, which runs `Lifecycle::run` (the lifecycle of `Sb::run`) with a
  body that hands the open case to the fixture and waits for it to come back.
  The steps use the case from the test's runtime. When the fixture is dropped
  it sends the case back with the verdict (`thread::panicking()`, and the
  message of the panic, kept by a chained panic hook) and joins the thread; the
  thread then does what the lifecycle does after a body: evidence of a failure
  with the page still open, `--pdb`/`--final-debug` prompts, finalizers, close.
  Nothing needs the test's runtime to make progress except a connection a step
  opens late (see the limit below).

So `SbHarness` of the spec does not exist: the fixture alone does the job, and
`rstest-bdd-harness` / `rstest-bdd-harness-tokio` are only needed by
users who want their own harness. (They stay in the feature's dependencies for
now; remove them if no lane needs them.)

## Limits this leaves, and what is documented

* A scenario's steps run once: `reruns` is forced to 0, and `--time-limit` is
  the case's own soft limit, not a hard stop (a step cannot be dropped halfway).
* A connection a step opens late (after the fixture's own keep-alive one was
  closed) is driven by the test's runtime, which is blocked while the fixture is
  dropped. With `#[tokio::test(flavor = "multi_thread")]` its other workers keep
  it alive; with the default current-thread flavor the evidence may then time
  out. The docs recommend the multi-thread flavor.
* Outlines expand to rstest cases; the test names are in the scenario-name table
  below.

## Scenario-name table (for `sbrs behave-gui`, GUI-4)

`#[scenario(path, name = "...")]` names the test function the user writes.
`scenarios!("dir")` names tests after the feature file and the scenario title,
ASCII-sanitized. `sbrs behave-gui` therefore filters `cargo test` by the
snake-case form of the scenario's title (lower case, runs of other characters
as one `_`): a scenario `Valid login` is selected by `valid_login`; an outline
runs all of its example rows (`case_1`, `case_2`, ...) under the same filter.
`feature_files::scenario_filter` is that function.

## Library-defined steps (for RUN-8)

Steps defined in this crate register through `inventory`. A test binary that
uses nothing else of this crate can have the object file dropped by the linker;
`seleniumbase_rs::testing::bdd::steps::link()` is the place for a reference
(RUN-8 owns it). This spike did not test a step defined in a library crate.
