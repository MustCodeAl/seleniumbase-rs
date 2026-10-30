//! The prompt of [`BaseCase::recorder_breakpoint`](crate::BaseCase::recorder_breakpoint):
//! the loop that waits for a person to answer while the browser is watched.
//!
//! The loop knows nothing about a browser. It is given a way to read a line
//! and a way to look at the browser once per tick, so a test can drive it
//! with scripted input and no terminal.

use std::future::Future;
use std::io::{self, Write};
use std::time::Duration;

use crate::common::color::palette;

/// How often the browser is looked at while the prompt waits.
pub(crate) const WATCH_TICK: Duration = Duration::from_millis(500);

/// How a breakpoint ended.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BreakpointExit {
    /// The person typed `c` or `continue`.
    Continue,
    /// The person typed `q` or `quit`.
    Quit,
    /// There is no more input (nextest's null input, CI, Ctrl-D): the test
    /// goes on, so a breakpoint left in a test never hangs a run.
    InputClosed,
    /// The watch found no browser window left.
    WindowsGone,
}

/// What a typed line asks for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Answer {
    Continue,
    Quit,
    Again,
}

fn answer(line: &str) -> Answer {
    match line.trim().to_ascii_lowercase().as_str() {
        "c" | "continue" => Answer::Continue,
        "q" | "quit" => Answer::Quit,
        _ => Answer::Again,
    }
}

/// The text of the prompt.
pub(crate) fn prompt_text(message: &str, color: bool) -> String {
    let label = palette::BANNER_LABEL.paint("[RECORDER BREAKPOINT]", color);
    format!(
        "\n{label}: {message}\nPerform your manual actions in the browser now.\n\
         Type 'c' and press [ENTER] to continue execution: "
    )
}

/// Asks, and waits for the answer, looking at the browser every `interval`.
///
/// `read_line` starts reading one line (`None` is the end of the input). The
/// pending read is polled again after each tick, never dropped, so no line is
/// lost to an abandoned reader. `tick` looks at the browser and says whether
/// the watch is over because no window is left.
///
/// A line that is neither `c` nor `q` asks again.
///
/// # Errors
///
/// Returns the error of writing the prompt.
pub(crate) async fn run_prompt<R, F>(
    message: &str,
    out: &mut dyn Write,
    mut read_line: R,
    mut tick: impl AsyncFnMut() -> bool,
    interval: Duration,
    color: bool,
) -> io::Result<BreakpointExit>
where
    R: FnMut() -> F,
    F: Future<Output = io::Result<Option<String>>>,
{
    let ask = |out: &mut dyn Write| -> io::Result<()> {
        out.write_all(prompt_text(message, color).as_bytes())?;
        out.flush()
    };
    ask(out)?;
    let mut pending = Box::pin(read_line());
    loop {
        tokio::select! {
            line = &mut pending => match line {
                Ok(Some(text)) => match answer(&text) {
                    Answer::Continue => return Ok(BreakpointExit::Continue),
                    Answer::Quit => return Ok(BreakpointExit::Quit),
                    Answer::Again => {
                        ask(out)?;
                        pending = Box::pin(read_line());
                    }
                },
                Ok(None) | Err(_) => {
                    writeln!(out, "input closed: continuing")?;
                    return Ok(BreakpointExit::InputClosed);
                }
            },
            () = tokio::time::sleep(interval) => {
                if tick().await {
                    return Ok(BreakpointExit::WindowsGone);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    /// A reader that answers with `lines`, each after `wait`, then ends.
    fn lines(
        lines: &[&str],
        wait: Duration,
    ) -> impl FnMut() -> std::pin::Pin<Box<dyn Future<Output = io::Result<Option<String>>> + Send>>
    {
        let queue = Arc::new(Mutex::new(
            lines
                .iter()
                .map(|line| (*line).to_owned())
                .collect::<VecDeque<_>>(),
        ));
        move || {
            let next = queue.lock().unwrap().pop_front();
            Box::pin(async move {
                tokio::time::sleep(wait).await;
                Ok(next)
            })
        }
    }

    async fn run(
        input: &[&str],
        ticks_until_over: Option<usize>,
    ) -> (BreakpointExit, String, usize) {
        let mut out = Vec::new();
        let mut ticks = 0;
        let exit = run_prompt(
            "look here",
            &mut out,
            lines(input, Duration::from_millis(35)),
            async || {
                ticks += 1;
                ticks_until_over.is_some_and(|limit| ticks >= limit)
            },
            Duration::from_millis(10),
            false,
        )
        .await
        .unwrap();
        (exit, String::from_utf8(out).unwrap(), ticks)
    }

    #[tokio::test]
    async fn the_label_is_coloured_only_when_asked() {
        let plain = prompt_text("look", false);
        assert!(
            plain.starts_with("\n[RECORDER BREAKPOINT]: look\nPerform your manual actions"),
            "{plain:?}"
        );
        assert!(!plain.contains('\x1b'), "{plain:?}");
        // The label in Python's `c0`, blue on light cyan; the rest is plain.
        let coloured = prompt_text("look", true);
        assert!(
            coloured.starts_with("\n\x1b[34;106m[RECORDER BREAKPOINT]\x1b[0m: look\nPerform"),
            "{coloured:?}"
        );
        assert!(coloured.ends_with("continue execution: "), "{coloured:?}");
    }

    #[tokio::test]

    async fn c_continues_and_the_prompt_names_the_message() {
        let (exit, said, ticks) = run(&["c"], None).await;
        assert_eq!(exit, BreakpointExit::Continue);
        assert!(said.contains("[RECORDER BREAKPOINT]: look here"), "{said}");
        assert!(
            said.contains("Type 'c' and press [ENTER] to continue execution: "),
            "{said}"
        );
        assert!(
            ticks >= 1,
            "the browser was watched while the prompt waited"
        );
    }

    #[tokio::test]
    async fn other_text_asks_again_and_then_c_continues() {
        let (exit, said, _) = run(&["x", "what", "C"], None).await;
        assert_eq!(exit, BreakpointExit::Continue);
        assert_eq!(said.matches("[RECORDER BREAKPOINT]").count(), 3, "{said}");
    }

    #[tokio::test]
    async fn q_quits() {
        assert_eq!(run(&["quit"], None).await.0, BreakpointExit::Quit);
        assert_eq!(run(&["Q"], None).await.0, BreakpointExit::Quit);
    }

    #[tokio::test]
    async fn the_end_of_the_input_continues_and_says_so() {
        let (exit, said, _) = run(&[], None).await;
        assert_eq!(exit, BreakpointExit::InputClosed);
        assert!(said.ends_with("input closed: continuing\n"), "{said}");
    }

    #[tokio::test]
    async fn no_window_left_ends_the_wait() {
        let (exit, _, ticks) = run(&["c"], Some(2)).await;
        assert_eq!(exit, BreakpointExit::WindowsGone);
        assert_eq!(ticks, 2);
    }
}
