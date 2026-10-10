# Benchmarks

What the library costs, measured, and what Chrome costs instead. For advice on
making your own tests faster, see [Performance](performance.md). Every figure
here was measured on one machine (Apple M1, 8 cores, macOS 27, Chrome 155,
release build, headless, a local page, nothing else running). Timings from
another machine will differ; the harnesses are in `benches/` so you can
repeat them.

```text
cargo bench --bench cpu                    # the library's own work, no browser
cargo bench --bench cpu --features turso   # adds the result store and vault
cargo bench --bench cdp_latency            # the Pure CDP engine against Chrome
```

**There is no comparison with Python SeleniumBase here.** It is not installed
on the machine these numbers came from, so nothing below claims "X times faster
than Python". What can be said is structural: the Pure CDP engine talks to
Chrome over one WebSocket, with one message per command, and the driving
process stays small. A WebDriver session goes through a separate chromedriver
process and an HTTP request per command; that cost was not measured.

## One tab, one command at a time

| operation | median | p95 |
|---|---|---|
| evaluate `1 + 1` | 137 µs | 358 µs |
| locator count | 101 µs | 161 µs |
| read an element's text | 247 µs | 313 µs |
| click a button | 481 µs | 612 µs |
| fill a field (11 characters) | 408 µs | 611 µs |
| type 11 characters key by key | 142 ms | 150 ms |
| viewport screenshot (PNG) | 33 ms | 69 ms |
| `goto` a local page | 8.3 ms | 10.8 ms |
| a wait notices an element that appeared | 7.5 ms | 9.2 ms |

A protocol round trip costs about 0.1 ms, so almost everything above that is
Chrome's own work, not the library's. Typing is slow on purpose: each key waits
`KEY_DELAY` (8 ms) so a page sees a person's rhythm. `fill` sets the value in
one step and is the fast way to enter text.

## Many tabs at once

Each tab clicks a button 150 times, all tabs at the same time, in one Chrome.

| tabs | clicks / second |
|---|---|
| 1 | 1,105 |
| 2 | 1,791 |
| 4 | 2,524 |
| 8 | 3,223 |

## Start-up and memory

| step | median | fastest |
|---|---|---|
| launch Chrome to a DevTools connection | 482 ms | 467 ms |
| first page attached and loaded | 48 ms | 24 ms |

The process that drives the browsers uses about 9 MiB of resident memory
whether it drives one Chrome or eight. (Chrome's own memory is not counted.)

## The library's own work

No browser involved (`cargo bench --bench cpu`):

| operation | time |
|---|---|
| generate a random fingerprint | 1.6 µs |
| build the stealth bootstrap script (24 KiB) | 87 µs |
| check a fingerprint | 96 ns |
| classify a CSS or XPath selector | 9 ns |
| plan a human mouse path | 3.7 µs |
| plan human typing for 100 characters | 5.9 µs |
| inspect a 6 KiB page for accessibility issues | 268 µs |
| inspect a 260 KiB page | 9.2 ms |

With the `turso` feature: recording a test result takes about 307 µs, the
flaky-test query over 2,000 results about 0.8 ms, sealing and storing an
encrypted profile about 19 µs and opening one about 15 µs. Opening the vault
takes about 58 ms on purpose: that is the key derivation (PBKDF2, 600,000
rounds), which is what makes a stolen vault file slow to attack.

## What changed because of these measurements

Before this work, a click took **33.4 ms** and eight tabs together managed
**239 clicks a second**. The harness found two things:

- **A click on a tab that was not frontmost never finished.** Chrome only
  acknowledges input for the tab it considers focused, so the click waited for
  the 30 second command timeout. Every tab is now told it has focus when it is
  attached (`Emulation.setFocusEmulationEnabled`), the same thing Playwright
  does. Several tabs can now be driven at once; this is what made the
  multi-tab figures above possible at all.
- **The whole cost of a click was waiting for the pointer move to be
  acknowledged.** Chrome answers a `mouseMoved` only at the next frame (two
  frames, 33 ms, in headless mode on this machine), while a press and release
  cost under a millisecond together. But Chrome delivers the pending move ahead
  of the press that follows it, so the move, press and release can be sent back
  to back and awaited together. The page sees exactly the same events in the
  same order (checked on a real Chrome: `mouseover`, `mousemove`, `mousedown`,
  `mouseup`, `click`, all trusted, at the right point). A click now takes
  **0.48 ms**, 69 times less, and eight tabs reach **3,223 clicks a second**,
  13 times more.

## What was measured and left alone

- **Moving the pointer alone (a hover) still takes a frame or two.** Nothing
  follows it to flush it, and a hover that returned early could let the next
  command see the old state.
- **Launching Chrome takes about half a second**, nearly all of it Chrome.
- **Screenshots take about 33 ms**, which is Chrome encoding the image.
- **Launch flags such as `--disable-frame-rate-limit`** halve the cost of a
  pointer move, but they make `requestAnimationFrame` fire faster than a real
  display does, which is a signal anti-bot scripts look for. They are not used.
