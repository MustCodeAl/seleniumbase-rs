# Performance

Rust test orchestration has low runtime overhead and native binaries start
without loading a Python or Node interpreter. Typed data processing, report
generation, and parallel coordination can also benefit from Rust's performance.

Browser tests are usually dominated by page loading, rendering, network
latency, WebDriver round trips, and browser process startup. Do not assume that
rewriting a test changes those costs.

## Measure meaningful workloads

- Benchmark complete representative tests, not empty language loops.
- Compare cold and warm browser startup separately.
- Track median and tail duration, CPU, memory, and failure rate.
- Keep browser version, driver version, machine size, network, and test data
  fixed during comparisons.
- Use bounded concurrency. More simultaneous browsers can increase contention
  and make the suite slower.

Prefer one browser action that expresses the intent over repeated polling from
test code. Use CDP directly only when it removes a measured bottleneck or
provides behavior unavailable through WebDriver.

