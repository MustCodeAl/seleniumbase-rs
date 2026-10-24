//! The command line of `sbrs-mcp tools`, `call`, `resources`, `read`, `prompts`, `prompt` and
//! `batch`: the flags, and what they ask for.

use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;

use seleniumbase_rs::mcp::client::Target;
use seleniumbase_rs::mcp::Profile;
use seleniumbase_rs::SeleniumBaseError;
use serde_json::{Map, Value};

/// The environment variable that names the server to connect to when no flag does.
pub const URL_VAR: &str = "SB_MCP_URL";

/// Seconds to wait for the connection and for each request, when `--timeout` is not given.
const DEFAULT_TIMEOUT: u64 = 60;

// How to reach the server; every command that talks to one flattens it in. (Plain comments, not
// docs: a doc comment on this struct would be help text.)
#[derive(Debug, Default, usage::Args)]
#[usage(args_override_self = false)]
pub struct Connect {
    /// Connect to the streamable HTTP server at this URL (default: SB_MCP_URL)
    ///
    /// The URL of the endpoint, such as http://127.0.0.1:8931/mcp. The bearer token the
    /// server asks for comes from SB_MCP_TOKEN or --token-file, never from the command line.
    /// Each command makes its own MCP session, and with it its own browser; `batch` runs
    /// several calls in one session.
    #[usage(long, value_name = "URL")]
    pub url: Option<String>,
    /// Read the bearer token for --url from this file (default: SB_MCP_TOKEN)
    #[usage(long, value_name = "FILE")]
    pub token_file: Option<String>,
    /// Start this program as the server and talk to it on its standard input and output
    ///
    /// Any MCP server that speaks stdio. Its arguments are --connect-arg, one each.
    #[usage(long, value_name = "PROGRAM")]
    pub connect: Option<String>,
    /// One argument of the program of --connect (repeatable)
    ///
    /// Write it as --connect-arg=--flag when it starts with a dash.
    #[usage(long, value_name = "ARG")]
    pub connect_arg: Vec<String>,
    /// Start this program as the server: cdp, driver, sb or dev (the default, with no flags)
    ///
    /// The server stops when the command ends, and closes its browser.
    #[usage(long, value_name = "cdp|driver|sb|dev")]
    pub server: Option<Profile>,
    /// Seconds to wait for the connection, and for each request (default 60)
    #[usage(long, value_name = "SECONDS")]
    pub timeout: Option<u64>,
}

impl Connect {
    /// The server these flags choose: `--url`, `--connect`, `--server`, else `SB_MCP_URL`, else
    /// this program as the `dev` server.
    pub fn target(&self) -> Result<Target, SeleniumBaseError> {
        let chosen = usize::from(self.url.is_some())
            + usize::from(self.connect.is_some())
            + usize::from(self.server.is_some());
        if chosen > 1 {
            return Err(SeleniumBaseError::invalid_config(
                "--url, --connect and --server each choose the server; give only one of them",
            ));
        }
        if self.connect.is_none() && !self.connect_arg.is_empty() {
            return Err(SeleniumBaseError::invalid_config(
                "--connect-arg is for the program of --connect",
            ));
        }
        if self.token_file.is_some() && self.url.is_none() {
            return Err(SeleniumBaseError::invalid_config(
                "--token-file is for --url; the servers this program starts need no token",
            ));
        }
        if let Some(url) = &self.url {
            return self.http(url);
        }
        if let Some(program) = &self.connect {
            return Ok(Target::Spawn {
                program: PathBuf::from(program),
                args: self.connect_arg.clone(),
            });
        }
        if let Some(server) = self.server {
            return spawn_self(server);
        }
        match std::env::var(URL_VAR) {
            Ok(url) if !url.trim().is_empty() => self.http(url.trim()),
            _ => spawn_self(Profile::Dev),
        }
    }

    fn http(&self, url: &str) -> Result<Target, SeleniumBaseError> {
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            return Err(SeleniumBaseError::invalid_config(format!(
                "{url:?} is not an http:// or https:// URL"
            )));
        }
        Ok(Target::Http {
            url: url.to_owned(),
            bearer: crate::http_token(self.token_file.as_deref())?,
        })
    }

    /// How long to wait for the connection and for each request.
    pub fn timeout(&self) -> Result<Duration, SeleniumBaseError> {
        match self.timeout.unwrap_or(DEFAULT_TIMEOUT) {
            0 => Err(SeleniumBaseError::invalid_config(
                "--timeout must be at least one second",
            )),
            seconds => Ok(Duration::from_secs(seconds)),
        }
    }
}

/// This program, started as the server `profile`.
fn spawn_self(profile: Profile) -> Result<Target, SeleniumBaseError> {
    let program = std::env::current_exe().map_err(|error| {
        SeleniumBaseError::invalid_config(format!("cannot find the running program: {error}"))
    })?;
    Ok(Target::Spawn {
        program,
        args: vec!["--server".to_owned(), profile.name().to_owned()],
    })
}

// The options of `tools`. (Plain comments, not docs.)
#[derive(Debug, usage::Args)]
#[usage(args_override_self = false)]
pub struct ToolsArgs {
    /// Show this one tool with its description and the JSON Schema of its arguments.
    #[usage(value_name = "TOOL")]
    pub tool: Option<String>,
    /// Print JSON (the tools as the server lists them) instead of a table.
    #[usage(long)]
    pub raw: bool,
    #[usage(flatten)]
    pub connect: Connect,
}

// The options of `call`. (Plain comments, not docs.)
#[derive(Debug, usage::Args)]
#[usage(args_override_self = false)]
pub struct CallArgs {
    /// The tool to call.
    #[usage(value_name = "TOOL")]
    pub tool: String,
    /// The arguments as a JSON object, or `-` for a JSON object on standard input.
    #[usage(value_name = "JSON")]
    pub arguments: Option<String>,
    /// One argument, KEY=VALUE (repeatable)
    ///
    /// VALUE is JSON when it parses as JSON (`true`, `3`, `"text"`, `[1,2]`, `{"a":1}`),
    /// text in single quotes is that text, and anything else is a string: `--arg
    /// selector=ref=e1` sends the string `ref=e1`. Not with JSON or --json.
    #[usage(long, value_name = "KEY=VALUE")]
    pub arg: Vec<String>,
    /// All the arguments as one JSON object, or `-` to read it from standard input
    #[usage(long, value_name = "OBJECT|-")]
    pub json: Option<String>,
    /// Print the structured result as compact JSON and nothing else
    ///
    /// A tool that sends no structured result prints its text.
    #[usage(long)]
    pub raw: bool,
    /// Save the images of the result in this folder, as image-1.png, image-2.png, ...
    #[usage(long, value_name = "DIR", value_hint = usage::ValueHint::AnyPath)]
    pub out: Option<PathBuf>,
    /// Print the progress of the tool to standard error (on by default on a terminal)
    #[usage(long)]
    pub progress: bool,
    #[usage(flatten)]
    pub connect: Connect,
}

impl CallArgs {
    /// The arguments of the call: from the JSON, from `--json` or from the `--arg` options.
    pub fn arguments(&self, stdin: &mut dyn Read) -> Result<Map<String, Value>, SeleniumBaseError> {
        let given = [
            self.arguments.is_some(),
            self.json.is_some(),
            !self.arg.is_empty(),
        ];
        if given.iter().filter(|given| **given).count() > 1 {
            return Err(SeleniumBaseError::invalid_config(
                "give the arguments one way: as JSON, with --json, or with --arg KEY=VALUE",
            ));
        }
        if let Some(text) = self.json.as_ref().or(self.arguments.as_ref()) {
            return object(&text_or_stdin(text, stdin)?);
        }
        pairs(&self.arg)
    }
}

// The options of `resources`. (Plain comments, not docs.)
#[derive(Debug, usage::Args)]
#[usage(args_override_self = false)]
pub struct ListArgs {
    /// Print JSON instead of a table.
    #[usage(long)]
    pub raw: bool,
    #[usage(flatten)]
    pub connect: Connect,
}

// The options of `read`. (Plain comments, not docs.)
#[derive(Debug, usage::Args)]
#[usage(args_override_self = false)]
pub struct ReadArgs {
    /// The URI of the resource, such as sbrs://journal.
    #[usage(value_name = "URI")]
    pub uri: String,
    /// Save a binary resource in this folder (text is printed)
    #[usage(long, value_name = "DIR", value_hint = usage::ValueHint::AnyPath)]
    pub out: Option<PathBuf>,
    #[usage(flatten)]
    pub connect: Connect,
}

// The options of `prompt`. (Plain comments, not docs.)
#[derive(Debug, usage::Args)]
#[usage(args_override_self = false)]
pub struct PromptArgs {
    /// The name of the prompt.
    #[usage(value_name = "NAME")]
    pub name: String,
    /// One argument of the prompt, KEY=VALUE, always text (repeatable)
    #[usage(long, value_name = "KEY=VALUE")]
    pub arg: Vec<String>,
    /// Print the messages as JSON.
    #[usage(long)]
    pub raw: bool,
    #[usage(flatten)]
    pub connect: Connect,
}

impl PromptArgs {
    /// The arguments of the prompt, every value a string.
    pub fn arguments(&self) -> Result<Map<String, Value>, SeleniumBaseError> {
        self.arg
            .iter()
            .map(|pair| split(pair).map(|(key, value)| (key, Value::String(value))))
            .collect()
    }
}

// The options of `batch`. (Plain comments, not docs.)
#[derive(Debug, usage::Args)]
#[usage(args_override_self = false)]
pub struct BatchArgs {
    /// Read the calls from this file (default: standard input).
    ///
    /// One JSON object per line, `{"tool": "open_url", "arguments": {"url": "https://x.test"}}`;
    /// blank lines and lines that start with # are skipped. The calls run in order in one MCP
    /// session, so a browser that one call starts is there for the next.
    #[usage(value_name = "FILE", value_hint = usage::ValueHint::AnyPath)]
    pub file: Option<PathBuf>,
    /// Go on after a call fails (default: stop at the first failure)
    #[usage(long)]
    pub keep_going: bool,
    /// Print one JSON object per call, with the text and the structured result
    #[usage(long)]
    pub raw: bool,
    /// Save the images of the results in this folder, numbered across the calls
    #[usage(long, value_name = "DIR", value_hint = usage::ValueHint::AnyPath)]
    pub out: Option<PathBuf>,
    /// Print the progress of the tools to standard error (on by default on a terminal)
    #[usage(long)]
    pub progress: bool,
    #[usage(flatten)]
    pub connect: Connect,
}

/// One call of a `batch`.
#[derive(Debug, Clone, PartialEq)]
pub struct BatchCall {
    /// The tool.
    pub tool: String,
    /// Its arguments.
    pub arguments: Map<String, Value>,
}

/// The calls in the lines of a batch, or the first line that is not one.
pub fn batch_calls(text: &str) -> Result<Vec<BatchCall>, SeleniumBaseError> {
    let mut calls = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let bad = |reason: &str| {
            SeleniumBaseError::invalid_config(format!("line {}: {reason}", index + 1))
        };
        let value: Value = serde_json::from_str(line).map_err(|error| bad(&error.to_string()))?;
        let Some(tool) = value.get("tool").and_then(Value::as_str) else {
            return Err(bad("expected a JSON object with a \"tool\" name"));
        };
        let arguments = match value.get("arguments") {
            None | Some(Value::Null) => Map::new(),
            Some(Value::Object(map)) => map.clone(),
            Some(_) => return Err(bad("\"arguments\" must be a JSON object")),
        };
        calls.push(BatchCall {
            tool: tool.to_owned(),
            arguments,
        });
    }
    if calls.is_empty() {
        return Err(SeleniumBaseError::invalid_config("the batch has no calls"));
    }
    Ok(calls)
}

fn text_or_stdin(text: &str, stdin: &mut dyn Read) -> Result<String, SeleniumBaseError> {
    if text != "-" {
        return Ok(text.to_owned());
    }
    let mut input = String::new();
    stdin.read_to_string(&mut input).map_err(|error| {
        SeleniumBaseError::invalid_config(format!("cannot read standard input: {error}"))
    })?;
    Ok(input)
}

/// A JSON object, or the reason it is not one.
fn object(text: &str) -> Result<Map<String, Value>, SeleniumBaseError> {
    match serde_json::from_str::<Value>(text) {
        Ok(Value::Object(map)) => Ok(map),
        Ok(_) => Err(SeleniumBaseError::invalid_config(
            "the arguments must be a JSON object, such as {\"url\": \"https://example.com\"}",
        )),
        Err(error) => Err(SeleniumBaseError::invalid_config(format!(
            "the arguments are not valid JSON: {error}"
        ))),
    }
}

fn split(pair: &str) -> Result<(String, String), SeleniumBaseError> {
    match pair.split_once('=') {
        Some((key, value)) if !key.trim().is_empty() => {
            Ok((key.trim().to_owned(), value.to_owned()))
        }
        _ => Err(SeleniumBaseError::invalid_config(format!(
            "--arg {pair:?} is not KEY=VALUE"
        ))),
    }
}

/// The `--arg KEY=VALUE` options as an object. A key given twice keeps the last value.
fn pairs(pairs: &[String]) -> Result<Map<String, Value>, SeleniumBaseError> {
    pairs
        .iter()
        .map(|pair| split(pair).map(|(key, value)| (key, value_of(&value))))
        .collect()
}

/// The value of an `--arg`: JSON when it parses as JSON, the text between single quotes, or
/// else the text itself.
fn value_of(text: &str) -> Value {
    if let Ok(value) = serde_json::from_str::<Value>(text) {
        return value;
    }
    if let Some(quoted) = text
        .strip_prefix('\'')
        .and_then(|rest| rest.strip_suffix('\''))
    {
        return Value::String(quoted.to_owned());
    }
    Value::String(text.to_owned())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn a_value_is_json_when_it_parses_as_json() {
        assert_eq!(value_of("true"), json!(true));
        assert_eq!(value_of("3"), json!(3));
        assert_eq!(value_of("\"text\""), json!("text"));
        assert_eq!(value_of("[1,2]"), json!([1, 2]));
        assert_eq!(value_of("{\"a\":1}"), json!({ "a": 1 }));
        assert_eq!(value_of("null"), Value::Null);
    }

    #[test]
    fn single_quotes_and_bare_words_are_strings() {
        assert_eq!(value_of("'true'"), json!("true"));
        assert_eq!(value_of("ref=e1"), json!("ref=e1"));
        assert_eq!(value_of("1.0.0"), json!("1.0.0"));
        assert_eq!(value_of("007"), json!("007"));
        assert_eq!(value_of(""), json!(""));
    }

    #[test]
    fn a_pair_splits_at_the_first_equals_sign() {
        let map = pairs(&["selector=ref=e1".to_owned(), "timeout=2.5".to_owned()]).unwrap();
        assert_eq!(map["selector"], json!("ref=e1"));
        assert_eq!(map["timeout"], json!(2.5));
    }

    #[test]
    fn a_pair_needs_a_key() {
        for bad in ["novalue", "=x", " =x"] {
            let error = pairs(&[bad.to_owned()]).unwrap_err();
            assert_eq!(error.exit_code(), 2, "{bad}: {error}");
        }
    }

    #[test]
    fn the_arguments_are_one_json_object() {
        assert_eq!(object("{\"a\":1}").unwrap()["a"], json!(1));
        for bad in ["[1]", "\"x\"", "{", ""] {
            assert_eq!(object(bad).unwrap_err().exit_code(), 2, "{bad}");
        }
    }

    #[test]
    fn a_dash_reads_the_arguments_from_standard_input() {
        let mut stdin: &[u8] = b"{\"x\": 7}";
        let text = text_or_stdin("-", &mut stdin).unwrap();
        assert_eq!(object(&text).unwrap()["x"], json!(7));
    }

    fn call(arguments: Option<&str>, json: Option<&str>, arg: &[&str]) -> CallArgs {
        CallArgs {
            tool: "t".to_owned(),
            arguments: arguments.map(str::to_owned),
            arg: arg.iter().map(|a| (*a).to_owned()).collect(),
            json: json.map(str::to_owned),
            raw: false,
            out: None,
            progress: false,
            connect: Connect::default(),
        }
    }

    #[test]
    fn the_arguments_come_one_way_only() {
        let mut stdin: &[u8] = b"";
        assert!(call(Some("{}"), None, &[]).arguments(&mut stdin).is_ok());
        assert!(call(None, Some("{}"), &[]).arguments(&mut stdin).is_ok());
        assert!(call(None, None, &["a=1"]).arguments(&mut stdin).is_ok());
        for both in [
            call(Some("{}"), Some("{}"), &[]),
            call(Some("{}"), None, &["a=1"]),
            call(None, Some("{}"), &["a=1"]),
        ] {
            assert_eq!(both.arguments(&mut stdin).unwrap_err().exit_code(), 2);
        }
    }

    #[test]
    fn a_prompt_argument_is_always_text() {
        let args = PromptArgs {
            name: "write_test".to_owned(),
            arg: vec!["goal=3".to_owned()],
            raw: false,
            connect: Connect::default(),
        };
        assert_eq!(args.arguments().unwrap()["goal"], json!("3"));
    }

    #[test]
    fn a_batch_is_a_list_of_calls() {
        let text = "# start\n\n{\"tool\":\"a\",\"arguments\":{\"x\":1}}\n  {\"tool\":\"b\"}  \n";
        let calls = batch_calls(text).unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].tool, "a");
        assert_eq!(calls[0].arguments["x"], json!(1));
        assert!(calls[1].arguments.is_empty());
    }

    #[test]
    fn a_bad_line_of_a_batch_is_named_by_its_number() {
        for (text, line) in [
            ("{\"tool\":\"a\"}\nnot json", "line 2"),
            ("{\"arguments\":{}}", "line 1"),
            ("{\"tool\":\"a\",\"arguments\":[1]}", "line 1"),
        ] {
            let error = batch_calls(text).unwrap_err();
            assert_eq!(error.exit_code(), 2);
            assert!(error.to_string().contains(line), "{text}: {error}");
        }
        assert_eq!(
            batch_calls("# only a comment\n").unwrap_err().exit_code(),
            2
        );
    }

    #[test]
    fn the_servers_that_may_be_chosen_are_exclusive() {
        let both = Connect {
            url: Some("http://127.0.0.1:1/mcp".to_owned()),
            server: Some(Profile::Dev),
            ..Connect::default()
        };
        assert_eq!(both.target().unwrap_err().exit_code(), 2);
        let orphan = Connect {
            connect_arg: vec!["--x".to_owned()],
            ..Connect::default()
        };
        assert_eq!(orphan.target().unwrap_err().exit_code(), 2);
        let token = Connect {
            token_file: Some("t".to_owned()),
            server: Some(Profile::Dev),
            ..Connect::default()
        };
        assert_eq!(token.target().unwrap_err().exit_code(), 2);
    }

    #[test]
    fn a_url_must_be_http() {
        let ftp = Connect {
            url: Some("ftp://x.test/mcp".to_owned()),
            ..Connect::default()
        };
        assert_eq!(ftp.target().unwrap_err().exit_code(), 2);
    }

    #[test]
    fn the_flags_choose_the_target() {
        let spawn = Connect {
            connect: Some("/bin/server".to_owned()),
            connect_arg: vec!["--stdio".to_owned()],
            ..Connect::default()
        }
        .target()
        .unwrap();
        assert_eq!(
            spawn,
            Target::Spawn {
                program: PathBuf::from("/bin/server"),
                args: vec!["--stdio".to_owned()],
            }
        );
        let own = Connect {
            server: Some(Profile::Cdp),
            ..Connect::default()
        }
        .target()
        .unwrap();
        let Target::Spawn { args, .. } = own else {
            panic!("a spawned server");
        };
        assert_eq!(args, ["--server", "cdp"]);
    }

    #[test]
    fn the_timeout_defaults_to_a_minute_and_cannot_be_zero() {
        assert_eq!(
            Connect::default().timeout().unwrap(),
            Duration::from_secs(60)
        );
        let zero = Connect {
            timeout: Some(0),
            ..Connect::default()
        };
        assert_eq!(zero.timeout().unwrap_err().exit_code(), 2);
    }
}
