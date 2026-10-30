//! Edits of a source text, and the lines of it.

/// Replaces `start..end` of the source with `text`; an empty range inserts.
#[derive(Clone, Debug)]
pub(super) struct Edit {
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) text: String,
}

/// Applies edits that do not overlap. An insertion at the end of a replacement comes after it.
pub(super) fn apply(source: &str, mut edits: Vec<Edit>) -> String {
    edits.sort_by_key(|edit| (edit.start, edit.end));
    let mut result = String::with_capacity(source.len());
    let mut cursor = 0;
    for edit in edits {
        if let Some(kept) = source.get(cursor..edit.start) {
            result.push_str(kept);
            result.push_str(&edit.text);
            cursor = edit.end;
        }
    }
    result.push_str(source.get(cursor..).unwrap_or(""));
    result
}

/// The start of the line that holds byte `position`.
pub(super) fn line_start(source: &str, position: usize) -> usize {
    source
        .get(..position)
        .and_then(|before| before.rfind('\n'))
        .map_or(0, |at| at + 1)
}

/// The end of the line that holds byte `position`: the line break, or the end of the source.
pub(super) fn line_end(source: &str, position: usize) -> usize {
    source
        .get(position..)
        .and_then(|after| after.find('\n'))
        .map_or(source.len(), |at| position + at)
}

/// Where the text of the line that ends at `line_end` ends: before the `\r` of a `\r\n`.
pub(super) fn text_end(source: &str, line_end: usize) -> usize {
    if line_end > 0 && source.as_bytes().get(line_end - 1) == Some(&b'\r') {
        line_end - 1
    } else {
        line_end
    }
}

/// The line break the source uses: `\r\n` when its first line break is one, else `\n`.
pub(super) fn line_ending(source: &str) -> &'static str {
    match source.find('\n') {
        Some(at) if at > 0 && source.as_bytes()[at - 1] == b'\r' => "\r\n",
        _ => "\n",
    }
}
