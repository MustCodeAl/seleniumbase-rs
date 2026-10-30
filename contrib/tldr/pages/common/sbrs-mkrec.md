# sbrs mkrec

> Record your actions in a browser and save them as a new Rust test file.
> Stop with `c` and `<Enter>` at the prompt, or `<Shift Esc>` in the browser with `--ee`. Also: `sbrs codegen`, `sbrs record`.
> More information: <https://mustcodeal-seleniumbase-rs.docs7.io/tutorials/recorder_mode>.

- Record a new test, starting at a page:

`sbrs mkrec {{path/to/new_test.rs}} --url {{https://example.com}}`

- Overwrite the test file if it already exists:

`sbrs mkrec --overwrite {{path/to/new_test.rs}} --url {{https://example.com}}`

- Also write the recording as a Gherkin feature:

`sbrs mkrec --behave {{path/to/new_test.rs}} --url {{https://example.com}}`

- Stop the recording with `<Shift Esc>` in the browser instead of the terminal prompt:

`sbrs mkrec {{path/to/new_test.rs}} --url {{https://example.com}} --ee`

- Write a Pure CDP script (no WebDriver) or an `SB()` manager script instead of a test:

`sbrs mkrec {{path/to/script.rs}} --url {{https://example.com}} --rec-sb-cdp|--rec-sb-mgr`

- Replay the saved recording in a fresh browser:

`sbrs replay {{recordings/new_test_rec.json}}`
