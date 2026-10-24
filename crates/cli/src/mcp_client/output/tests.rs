use super::*;
use seleniumbase_rs::mcp::client::PromptArgSummary;

fn tool(name: &str, description: &str) -> ToolSummary {
    ToolSummary {
        name: name.to_owned(),
        title: None,
        description: Some(description.to_owned()),
        input_schema: json!({ "type": "object" }),
        output_schema: None,
    }
}

fn outcome(text: &str, structured: Option<Value>) -> CallOutcome {
    CallOutcome {
        is_error: false,
        text: text.to_owned(),
        structured,
        images: Vec::new(),
    }
}

fn written(write: impl FnOnce(&mut Vec<u8>) -> io::Result<()>) -> String {
    let mut out = Vec::new();
    write(&mut out).expect("writing to memory works");
    String::from_utf8(out).expect("text")
}

#[test]
fn tools_are_a_table_of_names_and_first_lines() {
    let tools = [
        tool("open_url", "Opens a page.\nMore text."),
        tool("go", "Goes."),
    ];

    let table = written(|out| write_tools(&tools, false, out));

    assert_eq!(table, "open_url  Opens a page.\ngo        Goes.\n");
}

#[test]
fn raw_tools_are_one_json_array() {
    let table = written(|out| write_tools(&[tool("go", "Goes.")], true, out));

    let parsed: Value = serde_json::from_str(table.trim()).expect("JSON");
    assert_eq!(parsed[0]["name"], "go");
    assert_eq!(parsed[0]["inputSchema"], json!({ "type": "object" }));
    assert_eq!(table.matches('\n').count(), 1, "one line: {table}");
}

#[test]
fn one_tool_shows_its_schema() {
    let shown = written(|out| write_tool(&tool("go", "Goes."), false, out));

    assert!(shown.starts_with("go\n\nGoes.\n"), "{shown}");
    assert!(shown.contains("\"type\": \"object\""), "{shown}");
}

#[test]
fn a_call_prints_its_text_with_one_line_break_at_the_end() {
    assert_eq!(
        written(|out| write_outcome(&outcome("done", None), false, out)),
        "done\n"
    );
    assert_eq!(
        written(|out| write_outcome(&outcome("done\n", None), false, out)),
        "done\n"
    );
}

#[test]
fn raw_prints_the_structured_result_as_compact_json() {
    let with = outcome("text copy", Some(json!({ "a": [1, 2] })));
    let without = outcome("only text", None);

    assert_eq!(
        written(|out| write_outcome(&with, true, out)),
        "{\"a\":[1,2]}\n"
    );
    assert_eq!(
        written(|out| write_outcome(&without, true, out)),
        "only text\n"
    );
}

#[test]
fn a_batch_line_carries_the_tool_the_flag_and_both_results() {
    let mut failed = outcome("went wrong", Some(json!({ "kind": "failed" })));
    failed.is_error = true;

    let text = written(|out| write_outcome_json("go", &failed, out));

    let parsed: Value = serde_json::from_str(text.trim()).expect("JSON");
    assert_eq!(parsed["tool"], "go");
    assert_eq!(parsed["isError"], true);
    assert_eq!(parsed["structured"]["kind"], "failed");
}

#[test]
fn images_are_saved_in_order_and_numbered_across_calls() {
    let dir = tempfile::tempdir().unwrap();
    let mut shot = outcome("shot", None);
    shot.images = vec![
        ("image/png".to_owned(), vec![1, 2, 3]),
        ("image/jpeg".to_owned(), vec![4]),
    ];
    let mut counter = 0;

    let first = save_images(&shot, dir.path(), &mut counter).unwrap();
    let second = save_images(&shot, dir.path(), &mut counter).unwrap();

    let names = |paths: &[PathBuf]| -> Vec<String> {
        paths
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect()
    };
    assert_eq!(names(&first), ["image-1.png", "image-2.jpg"]);
    assert_eq!(names(&second), ["image-3.png", "image-4.jpg"]);
    assert_eq!(fs::read(&first[0]).unwrap(), [1, 2, 3]);
}

#[test]
fn a_result_without_images_creates_no_folder() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("not-made");

    let saved = save_images(&outcome("x", None), &target, &mut 0).unwrap();

    assert!(saved.is_empty());
    assert!(!target.exists());
}

#[test]
fn progress_is_a_short_line() {
    let line = |current, total, message: Option<&str>| {
        progress_line(&ClientProgress {
            current,
            total,
            message: message.map(str::to_owned),
        })
    };

    assert_eq!(line(2.0, Some(3.0), Some("two")), "progress: 2/3 two");
    assert_eq!(line(1.5, None, None), "progress: 1.5");
}

#[test]
fn binary_resources_are_saved_only_when_asked() {
    let dir = tempfile::tempdir().unwrap();
    let pieces = [ResourceContent {
        uri: "test://bytes".to_owned(),
        mime_type: Some("image/png".to_owned()),
        text: None,
        blob: Some(vec![9, 9]),
    }];
    let (mut out, mut err) = (Vec::new(), Vec::new());

    write_contents(&pieces, None, &mut out, &mut err).unwrap();
    let hint = String::from_utf8(err.clone()).unwrap();
    write_contents(&pieces, Some(dir.path()), &mut out, &mut err).unwrap();

    assert!(out.is_empty(), "binary never goes to standard output");
    assert!(hint.contains("--out DIR saves it"), "{hint}");
    assert_eq!(fs::read(dir.path().join("resource-1.png")).unwrap(), [9, 9]);
}

#[test]
fn text_resources_are_printed() {
    let pieces = [ResourceContent {
        uri: "test://note".to_owned(),
        mime_type: Some("text/plain".to_owned()),
        text: Some("hello".to_owned()),
        blob: None,
    }];
    let (mut out, mut err) = (Vec::new(), Vec::new());

    write_contents(&pieces, None, &mut out, &mut err).unwrap();

    assert_eq!(String::from_utf8(out).unwrap(), "hello\n");
    assert!(err.is_empty());
}

#[test]
fn prompts_are_listed_with_required_arguments_starred() {
    let prompts = [PromptSummary {
        name: "write_test".to_owned(),
        title: None,
        description: Some("Write a test.\nLong.".to_owned()),
        arguments: vec![
            PromptArgSummary {
                name: "url".to_owned(),
                description: None,
                required: true,
            },
            PromptArgSummary {
                name: "goal".to_owned(),
                description: None,
                required: false,
            },
        ],
    }];

    let table = written(|out| write_prompts(&prompts, false, out));

    assert_eq!(table, "write_test  (url*, goal)  Write a test.\n");
}

#[test]
fn a_prompt_prints_each_message_under_its_role() {
    let prompt = PromptOutcome {
        description: None,
        messages: vec![("user".to_owned(), "Do it.".to_owned())],
    };

    assert_eq!(
        written(|out| write_prompt(&prompt, false, out)),
        "[user]\nDo it.\n"
    );
}

#[test]
fn resources_mark_their_templates() {
    let resources = [ResourceSummary {
        uri: "sbrs://output/{+path}".to_owned(),
        name: "output".to_owned(),
        title: None,
        description: Some("Files.".to_owned()),
        mime_type: None,
        template: true,
    }];

    let table = written(|out| write_resources(&resources, false, out));

    assert_eq!(table, "sbrs://output/{+path}  output (template)  Files.\n");
}
