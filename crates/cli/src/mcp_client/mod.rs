//! `sbrs-mcp tools`, `call`, `resources`, `read`, `prompts`, `prompt` and `batch`: an MCP client
//! on the command line, so that a person, a script or an AI agent with only a shell can use any
//! tool of the servers, or of any other MCP server.
//!
//! The server is chosen by `--url` (streamable HTTP), `--connect` (a program that speaks stdio)
//! or `--server` (this program, started as `cdp`, `driver`, `sb` or `dev`); with none of them
//! it is `SB_MCP_URL`, else the `dev` server, which needs no setup.
//!
//! Exit status: 0 the command worked, 1 the tool reported a failure (or the output could not
//! be written), 2 a mistake in what was typed (a flag, an argument, a tool or resource the
//! server does not have), 3 the server could not be reached, refused the token or broke the
//! protocol.

mod args;
mod output;

use std::io::{self, IsTerminal};
use std::path::Path;

use seleniumbase_rs::mcp::client::{CallOutcome, McpClient, ProgressCallback};
use seleniumbase_rs::SeleniumBaseError;

use args::Connect;
pub use args::{BatchArgs, CallArgs, ListArgs, PromptArgs, ReadArgs, ToolsArgs};

/// The command worked.
const OK: i32 = 0;
/// A tool reported a failure, or the output could not be written.
const FAILED: i32 = 1;
/// What was typed is wrong.
const MISTAKE: i32 = 2;
/// The server could not be reached or broke the protocol.
const UNREACHABLE: i32 = 3;

/// The status for an error of the client: a request the server refused is a mistake of the
/// caller, anything else is a failure to talk to the server.
fn status_of(error: &SeleniumBaseError) -> i32 {
    if error.exit_code() == 2 {
        MISTAKE
    } else {
        UNREACHABLE
    }
}

fn fail(error: &SeleniumBaseError) -> i32 {
    eprintln!("sbrs-mcp: {error}");
    status_of(error)
}

/// The status for the result of writing the output. A reader that closes the pipe early
/// (`| head`) is its own choice, not a failure.
fn finish(written: io::Result<()>) -> i32 {
    match written {
        Ok(()) => OK,
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => OK,
        Err(error) => {
            eprintln!("sbrs-mcp: cannot write the output: {error}");
            FAILED
        }
    }
}

/// Connects, runs `work` with the client, closes the session and returns the exit status.
fn session(connect: &Connect, work: impl AsyncFnOnce(&McpClient) -> i32) -> i32 {
    let (target, timeout) = match (connect.target(), connect.timeout()) {
        (Ok(target), Ok(timeout)) => (target, timeout),
        (Err(error), _) | (_, Err(error)) => return fail(&error),
    };
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("sbrs-mcp: cannot start the async runtime: {error}");
            return UNREACHABLE;
        }
    };
    let status = runtime.block_on(async {
        let client = match McpClient::connect(target, timeout).await {
            Ok(client) => client,
            Err(error) => return fail(&error),
        };
        let status = work(&client).await;
        client.close().await;
        status
    });
    // A server that does not hang up must not keep this program alive.
    runtime.shutdown_background();
    status
}

fn progress_callback() -> ProgressCallback {
    Box::new(|report| eprintln!("{}", output::progress_line(&report)))
}

/// Progress goes to standard error when asked for, and by default on a terminal.
fn show_progress(asked: bool) -> bool {
    asked || io::stderr().is_terminal()
}

/// `sbrs-mcp tools [TOOL]`.
pub fn tools(args: ToolsArgs) -> i32 {
    let ToolsArgs { tool, raw, connect } = args;
    session(&connect, async move |client| {
        let listed = match client.list_tools().await {
            Ok(listed) => listed,
            Err(error) => return fail(&error),
        };
        let mut out = io::stdout().lock();
        match &tool {
            None => finish(output::write_tools(&listed, raw, &mut out)),
            Some(name) => match listed.iter().find(|candidate| &candidate.name == name) {
                Some(found) => finish(output::write_tool(found, raw, &mut out)),
                None => {
                    eprintln!(
                        "sbrs-mcp: the server has no tool {name}; `sbrs-mcp tools` lists them"
                    );
                    MISTAKE
                }
            },
        }
    })
}

/// Prints what a call returned and saves its images; returns 1 when the tool reported a failure.
fn report(outcome: &CallOutcome, raw: bool, dir: Option<&Path>, counter: &mut usize) -> i32 {
    let written = finish(output::write_outcome(
        outcome,
        raw,
        &mut io::stdout().lock(),
    ));
    if written != OK {
        return written;
    }
    save_images(outcome, dir, counter);
    if outcome.is_error {
        FAILED
    } else {
        OK
    }
}

fn save_images(outcome: &CallOutcome, dir: Option<&Path>, counter: &mut usize) {
    if outcome.images.is_empty() {
        return;
    }
    let Some(dir) = dir else {
        eprintln!(
            "sbrs-mcp: the result has {} image(s); --out DIR saves them",
            outcome.images.len()
        );
        return;
    };
    match output::save_images(outcome, dir, counter) {
        Ok(paths) => {
            for path in paths {
                eprintln!("sbrs-mcp: saved {}", path.display());
            }
        }
        Err(error) => eprintln!(
            "sbrs-mcp: cannot save the images in {}: {error}",
            dir.display()
        ),
    }
}

/// `sbrs-mcp call TOOL [JSON]`.
pub fn call(args: CallArgs) -> i32 {
    let arguments = match args.arguments(&mut io::stdin()) {
        Ok(arguments) => arguments,
        Err(error) => return fail(&error),
    };
    let CallArgs {
        tool,
        raw,
        out,
        progress,
        connect,
        ..
    } = args;
    let show = show_progress(progress);
    session(&connect, async move |client| {
        let callback = show.then(progress_callback);
        match client.call(&tool, arguments, callback).await {
            Ok(outcome) => report(&outcome, raw, out.as_deref(), &mut 0),
            Err(error) => fail(&error),
        }
    })
}

/// `sbrs-mcp batch [FILE]`: several calls in one session.
pub fn batch(args: BatchArgs) -> i32 {
    let BatchArgs {
        file,
        keep_going,
        raw,
        out,
        progress,
        connect,
    } = args;
    let text = match &file {
        Some(path) => std::fs::read_to_string(path).map_err(|error| {
            SeleniumBaseError::invalid_config(format!("cannot read {}: {error}", path.display()))
        }),
        None => io::read_to_string(io::stdin()).map_err(|error| {
            SeleniumBaseError::invalid_config(format!("cannot read standard input: {error}"))
        }),
    };
    let calls = match text.and_then(|text| args::batch_calls(&text)) {
        Ok(calls) => calls,
        Err(error) => return fail(&error),
    };
    let show = show_progress(progress);
    session(&connect, async move |client| {
        let (total, mut status, mut images) = (calls.len(), OK, 0);
        for (index, call) in calls.into_iter().enumerate() {
            eprintln!("sbrs-mcp: [{}/{total}] {}", index + 1, call.tool);
            let callback = show.then(progress_callback);
            let outcome = client.call(&call.tool, call.arguments, callback).await;
            let result = match outcome {
                Ok(outcome) if raw => {
                    let written =
                        output::write_outcome_json(&call.tool, &outcome, &mut io::stdout().lock());
                    save_images(&outcome, out.as_deref(), &mut images);
                    finish(written).max(if outcome.is_error { FAILED } else { OK })
                }
                Ok(outcome) => report(&outcome, false, out.as_deref(), &mut images),
                Err(error) => fail(&error),
            };
            if result != OK {
                if !keep_going {
                    return result;
                }
                if status == OK {
                    status = result;
                }
            }
        }
        status
    })
}

/// The server must offer the capability a command uses; `what` is `resources` or `prompts`.
fn offers(client: &McpClient, what: &str) -> bool {
    let info = client.server_info();
    let offered = if what == "resources" {
        info.resources
    } else {
        info.prompts
    };
    if !offered {
        eprintln!("sbrs-mcp: the server {} offers no {what}", info.name);
    }
    offered
}

/// `sbrs-mcp resources`.
pub fn resources(args: ListArgs) -> i32 {
    let ListArgs { raw, connect } = args;
    session(&connect, async move |client| {
        if !offers(client, "resources") {
            return MISTAKE;
        }
        match client.list_resources().await {
            Ok(listed) => finish(output::write_resources(
                &listed,
                raw,
                &mut io::stdout().lock(),
            )),
            Err(error) => fail(&error),
        }
    })
}

/// `sbrs-mcp read URI`.
pub fn read(args: ReadArgs) -> i32 {
    let ReadArgs { uri, out, connect } = args;
    session(&connect, async move |client| {
        if !offers(client, "resources") {
            return MISTAKE;
        }
        match client.read_resource(&uri).await {
            Ok(contents) => finish(output::write_contents(
                &contents,
                out.as_deref(),
                &mut io::stdout().lock(),
                &mut io::stderr().lock(),
            )),
            Err(error) => fail(&error),
        }
    })
}

/// `sbrs-mcp prompts`.
pub fn prompts(args: ListArgs) -> i32 {
    let ListArgs { raw, connect } = args;
    session(&connect, async move |client| {
        if !offers(client, "prompts") {
            return MISTAKE;
        }
        match client.list_prompts().await {
            Ok(listed) => finish(output::write_prompts(
                &listed,
                raw,
                &mut io::stdout().lock(),
            )),
            Err(error) => fail(&error),
        }
    })
}

/// `sbrs-mcp prompt NAME`.
pub fn prompt(args: PromptArgs) -> i32 {
    let arguments = match args.arguments() {
        Ok(arguments) => arguments,
        Err(error) => return fail(&error),
    };
    let PromptArgs {
        name, raw, connect, ..
    } = args;
    session(&connect, async move |client| {
        if !offers(client, "prompts") {
            return MISTAKE;
        }
        match client.get_prompt(&name, arguments).await {
            Ok(rendered) => finish(output::write_prompt(
                &rendered,
                raw,
                &mut io::stdout().lock(),
            )),
            Err(error) => fail(&error),
        }
    })
}
