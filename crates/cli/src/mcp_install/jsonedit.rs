//! Changing one entry of a JSON config by editing its text, so that the rest of the file keeps
//! its exact bytes: its comments-free layout, its inline arrays, its blank lines, its key order.
//!
//! The functions expect text that already parsed as plain JSON (no comments, no trailing
//! commas); they find where the members of an object start and end, and splice. A member is
//! added after the last one in the layout of the others (one per line, or inline), replaced
//! over the span of its value, or removed with its comma.

use serde_json::{Map, Value};

/// A member of an object: where its key and its value are in the text.
#[derive(Clone, Debug)]
struct Member {
    key: String,
    key_start: usize,
    value_start: usize,
    value_end: usize,
}

fn skip_white(bytes: &[u8], mut i: usize) -> usize {
    while i < bytes.len() && matches!(bytes[i], b' ' | b'\t' | b'\n' | b'\r') {
        i += 1;
    }
    i
}

/// The end (exclusive) of the string that opens at `i`.
fn string_end(bytes: &[u8], i: usize) -> usize {
    let mut j = i + 1;
    while j < bytes.len() {
        match bytes[j] {
            b'\\' => j += 2,
            b'"' => return j + 1,
            _ => j += 1,
        }
    }
    bytes.len()
}

/// The end (exclusive) of the value that starts at `i`.
fn value_end(bytes: &[u8], i: usize) -> usize {
    match bytes.get(i) {
        Some(b'"') => string_end(bytes, i),
        Some(b'{' | b'[') => {
            let mut depth = 0usize;
            let mut j = i;
            while j < bytes.len() {
                match bytes[j] {
                    b'"' => {
                        j = string_end(bytes, j);
                        continue;
                    }
                    b'{' | b'[' => depth += 1,
                    b'}' | b']' => {
                        depth = depth.saturating_sub(1);
                        if depth == 0 {
                            return j + 1;
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
            bytes.len()
        }
        _ => {
            let mut j = i;
            while j < bytes.len()
                && !matches!(bytes[j], b',' | b'}' | b']' | b' ' | b'\t' | b'\n' | b'\r')
            {
                j += 1;
            }
            j
        }
    }
}

/// The members of the object that opens at `open`, and the index of its closing brace; `None`
/// when `open` is not an object.
fn members(text: &str, open: usize) -> Option<(Vec<Member>, usize)> {
    let bytes = text.as_bytes();
    if bytes.get(open) != Some(&b'{') {
        return None;
    }
    let mut i = skip_white(bytes, open + 1);
    let mut out = Vec::new();
    loop {
        match bytes.get(i)? {
            b'}' => return Some((out, i)),
            b'"' => {}
            _ => return None,
        }
        let key_start = i;
        let key_end = string_end(bytes, i);
        let key: String = serde_json::from_str(text.get(key_start..key_end)?).ok()?;
        i = skip_white(bytes, key_end);
        if bytes.get(i) != Some(&b':') {
            return None;
        }
        i = skip_white(bytes, i + 1);
        let value_start = i;
        let end = value_end(bytes, i);
        out.push(Member {
            key,
            key_start,
            value_start,
            value_end: end,
        });
        i = skip_white(bytes, end);
        match bytes.get(i)? {
            b',' => i = skip_white(bytes, i + 1),
            b'}' => {}
            _ => return None,
        }
    }
}

/// The white space that starts the line `pos` is on.
fn leading_white(text: &str, pos: usize) -> &str {
    let start = text[..pos].rfind('\n').map_or(0, |n| n + 1);
    let line = &text[start..];
    let end = line
        .find(|c: char| c != ' ' && c != '\t')
        .unwrap_or(line.len());
    &line[..end]
}

/// The indent in front of `pos` when only white space separates it from the start of its
/// line, which is how a member written on a line of its own looks.
fn own_line_indent(text: &str, pos: usize) -> Option<&str> {
    let start = text[..pos].rfind('\n').map_or(0, |n| n + 1);
    let lead = &text[start..pos];
    lead.chars().all(|c| c == ' ' || c == '\t').then_some(lead)
}

fn splice(text: &str, from: usize, to: usize, with: &str) -> String {
    let mut out = String::with_capacity(text.len() + with.len());
    out.push_str(&text[..from]);
    out.push_str(with);
    out.push_str(&text[to..]);
    out
}

/// `value` as pretty JSON whose lines after the first start with `indent`, which is what a
/// value needs to sit in a member that is `indent` deep.
fn render(value: &Value, unit: &str, indent: &str) -> String {
    pretty(value, unit)
        .lines()
        .enumerate()
        .map(|(i, line)| {
            if i == 0 {
                line.to_owned()
            } else {
                format!("{indent}{line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// `value` on one line, for a file that keeps its members inline.
fn compact(value: &Value) -> String {
    value.to_string()
}

/// Pretty JSON with `unit` as the indent. Arrays of plain values stay on one line, the way
/// people write `args`.
pub fn pretty(value: &Value, unit: &str) -> String {
    let mut out = String::new();
    write_value(&mut out, value, unit, 0);
    out
}

fn write_value(out: &mut String, value: &Value, unit: &str, depth: usize) {
    match value {
        Value::Object(map) => write_object(out, map, unit, depth),
        Value::Array(items) if items.iter().any(|item| item.is_array() || item.is_object()) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push('\n');
                push_indent(out, unit, depth + 1);
                write_value(out, item, unit, depth + 1);
            }
            out.push('\n');
            push_indent(out, unit, depth);
            out.push(']');
        }
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(&item.to_string());
            }
            out.push(']');
        }
        scalar => out.push_str(&scalar.to_string()),
    }
}

fn write_object(out: &mut String, map: &Map<String, Value>, unit: &str, depth: usize) {
    if map.is_empty() {
        out.push_str("{}");
        return;
    }
    out.push('{');
    for (i, (key, value)) in map.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push('\n');
        push_indent(out, unit, depth + 1);
        out.push_str(&Value::String(key.clone()).to_string());
        out.push_str(": ");
        write_value(out, value, unit, depth + 1);
    }
    out.push('\n');
    push_indent(out, unit, depth);
    out.push('}');
}

fn push_indent(out: &mut String, unit: &str, depth: usize) {
    for _ in 0..depth {
        out.push_str(unit);
    }
}

/// Adds the member `key: value` to the object that opens at `open`, last, in the layout of
/// the members it has.
fn insert_member(text: &str, open: usize, key: &str, value: &Value, unit: &str) -> Option<String> {
    let (list, close) = members(text, open)?;
    let key_json = Value::String(key.to_owned()).to_string();
    let Some(last) = list.last() else {
        // An empty object grows one member per line, one level in from its own line.
        let closing = leading_white(text, open);
        let inner = format!("{closing}{unit}");
        let rendered = render(value, unit, &inner);
        let body = format!("\n{inner}{key_json}: {rendered}\n{closing}");
        return Some(splice(text, open + 1, close, &body));
    };
    let added = match own_line_indent(text, list[0].key_start) {
        Some(indent) => format!(",\n{indent}{key_json}: {}", render(value, unit, indent)),
        None => format!(", {key_json}: {}", compact(value)),
    };
    Some(splice(text, last.value_end, last.value_end, &added))
}

/// The object that holds the servers: where it opens and its members.
struct Servers {
    open: usize,
    list: Vec<Member>,
}

/// Where the root object opens, and the object under `key` (`None` inside when the root has no
/// such key). `None` when `text` is not an object, or `key` holds something other than an
/// object.
fn find_servers(text: &str, key: &str) -> Option<(usize, Option<Servers>)> {
    let root = skip_white(text.as_bytes(), 0);
    let (root_members, _) = members(text, root)?;
    let servers = match root_members.iter().rposition(|m| m.key == key) {
        None => None,
        Some(i) => {
            let open = root_members[i].value_start;
            let (list, _) = members(text, open)?;
            Some(Servers { open, list })
        }
    };
    Some((root, servers))
}

/// Puts `entry` under `name` in the object `key` of the root object of `text`: replaces the
/// entry that is there, or adds it last, or adds the object `key` too when the root has none.
///
/// `None` when `text` is not a JSON object with an object under `key`.
pub fn upsert(text: &str, key: &str, name: &str, entry: &Value, unit: &str) -> Option<String> {
    let (root, servers) = find_servers(text, key)?;
    match servers {
        None => {
            let mut object = Map::new();
            object.insert(name.to_owned(), entry.clone());
            insert_member(text, root, key, &Value::Object(object), unit)
        }
        Some(Servers { open, list }) => match list.iter().rposition(|m| m.key == name) {
            Some(i) => {
                let member = &list[i];
                let rendered = match own_line_indent(text, member.key_start) {
                    Some(indent) => render(entry, unit, indent),
                    None => compact(entry),
                };
                Some(splice(
                    text,
                    member.value_start,
                    member.value_end,
                    &rendered,
                ))
            }
            None => insert_member(text, open, name, entry, unit),
        },
    }
}

/// Removes the entry `name` from the object `key` of the root object of `text`, with its
/// comma. The text is returned as it was when there is no such entry.
///
/// `None` when `text` is not a JSON object with an object under `key`.
pub fn remove(text: &str, key: &str, name: &str) -> Option<String> {
    let (_, servers) = find_servers(text, key)?;
    let Some(Servers { open, list }) = servers else {
        return Some(text.to_owned());
    };
    let Some(i) = list.iter().rposition(|m| m.key == name) else {
        return Some(text.to_owned());
    };
    let (_, close) = members(text, open)?;
    Some(if list.len() == 1 {
        splice(text, open + 1, close, "")
    } else if i > 0 {
        splice(text, list[i - 1].value_end, list[i].value_end, "")
    } else {
        splice(text, list[0].key_start, list[1].key_start, "")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn entry() -> Value {
        json!({ "command": "/x/sbrs-mcp", "args": ["--server", "cdp"] })
    }

    fn parse(text: &str) -> Value {
        serde_json::from_str(text).unwrap()
    }

    #[test]
    fn an_entry_is_added_after_the_last_one_in_the_files_layout() {
        let text = "{\n    \"a\": 1,\n    \"servers\": {\n        \"x\": {\"command\": \"y\", \"args\": []}\n    },\n    \"z\": [1, 2]\n}\n";
        let out = upsert(text, "servers", "n", &entry(), "    ").unwrap();
        assert_eq!(
            out,
            "{\n    \"a\": 1,\n    \"servers\": {\n        \"x\": {\"command\": \"y\", \"args\": []},\n        \
             \"n\": {\n            \"command\": \"/x/sbrs-mcp\",\n            \"args\": [\"--server\", \"cdp\"]\n        }\n    },\n    \"z\": [1, 2]\n}\n"
        );
        assert_eq!(parse(&out)["servers"]["n"], entry());
    }

    #[test]
    fn an_inline_file_stays_inline() {
        let text = r#"{"servers": {"x": 1}, "z": 2}"#;
        let out = upsert(text, "servers", "n", &entry(), "  ").unwrap();
        assert_eq!(
            out,
            r#"{"servers": {"x": 1, "n": {"command":"/x/sbrs-mcp","args":["--server","cdp"]}}, "z": 2}"#
        );
    }

    #[test]
    fn an_empty_object_grows_one_member_per_line() {
        let text = "{\n  \"servers\": {}\n}";
        let out = upsert(text, "servers", "n", &entry(), "  ").unwrap();
        assert_eq!(
            out,
            "{\n  \"servers\": {\n    \"n\": {\n      \"command\": \"/x/sbrs-mcp\",\n      \"args\": [\"--server\", \"cdp\"]\n    }\n  }\n}"
        );
        let out = upsert("{}", "servers", "n", &entry(), "\t").unwrap();
        assert_eq!(
            out,
            "{\n\t\"servers\": {\n\t\t\"n\": {\n\t\t\t\"command\": \"/x/sbrs-mcp\",\n\t\t\t\"args\": [\"--server\", \"cdp\"]\n\t\t}\n\t}\n}"
        );
    }

    #[test]
    fn a_missing_servers_object_is_added_last_in_the_root() {
        let text = "{\n  \"theme\": \"dark\"\n}\n";
        let out = upsert(text, "servers", "n", &entry(), "  ").unwrap();
        assert!(
            out.starts_with("{\n  \"theme\": \"dark\",\n  \"servers\": {\n    \"n\": {"),
            "{out}"
        );
        assert!(out.ends_with("}\n  }\n}\n"), "{out}");
        assert_eq!(parse(&out)["servers"]["n"], entry());
    }

    #[test]
    fn an_entry_that_is_there_is_replaced_over_its_own_span() {
        let text = "{\n  \"servers\": {\n    \"first\": [1,2],\n    \"n\": {\"old\": true},\n    \"last\": \"}\"\n  }\n}\n";
        let out = upsert(text, "servers", "n", &entry(), "  ").unwrap();
        assert!(out.contains("\"first\": [1,2],"), "{out}");
        assert!(out.contains("\"last\": \"}\"\n  }\n}\n"), "{out}");
        assert_eq!(parse(&out)["servers"]["n"], entry());
        assert!(!out.contains("old"));
    }

    #[test]
    fn braces_and_quotes_inside_strings_do_not_confuse_the_scan() {
        let text = r#"{"servers": {"a": "}{\"", "b": ["]", "[", {"c": "\\"}]}, "after": 1}"#;
        let out = upsert(text, "servers", "n", &entry(), "  ").unwrap();
        let value = parse(&out);
        assert_eq!(value["servers"]["a"], "}{\"");
        assert_eq!(value["servers"]["b"][2]["c"], "\\");
        assert_eq!(value["after"], 1);
        assert_eq!(value["servers"]["n"], entry());
    }

    #[test]
    fn removing_the_first_a_middle_and_the_last_entry() {
        let text = "{\n  \"servers\": {\n    \"a\": 1,\n    \"b\": {\"x\": [1, 2]},\n    \"c\": 3\n  },\n  \"z\": 0\n}\n";
        assert_eq!(
            remove(text, "servers", "a").unwrap(),
            "{\n  \"servers\": {\n    \"b\": {\"x\": [1, 2]},\n    \"c\": 3\n  },\n  \"z\": 0\n}\n"
        );
        assert_eq!(
            remove(text, "servers", "b").unwrap(),
            "{\n  \"servers\": {\n    \"a\": 1,\n    \"c\": 3\n  },\n  \"z\": 0\n}\n"
        );
        assert_eq!(
            remove(text, "servers", "c").unwrap(),
            "{\n  \"servers\": {\n    \"a\": 1,\n    \"b\": {\"x\": [1, 2]}\n  },\n  \"z\": 0\n}\n"
        );
    }

    #[test]
    fn removing_the_only_entry_leaves_an_empty_object() {
        let text = "{\n  \"servers\": {\n    \"a\": 1\n  }\n}";
        assert_eq!(
            remove(text, "servers", "a").unwrap(),
            "{\n  \"servers\": {}\n}"
        );
    }

    #[test]
    fn removing_what_is_not_there_changes_nothing() {
        let text = "{\"servers\": {\"a\": 1}}";
        assert_eq!(remove(text, "servers", "b").unwrap(), text);
        assert_eq!(remove(text, "other", "a").unwrap(), text);
    }

    #[test]
    fn text_that_is_not_an_object_is_not_edited() {
        assert!(upsert("[1]", "servers", "n", &entry(), "  ").is_none());
        assert!(upsert(r#"{"servers": []}"#, "servers", "n", &entry(), "  ").is_none());
        assert!(remove("[1]", "servers", "n").is_none());
    }

    #[test]
    fn pretty_keeps_arrays_of_plain_values_on_one_line() {
        let value = json!({ "a": [1, "b", null], "c": [{"d": 1}], "e": [], "f": {} });
        assert_eq!(
            pretty(&value, "  "),
            "{\n  \"a\": [1, \"b\", null],\n  \"c\": [\n    {\n      \"d\": 1\n    }\n  ],\n  \"e\": [],\n  \"f\": {}\n}"
        );
    }
}
