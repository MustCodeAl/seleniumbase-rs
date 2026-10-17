---
title: "Run Settings: Demo, Slow and Fast Runs"
---

# Run Settings: Demo, Slow and Fast Runs

Python SeleniumBase reads a handful of run-wide options from the pytest command
line: `--demo`, `--slow`, `--fast`, `--demo-sleep`, `--message-duration` and a
few more. A Rust test has no pytest, so `RunSettings` carries the same knobs as
plain data. You build one, hand it to `BaseCase::set_run_settings`, and that
`BaseCase` keeps it. `RunSettings::default()` changes nothing.

This page lists each setting with the Python option it replaces, shows a
complete test, says exactly which `BaseCase` calls honour which setting, and
lists the limits.

## What you will learn

- The eight settings and the Python option behind each.
- How to turn them on in a test, or from environment variables.
- That the demo and slow pauses apply to five calls only.
- What `RunSettings` does not do.

## The settings

`RunSettings` and `PauseKind` are at the crate root (`seleniumbase_rs::RunSettings`;
the module is `seleniumbase_rs::api::run_settings`). Every method below is a
builder that takes and returns `RunSettings`.

| Builder | Python option | Default | Effect |
|---------|---------------|---------|--------|
| `with_demo_mode(bool)` | `--demo` (also `--demo-mode`, `--demo_mode`) | off | Scrolls the element into view and outlines it before `click`, `type_text`, `update_text` and `press_keys`, then pauses after those calls and after `open`. |
| `with_slow_mode(bool)` | `--slow` (also `--slow-mode`, `--slow_mode`, `--slowmo`) | off | The same pauses, without the outline. |
| `with_fast_mode(bool)` | `--fast` (also `--fast-mode`, `--fast_mode`) | off | Deprecated, does nothing. Python reads `--fast` only in CDP mode, to skip waits this crate never makes; it never cancels demo or slow mode. |
| `with_demo_sleep(Duration)` | `--demo-sleep` (also `--demo_sleep`) | 0.5 s | How long the demo and slow pauses last. Python takes a number of seconds; here it is a `Duration`. |
| `with_message_duration(Duration)` | `--message-duration` (also `--message_duration`) | 2.55 s | How long `post_message` and its relatives keep a message on the page. `message_duration()` keeps the milliseconds and `message_duration_headless()` cuts it to 0.75 s as Python does; `post_message` itself still rounds up to whole seconds, so with the default it shows a message for 3 s, not 2.55 s, until it moves to the millisecond value. A zero duration is honoured here; Python treats `0` as unset. |
| `with_default_timeout(Duration)` | none (Rust only; `SB_TIMEOUT` sets it from the environment) | 10 s | The default wait of the `BaseCase`, as `set_timeout` sets it, in whole seconds (a part of a second rounds up). Applied by `run_browser_test` and `Plugins::run_browser_test`; a `BaseCase::new` ignores it, so call `set_timeout` there. |
| `with_skip_js_waits(bool)` | `--skip-js-waits` (also `--skip_js_waits`, `--sjw`) | off | Makes `wait_for_ready_state_complete` return at once. |
| `with_record_sleep(bool)` | `--record-sleep` (also `--rec-sleep`) | off | Puts `BaseCase::sleep` calls into the recording. |
| `with_visual_baseline_reset(bool)` | `--visual-baseline` (also `--visual_baseline`) | off | Makes `check_window` save the page's structure as the new baseline instead of comparing with the old one. |

The defaults are `DEFAULT_DEMO_PAUSE` (500 ms, Python's `DEFAULT_DEMO_MODE_TIMEOUT`) and `DEFAULT_MESSAGE_DURATION`
(2.55 s) in the same module.

Read-only helpers, `pause_for` and `message_duration`, show the combined effect of
the settings, which is handy in your own code and tests. `PauseKind::Full` is a
pause after an action that changed the page, `PauseKind::Short` one after an
action that did not (1/3.4 of the time in demo mode):

```rust,no_run
use std::time::Duration;
use seleniumbase_rs::api::run_settings::{PauseKind, RunSettings};

let settings = RunSettings::default()
    .with_slow_mode(true)
    .with_demo_sleep(Duration::from_millis(500));

// How long a navigating action would pause, and how long a quiet one would.
assert_eq!(settings.pause_for(PauseKind::Full), Duration::from_millis(500));
assert_eq!(settings.pause_for(PauseKind::Short), Duration::from_millis(500));
// How long post_message keeps its message.
assert_eq!(settings.message_duration(), Duration::from_millis(2550));
```

`BaseCase::run_settings()` returns the settings in force. Besides `pause_for` and
`message_duration` there are plain getters `demo_mode()`, `slow_mode()`,
`demo_sleep()`, `skip_js_waits()`, `record_sleep()` and
`reset_visual_baseline()`. The older `pause(tiny: bool)` and `message_seconds()`
stay for code that already calls them.

## A complete test

This test is for watching. It needs a visible browser, so it sets
`with_headless(false)`. The settings go in the configuration, so they apply from
the first action: `run_browser_test` (and `Plugins::run_browser_test`) give
`BrowserConfig::with_run_settings` to the `BaseCase` they start. A `BaseCase`
you build with `BaseCase::new` starts with the defaults; call
`set_run_settings` on it before the first action.

```rust,no_run
use std::time::Duration;

use seleniumbase_rs::{run_browser_test, BrowserConfig, Result, RunSettings};

#[tokio::test]
async fn login_for_a_demo() -> Result<()> {
    // Python: pytest --demo --demo-sleep=0.8 --message-duration=2
    let config = BrowserConfig::default()
        .with_headless(false)
        .with_run_settings(
            RunSettings::default()
                .with_demo_mode(true)
                .with_demo_sleep(Duration::from_millis(800))
                .with_message_duration(Duration::from_secs(2)),
        );

    run_browser_test(config, async |sb| {
        sb.open("https://seleniumbase.io/simple/login").await?; // pause
        sb.type_text("#username", "demo_user").await?; // outline, short pause
        sb.type_text("#password", "secret_pass").await?; // outline, short pause
        sb.click("#log-in").await?; // outline, pause
        sb.assert_text_visible("Welcome!", "body").await?; // no pause
        sb.post_message("Logged in").await?; // stays 2 seconds
        Ok(())
    })
    .await
}
```

With the settings above, `open` and `click` pause 0.8 s. Typing pauses for less
than a third of that, 0.8 s / 3.4 or about 0.24 s (demo mode divides the time by
3.4 for typing, as Python does). `assert_text_visible` has no pause, so the run
moves on at once.

Replace `with_demo_mode(true)` with `with_slow_mode(true)` for the same pauses
without the outline. Slow mode always pauses for the full time, even when typing.

`set_run_settings` replaces the whole value, it does not merge. To undo a
setting, set a new `RunSettings` that leaves it out.

### Choosing the settings when you run the test

Python picks them with command-line options. In Rust the library reads none of
them for you, so a small helper in your test code is the closest thing. The
variable names here belong to this example; the crate reads none of them.

```rust,no_run
use std::time::Duration;

use seleniumbase_rs::api::run_settings::RunSettings;

/// `MY_DEMO=1 cargo test` turns demo mode on, `MY_DEMO_SLEEP=0.5` shortens its pause.
fn run_settings_from_env() -> RunSettings {
    let on = |name: &str| {
        std::env::var(name).is_ok_and(|value| matches!(value.as_str(), "1" | "true" | "on"))
    };
    let mut settings = RunSettings::default()
        .with_demo_mode(on("MY_DEMO"))
        .with_slow_mode(on("MY_SLOW"))
        .with_skip_js_waits(on("MY_SKIP_JS_WAITS"))
        .with_record_sleep(on("MY_RECORD_SLEEP"))
        .with_visual_baseline_reset(on("MY_VISUAL_BASELINE"));
    // try_from_secs_f64 refuses a negative or non-finite number instead of panicking.
    let pause = std::env::var("MY_DEMO_SLEEP")
        .ok()
        .and_then(|value| value.parse::<f64>().ok())
        .and_then(|seconds| Duration::try_from_secs_f64(seconds).ok());
    if let Some(pause) = pause {
        settings = settings.with_demo_sleep(pause);
    }
    settings
}
```

Then build the configuration with
`BrowserConfig::default().with_run_settings(run_settings_from_env())` (or call
`sb.set_run_settings(run_settings_from_env())` on a `BaseCase` you made with
`BaseCase::new`).
Run the test with `MY_DEMO=1 cargo test login_for_a_demo -- --nocapture`.

## Which calls honour which setting

<Warning>

This is the part to read before trusting a setting.

</Warning>

**Demo, slow, fast and the demo sleep** act in five `BaseCase` methods and no
others:

| Call | Outline in demo mode | Pause after | Length of the pause |
|------|----------------------|-------------|---------------------|
| `open` | no (there is no element) | yes | full |
| `click` | yes | yes | full |
| `type_text` | yes | yes | full in slow mode, full divided by 3.4 in demo mode |
| `update_text` | yes | yes | as `type_text` |
| `press_keys` | yes | yes | as `type_text` |

Every other call ignores them: `clear`, `click_link_text`, `highlight_click`,
`refresh`, `go_back`, `go_forward`, the waits, the assertions, `execute_script`,
and the rest. A test that mostly uses those will not slow down in slow mode.

How the three modes combine:

| Settings | Pause for `open` and `click` | Pause for typing |
|----------|------------------------------|------------------|
| none | none | none |
| slow | full | full |
| demo | full | full divided by 3.4 (about 0.35 s by default) |
| demo and slow | full | full divided by 3.4 |
| fast, with or without demo or slow | none | none |

<Note>

Fast mode cancels the pauses only. It does not make the browser or any wait
faster, and demo mode still outlines elements under it, so leave demo mode off
too for a plain fast run.

</Note>

The outline is one magenta box, drawn by a script in the page and put back by a
timer after about a second, so the call does not wait for it. It is not Python's
animation. A selector the outline script cannot use does not fail the action; the
action runs without the outline.

**Message duration** is used by `post_message`, `post_success_message`,
`post_error_message` and `post_message_and_highlight`. `post_message_for(message,
seconds)` takes its own number of seconds and ignores the setting. The messages are
plain boxes, not Python's Messenger widget.

**Skip JavaScript waits** is read by `wait_for_ready_state_complete`, which
`check_window` also calls before it reads the page. `open` does not call it, so
skipping changes nothing for ordinary navigation. A direct call to
`wait_for_angularjs` still waits.

**Record sleep** is read by `BaseCase::sleep`. With it on, each sleep becomes a
`sleep` action in the recording, and the Rust test the recorder generates calls
`sb.sleep(2.0).await`:

```rust,no_run
# use seleniumbase_rs::api::run_settings::RunSettings;
# use seleniumbase_rs::BaseCase;
# async fn demo(sb: &mut BaseCase) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
sb.set_run_settings(RunSettings::default().with_record_sleep(true));
sb.sleep(2.0).await;
let actions = sb.recorded_actions()?;
assert!(actions.iter().any(|action| action.name == "sleep"));
# Ok(())
# }
```

**Visual baseline reset** is read by `check_window`, and so by
`deferred_check_window`, which calls it. The baseline lives in
`visual_baseline/<test>/<name>`, relative to the working directory. `<test>` is
the name of the thread running the test, which Rust's test harness sets to the
test's name; under `#[tokio::main]` it is `main`. Set the reset for one run to
accept the page as it is now, then go back to the default:

```rust,no_run
# use seleniumbase_rs::api::run_settings::RunSettings;
# use seleniumbase_rs::BaseCase;
# async fn demo(sb: &mut BaseCase) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
sb.set_run_settings(RunSettings::default().with_visual_baseline_reset(true));
sb.check_window("home", 3.0).await?; // saves the baseline
sb.set_run_settings(RunSettings::default());
sb.check_window("home", 3.0).await?; // compares with it
# Ok(())
# }
```

What `check_window` compares, and its levels 0 to 3, are in the rustdoc of
`BaseCase::check_window`; the [API Guide](./api_reference.md) lists it. The
reset applies to `assert_visual_match` (feature `visual`) too, which compares
the screenshot's pixels instead of the HTML and keeps its baseline as
`visual_baseline/<test>/<name>.png`, `<test>` there being the test's whole
path (`cart.checkout.renders`); see
[Visual regression testing](./remaining_helpers.md#visual-regression-testing-pixels).

## Limits

<Danger>

**Never in CI.** A demo or slow run waits on purpose and can make a suite many
times longer. Keep them off unless a person is watching.

</Danger>

- **Code only.** Nothing reads these from the command line, the environment, a
  config file or `Settings`. The `sbrs` CLI and the MCP servers do not set them.
  Use a helper like the one above.
- **One `BaseCase`.** The settings belong to the `BaseCase` you called
  `set_run_settings` on. The extra browsers that `get_new_driver` opens belong to
  that same `BaseCase` and share its settings; another `BaseCase` starts from the
  defaults.
- **WebDriver engine only.** The pure CDP engine (`sb_cdp`) has no `RunSettings`.
  Its `Page::post_message` takes a `MessageOptions` with its own duration.
- **Five pausing calls.** See the table above. `click_link_text` and `press_key`,
  for example, never pause.
- **One outline.** Python's `--highlights` (how many times to animate) has no
  counterpart; the outline is drawn once.
- **Not a speed-up.** There is no switch that removes the pauses of demo and
  slow mode except turning them off. `with_skip_js_waits` removes one wait, at
  the cost of more flaky clicks on pages that are still loading.

## What is tested

- The arithmetic is unit-tested without a browser: the default pauses nothing,
  slow mode pauses the full time, demo mode divides it by 3.4 for typing, fast
  mode wins, a message duration rounds up to whole seconds.
- `sleep` recording and skipping the JavaScript waits are unit-tested without a
  browser.
- `check_window` and its baseline reset are unit-tested, and
  `tests/check_window_chrome.rs` exercises them in a real Chrome. It is
  `#[ignore]`d, so it only runs when asked for.
- The outline and the pauses inside `open`, `click`, `type_text`, `update_text`
  and `press_keys` are not run by any test in the repository, with a browser or
  with the mock. They are a few lines each, but treat them as unmeasured.

## Related reading

- [Settings and Configuration](./settings_and_config.md): `BrowserConfig`, the
  config file and the `SB_*` variables.
- [Waits and Assertions](./waits_assertions.md): the waits that
  `with_skip_js_waits` leaves alone.
- [Recorder Mode](./recorder_mode.md): recording actions and exporting them as
  a test.
- [API Guide](./api_reference.md): `check_window`, `post_message` and the rest.
- [Performance](../performance.md): the pauses that are there on purpose.
- [API parity table](../parity.md): every Python option and its Rust name.
