# Performance

Rust's low runtime overhead and native binaries can make test orchestration
faster, especially for suites that process a lot of data, generate reports, or
coordinate many parallel browsers. However, browser tests are usually dominated
by external factors: page loading, rendering, network latency, WebDriver round
trips, and browser process startup.

This page explains where Rust helps, where it does not, and how to measure
performance meaningfully.

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

## Performance checklist

- [ ] Measure complete representative tests, not empty loops.
- [ ] Separate cold startup from warm execution.
- [ ] Track median, tail, CPU, memory, and failure rate.
- [ ] Keep browser, driver, machine, network, and data fixed during comparisons.
- [ ] Increase concurrency only after measuring a benefit.
- [ ] Prefer a single wait or action over manual polling loops.
- [ ] Use CDP only when it solves a measured bottleneck.

