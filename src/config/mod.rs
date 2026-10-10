pub mod ad_block_list;
pub mod proxy_list;
pub mod runtime;
pub mod settings;

pub use runtime::{LogFormat, RuntimeConfig};

/// The line without a trailing `# comment`.
///
/// A `#` starts a comment only at the start of the line or after whitespace;
/// inside a word, as in the cosmetic rule `example.net##.advert`, it does not.
pub(crate) fn strip_comment(line: &str) -> &str {
    let mut after_space = true;
    for (index, c) in line.char_indices() {
        if c == '#' && after_space {
            return &line[..index];
        }
        after_space = c.is_whitespace();
    }
    line
}

#[cfg(test)]
mod tests {
    use super::strip_comment;

    #[test]
    fn a_comment_starts_at_a_hash_after_whitespace() {
        assert_eq!(strip_comment("host:80 # office"), "host:80 ");
        assert_eq!(strip_comment("# whole line"), "");
        assert_eq!(strip_comment("host:80\t#x"), "host:80\t");
    }

    #[test]
    fn a_hash_inside_a_word_is_not_a_comment() {
        assert_eq!(
            strip_comment("example.net##.advert"),
            "example.net##.advert"
        );
        assert_eq!(strip_comment("user:p#ss@host:80"), "user:p#ss@host:80");
    }

    #[test]
    fn a_line_without_a_hash_is_unchanged() {
        assert_eq!(strip_comment("plain"), "plain");
        assert_eq!(strip_comment(""), "");
    }
}
