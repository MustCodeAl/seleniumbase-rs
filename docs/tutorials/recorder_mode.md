---
title: "Recorder Mode Guide"
sidebarTitle: "Recorder mode"
---

Recorder Mode captures what you do in a browser and writes it as Rust code: an
rstest test by default, or a standalone script, pure CDP code and a Gherkin
feature. It is a fast way to bootstrap a UI test: point, click, type, assert,
and then refine the generated file. It needs a Chromium-based browser; with
another one `BaseCase::new` fails.

<Info>

**What you will learn**

- How to record with `sbrs mkrec`, and how to end a recording.
- Which output formats there are, and which flags choose them.
- How to use Recorder Mode from a `BaseCase` test.
- How to replay a recording with `sbrs replay`, and what the `sbrs recorder` terminal app does.
- How to export the calls a test makes itself, as Rust or JSON.

</Info>

<Visibility for="agents">

Summary for AI exports: `sbrs mkrec tests/login.rs --url seleniumbase.io/demo_page` opens a browser in Recorder Mode; type `c` and press Enter in the terminal, or press SHIFT+ESC in the browser with `--ee`, to end it. It writes `recordings/login_rec.rs` and `recordings/login_rec.json` and copies the code to `tests/login.rs`. `--rec-sb-mgr` writes a script, `--rec-sb-cdp` pure CDP code, `--behave` (`--rec-behave`) also a Gherkin feature. `codegen` and `record` are other names for `mkrec`. From a test, `BrowserConfig::with_recorder_mode(true)` (or `sbrs --rec test`, or `SB_RECORDER=true`) writes the recording when the browser quits, and `recorder_breakpoint` pauses for the manual part. `sbrs replay recordings/login_rec.json` replays the JSON file. `sbrs recorder` is a terminal app (feature `tui`) that runs these steps from a form. Separately, `BaseCase` logs the calls it makes: `export_recording_as_rust()` returns a Rust program, `save_recording_to_logs()` saves JSON and Rust, and `ActionRecorder::to_rust_test(name)` returns a `#[tokio::test]`. `sbrs run-scenario` reads hand-written scenarios, not action files.

</Visibility>

## Record with `sbrs mkrec`

`sbrs mkrec FILE` starts a browser in Recorder Mode, waits while you use it,
and writes what you did as a test. `codegen` and `record` are other names for
the same command.

```bash Terminal
sbrs mkrec tests/login.rs --url seleniumbase.io/demo_page
cargo test --test login
```

1. The browser opens on `--url`. Click, type and navigate as you would in the
   test. ESC pauses the recording, and `~` (or a backtick) resumes it.
2. End the recording. At the prompt in the terminal, type `c` and press Enter.
   With `--ee`, press SHIFT+ESC in the browser instead; that also ends when
   every window is closed, or after 600 seconds. Ctrl-C saves what was recorded
   so far.
3. `mkrec` writes `recordings/login_rec.rs` and `recordings/login_rec.json`,
   shows the code with line numbers, and copies it to `tests/login.rs`.

| Flag | What it does |
|---|---|
| `FILE` | The test to create. It must end in `.rs`. A subfolder of the current folder is fine (`tests/login.rs`); an absolute path or `..` is not. |
| `--overwrite` | Replace `FILE` when it exists. Without it `mkrec` refuses. `FILE` is replaced only after a recording was made. |
| `--url URL` | The first page to open. |
| `--ee` (`--esc-end`) | End with SHIFT+ESC in the browser instead of the prompt. |
| `--behave` (`--gherkin`) | Also write a Gherkin feature; the same as `--rec-behave`. |
| `--rec-sb-mgr`, `--rec-sb-cdp` | Write a script or pure CDP code instead of a test. See the table below. |
| `--browser NAME`, `--headed`, `--uc` | The usual browser options. |

## Output formats

Every recording writes the code file and the JSON file; the flags change what
the code file holds and add a feature file. They are global `sbrs` flags, so
they work on `mkrec` and before `test`, and each has an `SB_*` variable and a
`BrowserConfig` method.

| You get | Flag | Variable | `BrowserConfig` | File |
|---|---|---|---|---|
| An rstest test that uses the `sb` fixture (the default) | none | | | `recordings/NAME_rec.rs` |
| A standalone `#[tokio::main]` program that calls `BaseCase` | `--rec-sb-mgr` | `SB_REC_SB_MGR` | `with_rec_sb_mgr(true)` | `recordings/NAME_rec.rs` |
| A standalone program for the pure CDP engine | `--rec-sb-cdp` | `SB_REC_SB_CDP` | `with_rec_sb_cdp(true)` | `recordings/NAME_rec.rs` |
| A Gherkin feature, as well as the code | `--rec-behave` (`--rec-gherkin`; `mkrec --behave`) | `SB_REC_BEHAVE` | `with_rec_behave(true)` | `recordings/features/NAME_rec.feature` |
| The code printed with line numbers | `--rec-print` | `SB_REC_PRINT` | `with_rec_print(true)` | the terminal |
| The recorded actions as JSON | always | | | `recordings/NAME_rec.json` |

`--rec-sleep` (`SB_REC_SLEEP`) keeps the sleeps of the recording.

The feature has the steps `Open "url"`, `Click "selector"`,
`Type "text" into "selector"`, `Go back` and `Go forward`; an action with no
step becomes a `# skipped:` comment. No test file that binds the feature to step functions is
written yet. The pure CDP output is described [below](#pure-cdp-code---rec-sb-cdp).

## Recorder Mode in a test

Any test can run in Recorder Mode. The recorder script goes into every page,
what the person does in each tab is kept, and `quit` writes the files above.
Turn it on with `sbrs --rec test`, which hands the flags to the tests as `SB_*`
variables and runs them one at a time, with `SB_RECORDER=true`, or in code:

```rust,no_run
use seleniumbase_rs::{BaseCase, BrowserConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut sb = BaseCase::new(BrowserConfig::default().with_recorder_mode(true)).await?;
    sb.open("https://seleniumbase.io/demo_page").await?;
    // Pause here: do the manual part in the browser, then type c and press Enter.
    sb.recorder_breakpoint("Click around, then press c here").await?;
    sb.quit().await?; // writes the recording under recordings/
    Ok(())
}
```

`sbrs mkfile NAME.rs --rec` writes a fixture test that opens a page and stops
at a `recorder_breakpoint`; with the file in `tests/`, run it with
`SB_RECORDER=true cargo test --test NAME`.

| Method | What it does |
|---|---|
| `BrowserConfig::with_recorder_mode(bool)` | Turns Recorder Mode on (`--rec`, `SB_RECORDER`). `with_rec_print`, `with_rec_behave`, `with_rec_sb_mgr` and `with_rec_sb_cdp` turn it on too. |
| `BaseCase::recorder_breakpoint(message)` | Pauses the test. `c` continues, `q` ends the test as skipped, and the end of stdin (nextest, CI) continues at once. A test built on the fixture calls `api::recorder::recorder_breakpoint(sb, message)`. |
| `BaseCase::wait_for_recorder_end(limit)` | Waits for SHIFT+ESC (`--ee`), for every window to close, or for `limit`. |
| `BaseCase::activate_recorder()` | Starts recording on the newest tab, for a test that did not turn Recorder Mode on. |
| `BaseCase::set_recording_target(stem, feature, scenario)` | Names the files: `x` is written as `recordings/x_rec.rs`. |
| `BaseCase::finish_recording()` | Writes the files now; `quit` does it otherwise, once. |
| `BaseCase::save_recorded_actions()` | Python's: keeps what the page recorded in this tab, for a test that goes to another origin in the same tab. |

The `sb` fixture names a recording after its test: `tests/login.rs` is recorded
in `recordings/login_rec.rs`. Without it the stem comes from `SB_REC_FILE`,
else from the name of the test thread, else it is `recorded`. A second test of
the same file recorded in one run is added to the same `_rec.rs` file.

## Replay a recording

```bash Terminal
sbrs replay recordings/login_rec.json
```

`replay` starts a fresh browser, runs each action of the file with no
compiling, and prints `ok` or `FAIL` for each, then a summary such as
`Replayed 3 step(s): 3 passed, 0 failed, 0 skipped`. An action with no call to
replay is counted as skipped and named on a `Skipped:` line. The command exits
with status 1 if a step failed. It takes the usual browser options (`--headed`,
`--browser`, `--uc`, `--demo`).

A password typed while recording is not written to the file or the code. A
generated test and `replay` read it from `SB_RECORDED_SECRET`; set
`SB_REC_KEEP_SECRETS=true` before recording to keep the typed text instead.

## The `sbrs recorder` terminal app

`sbrs recorder` is Python's Recorder Desktop App in the terminal. It needs a
build with the `tui` feature (`cargo install --path crates/cli --features
tui`); without it the command prints a hint and exits with status 2.

It shows a form: the file name (`new_recording.rs` to begin with), the start
URL, Overwrite, the browser (Chrome, Chromium, Edge, Opera, Brave or Comet),
Stealth Mode / UC, the output format (test, `--rec-sb-mgr` script,
`--rec-sb-cdp`, or test and Gherkin), `--ee`, and Demo Mode for Run and
Replay. Tab and the arrow keys move between rows, Space or Enter ticks a box or
cycles a list, and Esc leaves. Three actions, from the buttons or from any row:

| Action | Keys | What it runs |
|---|---|---|
| Record | Ctrl-R, or Enter in the URL box | `sbrs mkrec FILE --url=URL --headed ...` with the terminal handed to it. Type `c` and Enter in that terminal, or press SHIFT+ESC with `--ee` ticked. |
| Run | Ctrl-T | `cargo test --test NAME` for a file in `tests/`, `cargo run --example NAME` for one in `examples/`. |
| Replay | Ctrl-P | `sbrs replay recordings/NAME_rec.json`. |

`--url`, `--browser`, `--uc`, `--cdp`, `--ee`, `--rec-behave`, `--rec-sb-mgr`
and `--rec-sb-cdp` fill the form in when the app starts:

```bash Terminal
sbrs recorder --uc --url seleniumbase.io
```

## Record from code

Apart from Recorder Mode, `BaseCase` logs the calls you make through its API.
There is no separate start/stop toggle for that log; run the interactions you
want to capture and then export it. If you want only part of a run,
`start_recording()` marks a starting point and `stop_recording()` returns the
actions recorded since then.

```rust,no_run
use seleniumbase_rs::{BaseCase, BrowserConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut sb = BaseCase::new(BrowserConfig::default()).await?;
    sb.open("https://seleniumbase.io/demo_page").await?;

    sb.click("#myButton").await?;
    sb.type_text("#myInput", "hello").await?;

    // Export the recorded actions as Rust source and JSON.
    let rust_test = sb.export_recording_as_rust()?;
    std::fs::write("my_test.rs", rust_test)?;

    // Or save both formats to the logs directory in one call.
    let (_json, _rust) = sb.save_recording_to_logs()?;

    sb.quit().await?;
    Ok(())
}
```

## Generated output

The recorder produces method calls such as:

```rust,no_run
# use seleniumbase_rs::BaseCase;
# async fn demo(sb: &mut BaseCase) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
sb.click("#myButton").await?;
sb.type_text("#myInput", "hello").await?;
sb.assert_text("Success!", "body").await?;
# Ok(())
# }
```

<Warning>

Only some `BaseCase` methods record themselves: navigation and interaction ones
such as `open`, `click`, `type_text`, `clear`, `hover`, the `select_option_*`
methods, and some waits and assertions. Calls to other methods leave no trace
in the recording.

</Warning>

`export_recording_as_rust` (and the `.rs` file `save_recording_to_logs` writes)
is a complete `#[tokio::main]` program: it imports `BaseCase` and
`BrowserConfig`, creates a `BaseCase` from `BrowserConfig::default()`, runs the
calls, and quits. To get a `#[tokio::test]` function instead, call
`ActionRecorder::to_rust_test(name)`, or `save_recording_as_test(dir, name)` to
write it to `dir/name.rs`. The generated code has no custom timeout and does not
use the `sb_test!` macro.

<Tip>

Always review the output before committing it to your suite.

</Tip>

## Pure CDP code (`--rec-sb-cdp`)

The same recording can be written for the pure CDP engine (`sb_cdp`, no
chromedriver) instead of `BaseCase`, as Python's `--rec-sb-cdp` does with
`sb_cdp` calls. `ActionRecorder::to_cdp_script()` returns a standalone
`#[tokio::main]` program, `to_cdp_test(name)` a `#[tokio::test]` function, and
`save_recording_as_cdp_test(dir, name)` writes that to `dir/name.rs`. For the
recording above the program is:

```rust,ignore
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let browser = Browser::launch(LaunchOptions::builder().build()?).await?;
    let page = browser.default_page().await?;
    page.goto("https://seleniumbase.io/demo_page").await?;
    page.locator("#myButton").click().await?;
    page.locator("#myInput").type_text("hello").await?;
    browser.close().await?;
    Ok(())
}
```

`BaseCase::export_recording_as_cdp_rust()` returns it from a live test. To make
`export_recording_as_rust` and `save_recording_to_logs` write it, as the Python
flag does, turn the setting on with `BrowserConfig::with_rec_sb_cdp(true)`, the
`sbrs --rec-sb-cdp` flag or `SB_REC_SB_CDP=true`; the test itself still runs on
WebDriver, and only the generated code changes.

Every selector and value is written as a Rust string literal. 51 recorded
actions have a pure CDP call: `open`, the clicks, typing (`type_text`, which
empties the field first as `BaseCase::type_text` does, and `send_keys` for
`add_text`, which adds to what the field holds), the `select_option_*` methods, link
clicks (`link=` and `p_link=` selectors), drag and drop, the element waits, the
text, attribute and title assertions, history and reload, scrolling, cookies,
`localStorage`, `maximize_window`, `click_with_offset` (from the top-left corner,
or the centre when the recording says `x, y, center`) and `sleep`. The rest
(frame, window and alert switching, JavaScript clicks, `set_window_size`, and
so on) become a `// skipped:` comment that quotes the `BaseCase` call, so a gap
is never silent. Differences from the WebDriver program: a wait uses the pure
CDP engine's own timeout, not ten seconds, and the program is not written when
the test ends; you ask for it.

## JSON action files

Recorder Mode and `sbrs mkrec` write the actions as `recordings/NAME_rec.json`. A test
can also save the calls it made as JSON. From code:

```rust,no_run
# use seleniumbase_rs::BaseCase;
# fn demo(sb: &BaseCase) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
sb.save_recording_json("recorded.json")?;
# Ok(())
# }
```

The file goes in the `latest_logs` folder (the name must be a plain file name),
and holds an array of `{ "name", "target", "value", "timestamp_ms" }` objects.
Replay it with the CLI:

```bash Terminal
cargo run --bin sbrs -- replay latest_logs/recorded.json
```

<Warning>

This is not the file `sbrs run-scenario` reads. That command takes a
hand-written scenario, an object with a `name` and a list of `steps` such as
`{ "action": "click", "css": "#myButton" }`; passing it an action file fails to
parse. See [Recorder CLI](/help/recorder_cli) for both formats.

</Warning>

```mermaid
graph LR
  A["sbrs mkrec, or save_recording_json"] --> B["Action file: an array of actions"]
  B --> C["sbrs replay"]
  D["Hand-written scenario: a name and steps"] --> E["sbrs run-scenario"]
  B -.->|fails to parse| E
```

Action files are portable and can be replayed by CI jobs without recompiling a
test binary. `replay` runs every action the recorder can write as a `BaseCase`
call; any other action is skipped and named on the `Skipped:` line.

## Best practices

- Wait for elements to settle before acting; the recorder captures explicit waits automatically.
- Review the generated code and add assertions where needed.
- Use meaningful selectors in the application under test so generated locators remain stable.
- Run the generated test immediately to confirm it passes before editing it.

## Troubleshooting

<AccordionGroup>

<Accordion title="Recording is empty" icon="video-slash">

**Likely cause:** Actions were performed without a `BaseCase` method (e.g. raw JS).

**Fix:** Use `BaseCase` interaction methods so the internal recorder captures them.

</Accordion>

<Accordion title="Generated selector is brittle" icon="link-slash">

**Likely cause:** Page uses auto-generated IDs.

**Fix:** Replace recorded selectors with stable `data-testid` or class selectors.

</Accordion>

<Accordion title="Replay fails" icon="rotate-left">

**Likely cause:** Target application changed.

**Fix:** Regenerate or hand-edit the action file.

</Accordion>

<Accordion title="run-scenario rejects the saved file" icon="file-circle-xmark">

**Likely cause:** It is an action file, not a scenario.

**Fix:** Replay it with `sbrs replay`.

</Accordion>

<Accordion title="replay reports skipped steps" icon="forward-step">

**Likely cause:** The action has no call to replay; its name is on the `Skipped:` line.

**Fix:** Use the generated Rust test instead, or hand-edit the file.

</Accordion>

</AccordionGroup>
