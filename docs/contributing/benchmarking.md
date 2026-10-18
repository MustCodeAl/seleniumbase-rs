---
title: "Benchmarking"
description: "Run the cpu and cdp_latency harnesses in benches/, keep their output, and compare a change against main on a fair footing."
sidebarTitle: "Benchmarking"
---

The repository has two benchmarks in `benches/`. They measure what the library
itself costs: its own CPU work, and what it adds on top of a Chrome that is
already running. This page says how to run them and how to compare a change.
The measured figures are on [Benchmarks](/benchmarks), and advice for making
your own tests faster is on [Performance](/performance).

<Info>

**What you will learn**

- What `cpu` and `cdp_latency` measure, and what each needs.
- How to run them, and where their results go.
- How the published figures were produced.
- How to compare before and after a change, and keep the runs fair.

</Info>

<Visibility for="agents">

Summary for AI exports: `benches/cpu.rs` (no browser; `--features turso` adds database figures) and `benches/cdp_latency.rs` (needs Chrome or Chromium; headless, local page) are plain programs with `harness = false`, timed with `std::time::Instant`. Run `cargo bench --bench cpu` or `cargo bench --bench cdp_latency`. They print Markdown tables to standard output and save nothing; redirect the output to keep it. There is no criterion or divan, no saved baseline and no CI benchmark job.

</Visibility>

<RequestExample>

```bash cpu
cargo bench --bench cpu
```

```bash cpu with turso
cargo bench --bench cpu --features turso
```

```bash cdp_latency
cargo bench --bench cdp_latency
```

</RequestExample>

<ResponseExample>

```markdown cpu
# CPU benchmark (no browser)

| operation | time per call | calls / second |
|---|---|---|
| `Fingerprint::randomized` (Windows) | … | … |
| build the stealth bootstrap script (… KiB) | … | … |
| … | … | … |
```

```markdown cdp_latency
# Pure CDP benchmark

Rust stable, N cores, release build.

## Start-up

| step | runs | median | p95 | fastest |
|---|---|---|---|---|
| launch Chrome to a DevTools connection | 5 | … | … | … |
| first page attached and loaded | 5 | … | … | … |
```

</ResponseExample>

## The two benchmarks

Both are plain programs, not a framework. Each `[[bench]]` in `Cargo.toml` sets
`harness = false`, and each file has its own `main` that times with
`std::time::Instant` and prints Markdown tables. There is no criterion or divan,
and no saved baseline.

| Bench | Measures | Needs | Command |
|---|---|---|---|
| `cpu` | The library's own work | <Badge color="green">no browser</Badge> | `cargo bench --bench cpu` |
| `cpu` with `turso` | The same, plus the embedded database | <Badge color="green">no browser</Badge> | `cargo bench --bench cpu --features turso` |
| `cdp_latency` | The pure CDP engine against a real Chrome | <Badge color="orange">Chrome or Chromium</Badge> | `cargo bench --bench cdp_latency` |

### `cpu`

It times, with no browser involved:

- `Fingerprint::randomized` for a Windows persona, building the stealth
  bootstrap script, and `Fingerprint::validate`;
- classifying a CSS or XPath selector (`Selector::auto`, eight selectors per
  call);
- planning a human mouse path (600 px) and human typing (100 characters);
- `HtmlInspector::inspect` on a page of 50 rows and one of 2,000 rows.

Each operation is warmed up (up to 50 calls), then timed in nine batches of a
fixed number of calls. The figure printed is the median batch divided by the
number of calls, with the calls per second next to it. `std::hint::black_box`
keeps the compiler from removing the work.

With `--features turso` a second table follows, from an in-memory
`ResultStore` and `ProfileVault`: recording a test result, the flaky-test query,
opening a vault, and sealing and loading a profile. Each is one timed loop, not
nine batches. Opening the vault is slow on purpose: that is the key derivation.

### `cdp_latency`

It launches Chrome with the pure CDP engine, headless, and serves a fixed page
on a loopback port, so no outside network is involved. What it prints is the
cost the library adds to a browser that is already running.

| Table | What is timed | Runs |
|---|---|---|
| Start-up | Launching Chrome to a DevTools connection, then attaching and loading the first page | 5 launches |
| One tab, one command at a time | `evaluate`, a locator count, reading text, a click, `fill`, typing key by key, a viewport screenshot, `goto` a local page; each warmed up 10 times first | 30 to 500 per operation |
| How long a wait takes to notice an element | The element appears 100 ms after a script runs; the page's own clock measures how late the wait sees it, including one round trip | 40 |
| Many tabs at once | 1, 2, 4 and 8 tabs in one Chrome, each clicking a button 150 times at the same time: wall time and clicks per second | one pass per tab count |
| Memory of the driving process | Resident memory of the benchmark process, not Chrome's, with 0, 1, 4 and 8 browsers | one reading each |

These variables change how it runs:

<ParamField path="CHROME_BIN" type="path">

The Chrome to launch. `CHROMIUM_BIN` is tried next, then the usual install
locations, as the pure CDP engine always does.

</ParamField>

<ParamField path="CI" type="any value">

When set, Chrome is launched with `--no-sandbox`, which it needs as root or in a
container.

</ParamField>

<ParamField path="RUSTC_VERSION" type="text, at build time">

Read when the bench is compiled, for the header line: "Rust stable" when it is
unset. Set it to label a run, for example
`RUSTC_VERSION="$(rustc --version)" cargo bench --bench cdp_latency`.

</ParamField>

<Note>

The memory table reads `ps -o rss=`. Where there is no `ps`, as on Windows, it
shows 0.

</Note>

## Run them

<Steps>

<Step title="Quiet the machine" icon="moon">

Close other builds, browsers and anything else that uses the CPU. On a laptop,
use mains power. Timings from a loaded machine mean little.

</Step>

<Step title="Build and run" icon="play">

```bash Terminal
cargo bench --bench cpu
cargo bench --bench cdp_latency
```

`cargo bench` builds with Cargo's `bench` profile, which inherits `release`;
neither manifest sets a profile of its own. Always name `--bench`: a bare
`cargo bench` also builds the library and the binaries as benchmark targets,
which takes longer and adds nothing, since neither has a benchmark.

</Step>

<Step title="Keep the output" icon="floppy-disk">

The tables go to standard output, and cargo's own messages to standard error.
Nothing is written anywhere else, so redirect the output to keep it:

```bash Terminal
cargo bench --bench cdp_latency > target/bench-cdp.md
```

`target/` is ignored by git, so the file stays out of commits.

</Step>

</Steps>

## How the published figures were made

[Benchmarks](/benchmarks) shows one run of each harness on one machine. That
page names the machine, the Chrome version, the date and the commit, and says
what is uncertain about each figure. The runs were release builds, headless,
against a local page, with nothing else running. The tables there are the
harness tables with some columns left out, and [Performance](/performance)
quotes a few of their figures.

<Warning>

Do not copy those figures into a new page or a pull request as if they were
yours. Run the harnesses on your machine, and quote what you measured with the
machine and the command.

</Warning>

## Compare before and after a change

Nothing in the repository compares two runs for you. Do it by hand, the same
way each time.

<Steps>

<Step title="Measure the baseline" icon="flag">

<Tabs>

<Tab title="Committed change" icon="code-branch">

```bash Terminal
git switch main
cargo bench --bench cpu > target/bench-before-1.md
```

</Tab>

<Tab title="Uncommitted change" icon="box-archive">

```bash Terminal
git stash
cargo bench --bench cpu > target/bench-before-1.md
git stash pop
```

</Tab>

</Tabs>

</Step>

<Step title="Measure the change" icon="code-compare">

```bash Terminal
git switch my-branch    # not needed for an uncommitted change
cargo bench --bench cpu > target/bench-after-1.md
```

</Step>

<Step title="Repeat and compare" icon="scale-balanced">

Run each side at least three times, alternating before and after, and compare
the medians row by row, for example with
`diff target/bench-before-1.md target/bench-after-1.md`. A difference smaller
than the spread between repeated runs of the same commit is noise.

</Step>

</Steps>

<Warning>

To keep the baseline in a second worktree instead, give each worktree its own
target directory. Two checkouts that share one `CARGO_TARGET_DIR` overwrite each
other's compiled library.

</Warning>

When a pull request is about speed, put the before and after figures in its
description, with the machine, the Chrome version and the command.

## Keep the runs fair

- **An optimised build.** Time `cargo bench` only, never a debug build.
- **The same everything.** The same machine, power settings, Chrome version (for
  `cdp_latency`), features and command on both sides.
- **No background load.** A single run is a sample: on a busy machine, the same
  table can move a lot between two runs.
- **Headless only.** The harness always runs Chrome headless. Real-Chrome
  timings differ between headless and headed runs, so do not compare one with
  the other.
- **Cold starts vary.** The start-up table launches Chrome five times; a cold
  start can take much longer than the median.
- **Containers.** Inside one, set `CI` for `--no-sandbox`, and keep its memory
  limit the same for both sides.

## In CI

<Note>

No CI job runs the benchmarks. `.github/workflows/` builds, tests, lints, checks
the minimum Rust version, the dependency policy and the docs, but has no
`cargo bench`. The `Dockerfile` only creates empty `benches/cdp_latency.rs` and
`benches/cpu.rs` in its skeleton step.

</Note>

## Add a benchmark

<Steps>

<Step title="Write the program" icon="file-code">

Add a file to `benches/` with its own `main`, as the two that exist: warm up,
time several runs, and print the median as a Markdown table.

</Step>

<Step title="Declare it" icon="file-lines">

Add a `[[bench]]` with `harness = false` to `Cargo.toml`.

</Step>

<Step title="List it in the Dockerfile" icon="docker">

Add the file to the skeleton step of the `Dockerfile`, or the image build fails;
`tests/dockerfile_skeleton.rs` checks it.

</Step>

</Steps>

## Next steps

<Columns cols={2}>

<Card color="#1a85d4" title="Benchmarks" icon="chart-column" href="/benchmarks">

The figures from these harnesses, and what changed because of them.

</Card>

<Card color="#199f43" title="Performance" icon="gauge-high" href="/performance">

How to make your own tests faster, and how to measure them.

</Card>

<Card color="#d47628" title="Build from source" icon="hammer" href="/contributing/from-source">

The toolchain, the build settings and the checks.

</Card>

<Card color="#a631be" title="Debugging" icon="bug" href="/contributing/debugging">

Logs, evidence on failure, and a browser you can watch.

</Card>

</Columns>
