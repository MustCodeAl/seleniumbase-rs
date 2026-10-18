---
title: "MasterQA Guide"
---

MasterQA is a hybrid testing workflow that mixes automated navigation with
manual verification steps. It is useful when some checks require human judgment
or sign-off, but the setup and navigation can still be scripted.

## What you will learn

- How to start a MasterQA session.
- How to insert manual verification steps.
- What the generated report contains.
- When MasterQA is the right tool.

## Start a MasterQA session

```rust,no_run
use seleniumbase_rs::{BaseCase, BrowserConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut sb = BaseCase::new(BrowserConfig::default().with_headless(false)).await?;
    sb.open("https://seleniumbase.io/simple/login").await?;

    sb.start_qa_session("Verify Login Page");

    // Automated steps
    sb.type_text("#username", "demo_user").await?;
    sb.type_text("#password", "secret_pass").await?;
    sb.click("#log-in").await?;

    // Manual verification steps: each one prompts in the terminal.
    sb.manual_verify("The dashboard loads.");
    sb.manual_verify("The user avatar is visible.");

    // Write the report of the session and keep its path.
    let report = sb.save_qa_report()?;
    println!("Report written to: {}", report.display());

    sb.quit().await?;
    Ok(())
}
```

## How it works

- `start_qa_session(title)` starts a `MasterQaSession` with that title and makes
  it the active one (it replaces any earlier session). It returns
  `&mut MasterQaSession`, and `qa_session()` returns the active one.
- `manual_verify(question)` is synchronous. It prints
  `Manual QA verification required: <question> [Y/n]` and waits for a line on
  standard input: Enter, `y` or `yes` passes the step, `n` or `no` fails it. The
  answer is recorded in the active session and returned as a `bool`. With no
  active session one titled "Manual verification" is started and stays active.
  If standard input is closed, so nobody can answer, the step is recorded as
  failed.
- `MasterQaSession::verify_with_notes(question)` does the same and, when you
  answer no, asks for an optional failure note.
- `save_qa_report()` writes the Markdown report of the active session to
  `latest_logs/qa_report_<milliseconds>.md` and returns where it went;
  `save_qa_report_to(path)` writes it to `path`. Both are an `InvalidConfig`
  error when there is no session. Nothing writes a report on its own at the end
  of a session; you call one of them (or `MasterQaSession::save_markdown(path)`).
- Without a browser, `MasterQA::verify(question)` is a one-off prompt.

`MasterQaSession`, `QaStep` and `MasterQA` live in
`seleniumbase_rs::api::master_qa`. A session also offers `all_passed()` (true
only when there is at least one step and every step passed) and
`passed_count()`.

## Report format

The generated Markdown report is a title, then a table with one row per manual
step, then a summary line:

```markdown
# Verify Login Page

| Step | Question | Result | Notes |
|------|----------|--------|-------|
| 1 | The dashboard loads. | PASS | - |
| 2 | The user avatar is visible. | FAIL | - |

**Summary:** 1 / 2 steps passed.
```

<Note>

The report lists only the manual verification steps. Automated steps are not
recorded in it, and there is no timestamp inside the file (the file name carries
one).

</Note>

## When to use MasterQA

- Exploratory testing where full automation is too expensive.
- Validating visual layouts that are hard to assert programmatically.
- Compliance checks that require human sign-off.

## Best practices

- Keep manual verification steps focused on a single question.
- Run MasterQA in headed mode so the operator can see the browser
  (`BrowserConfig::default()` is headless).
- Run it from an interactive terminal (see the warning below).
- Store reports in CI artifacts for audit trails.

<Warning>

Run it from an interactive terminal: a CI job with no standard input records
every manual step as failed.

</Warning>
