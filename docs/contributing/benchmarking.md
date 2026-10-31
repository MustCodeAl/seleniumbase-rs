---
title: "Benchmarking"
description: "Run the cpu and cdp_latency harnesses in benches/, time the sbrs binary with hyperfine, and compare a change against main fairly."
sidebarTitle: "Benchmarking"
keywords: ["benchmark", "hyperfine", "cargo bench", "performance", "latency", "baseline"]
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
- How to time the `sbrs` binary itself with hyperfine, and why the first run is slow.

</Info>

<Visibility for="agents">

Summary for AI exports: `benches/cpu.rs` (no browser; `--features turso` adds database figures) and `benches/cdp_latency.rs` (needs Chrome or Chromium; headless, local page) are plain programs with `harness = false`, timed with `std::time::Instant`. Run `cargo bench --bench cpu` or `cargo bench --bench cdp_latency`. They print Markdown tables to standard output and save nothing; redirect the output to keep it. There is no criterion or divan, no saved baseline and no CI benchmark job. To time the `sbrs` binary, build it with `cargo build --release --locked --bin sbrs` and run `hyperfine --warmup 3 --runs 20 'target/release/sbrs --version'`; to compare two builds, keep the old binary and give hyperfine both commands.

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

## Time the sbrs binary with hyperfine

The harnesses time the library. To time the `sbrs` program itself, such as how
long it takes to start, use [hyperfine](https://github.com/sharkdp/hyperfine). It
runs a command many times and prints the mean, the spread and the fastest and
slowest run. The harnesses are not criterion, so there is no saved criterion
baseline either; hyperfine compares builds by timing both in one run, as below.

<Steps>

<Step title="Build the release binary" icon="hammer">

```bash Terminal
cargo build --release --locked --bin sbrs
```

Time `target/release/sbrs`, the file itself. A debug build is slower than what
users run, and a launcher in front of the binary adds its own start-up time.

</Step>

<Step title="Warm up, then time it" icon="stopwatch">

<Badge color="orange" icon="flask" stroke>Not run here</Badge>

```bash Terminal
hyperfine --warmup 3 --runs 20 'target/release/sbrs --version'
```

`--warmup 3` runs the command three times before timing. A program that was
just written or replaced is slow the first time it runs, because macOS scans it
(see [Known traps](/contributing/debugging#known-traps)), and filesystem caches
are not filled yet. Without a warm-up, hyperfine warns when the first run was
significantly slower than the rest. `--runs 20` fixes the
number of timed runs; left out, hyperfine takes at least 10 runs and at least
three seconds.

</Step>

<Step title="Compare two builds" icon="code-compare">

There is no saved baseline to compare against, so keep the old binary and time
both in one call:

<Badge color="orange" icon="flask" stroke>Not run here</Badge>

```bash Terminal
git switch main
cargo build --release --locked --bin sbrs
cp target/release/sbrs target/sbrs-before
git switch my-branch
cargo build --release --locked --bin sbrs
hyperfine --warmup 3 --runs 20 --export-markdown target/bench-sbrs.md \
    'target/sbrs-before --version' 'target/release/sbrs --version'
```

With several commands, hyperfine ends with a summary of how many times faster
one ran than the other. `--export-markdown` writes the table to a file that you
can paste into a pull request, with the machine and the command.

</Step>

</Steps>

This is what hyperfine 2.0.0 printed on the machine this section was written
on, for `hyperfine --runs 3 'sbrs --version'`. It ran with three runs and no
warm-up, so it shows the warning rather than a figure to quote, and `sbrs` was a
mise shim there, not the binary: 111 ms is far above the 9 ms that
[the CLI guide](/tutorials/cli_usage) measured for `sbrs version` itself.

<Badge color="green" icon="check" stroke>Run here</Badge>

```text hyperfine --runs 3
Benchmark 1: sbrs --version (3 runs)
                mean     ±       σ          min     …     max
  Wall Time    111.6 ms  ±     8.1 ms     106.7 ms  …   120.9 ms
  Memory         8.5 MiB ±     0.0 MiB      8.5 MiB …     8.5 MiB

  Warning: The first benchmarking run for this command was significantly slower than the rest (120.9 ms). This could be caused by (filesystem) caches that were not filled until after the first run. You should consider using the '--warmup' option to fill those caches before the actual benchmark. Alternatively, use the '--prepare' option to clear the caches before each timing run.
```

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
