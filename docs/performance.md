# Performance

Rust's low runtime overhead and native binaries can make test orchestration
faster, especially for suites that process a lot of data, generate reports, or
coordinate many parallel browsers. However, browser tests are usually dominated
by external factors: page loading, rendering, network latency, WebDriver round
trips, and browser process startup.

This page explains where Rust helps, where it does not, and how to measure
performance meaningfully. For what the library itself costs, measured against a
real Chrome (a click takes about half a millisecond, a protocol round trip
about 0.1 ms, and the driving process stays near 9 MiB), see
[Benchmarks](benchmarks.md).

## Where Rust helps

- **Test harness overhead**: compiling to native code removes interpreter startup
  and reduces per-test overhead.
- **Report generation**: parsing logs, building dashboards, and computing
  statistics benefit from Rust's performance.
- **Parallel coordination**: Tokio and explicit ownership make it easier to run
  a bounded number of browser instances without hidden contention.
- **Binary distribution**: a single native CLI starts quickly and has no Python
  or Node runtime to load.

## Where Rust does not help

The following costs are largely independent of the language driving the browser:

- Page load and rendering time.
- Network latency and DNS resolution.
- WebDriver command round trips.
- Browser process startup and shutdown.
- Time spent waiting for JavaScript frameworks to hydrate the DOM.

Because of this, do not assume that porting a test from Python or JavaScript to
Rust will automatically make it faster. Measure first.

## How to measure

### Benchmark representative tests

Measure complete, realistic test flows rather than micro-benchmarks of language
features. A fast loop that does not touch the browser tells you very little
about suite performance.

### Separate cold and warm startup

Browser and driver startup are expensive one-time costs. Report them separately
from steady-state test execution so you can tell whether optimizations are
helping the right thing.

### Track the right metrics

| Metric | Why it matters |
|--------|----------------|
| Median duration | Typical user-visible test time. |
| Tail duration (p95/p99) | Worst-case behavior and flaky-test candidates. |
| CPU and memory | Resource usage per browser and per worker. |
| Failure rate | Speed is meaningless if tests are flaky. |
| Driver / browser version | Needed to reproduce results. |

### Control variables

When comparing implementations, keep these fixed:

- Browser version and driver version.
- Machine size and CPU/memory limits.
- Network path and latency.
- Test data and application version.
- Concurrency level.

## Concurrency guidelines

More parallel browsers does not always mean faster tests. Browsers are heavy
processes, and too many concurrent instances can saturate CPU, memory, or the
WebDriver endpoint and make the suite slower.

Start with a small number of workers and increase only when measurements show a
benefit. The `SB_THREADS` setting and the `-n` CLI flag control parallelism:

```bash
cargo run --bin sbase -- -n 4 run-scenario --file suite.json
```

## Prefer intent-driven actions

Repeated polling from test code wastes time and adds WebDriver round trips.
Prefer a single action that expresses intent:

```rust
// Good: one wait that polls internally
sb.wait_for_element_visible("#results", 10).await?;

// Less good: manual polling loop
loop {
    if sb.is_element_visible("#results").await? {
        break;
    }
    tokio::time::sleep(Duration::from_millis(200)).await;
}
```

Use CDP directly only when it removes a measured bottleneck or provides behavior
that WebDriver cannot expose.

## CDP performance notes

CDP can reduce round trips for some operations, but it also adds complexity:

- CDP commands run against the browser's debug protocol and may not be
  supported by all browsers or driver configurations.
- Network interception and request mutation can add latency.
- Use CDP for operations such as setting headers, throttling network, or
  clearing cache when those operations are on the critical path.

## Pauses that are there on purpose

`BaseCase::set_run_settings` (`RunSettings`) adds or removes deliberate waiting:

- **Demo and slow modes** pause after `open`, `click`, `type_text` and
  `update_text` (demo mode also outlines the element first), so a person can
  follow the run. Never leave them on in CI.
- **Fast mode** cancels those demo and slow pauses. It does not make the browser
  faster.
- **Skip JavaScript waits** makes `wait_for_ready_state_complete` return at once.
  Use it only for pages that never finish loading, and expect more flaky
  clicks.
- **Message duration** sets how long `post_message` toasts stay on screen.

These pauses were added in the parity work and have been unit-tested without a
browser, but their cost has not been benchmarked.

## Choosing the engine

- The **pure CDP engine** (`sb_cdp`) has no chromedriver process and no HTTP
  request per command, and the library's own part of a round trip is about 0.1 ms
  (see [Benchmarks](benchmarks.md)). It is the lower-overhead choice when you do
  not need the Selenium API. The saving over WebDriver is structural, not
  measured.
- The **WebDriver engine** is the choice when you need Selenium Grid, Firefox or
  an existing `BaseCase` suite. Match the chromedriver to Chrome's major version
  (the launcher and `sbase install` do this); a mismatch costs far more than any
  tuning.
- Launching Chrome to a DevTools connection takes about 0.5 s, while attaching
  and loading a first page takes about 50 ms, so reuse a browser through
  `BrowserPool` leases instead of launching one per test.
- Enter text with `fill` (about 0.4 ms) and use key-by-key typing (about 142 ms for
  11 characters) only when a page needs to see keystrokes.
- A failed MCP tool call reads the page's state for at most two seconds before
  answering, so failures are slower than successes by design.

## Performance checklist

- [ ] Measure complete representative tests, not empty loops.
- [ ] Separate cold startup from warm execution.
- [ ] Track median, tail, CPU, memory, and failure rate.
- [ ] Keep browser, driver, machine, network, and data fixed during comparisons.
- [ ] Increase concurrency only after measuring a benefit.
- [ ] Prefer a single wait or action over manual polling loops.
- [ ] Use CDP only when it solves a measured bottleneck.

