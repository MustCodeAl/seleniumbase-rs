//! Element references: a short name for an element a page snapshot listed.
//!
//! `page_snapshot` gives the interactive nodes of a page refs such as `e5`.
//! The server keeps, for the last snapshot, the selector that finds each
//! element and the role and name it had. A tool argument that holds a selector
//! may then be written `ref=e5`: the host swaps in the stored selector, after
//! checking in the page that the element still has the role and name it had,
//! so a click never lands on something else that took its place.
//!
//! The table lives on the server, not in the page: a page global would be a
//! mark that a script could look for.

use std::future::Future;
use std::pin::Pin;

use super::host::Ctx;
use super::tool::ToolError;

/// What an argument starts with when it names a ref instead of a selector.
const PREFIX: &str = "ref=";

/// One element of the last snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RefEntry {
    /// The ref, such as `e5`.
    pub(crate) id: String,
    /// A selector that finds exactly this element; `host ::shadow inner` for
    /// an element inside an open shadow root.
    pub(crate) selector: String,
    /// The ARIA role the element had in the snapshot.
    pub(crate) role: String,
    /// Its accessible name.
    pub(crate) name: String,
}

/// The refs of the last `page_snapshot`, and the last text the page tools
/// read, which the `sbrs://page/...` resources serve.
#[derive(Debug, Default)]
pub(crate) struct RefTable {
    snapshot: u64,
    url: String,
    entries: Vec<RefEntry>,
    snapshot_text: Option<String>,
    page_text: Option<String>,
}

impl RefTable {
    /// Forgets everything: the browser it described is gone.
    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }

    /// The number of the last snapshot; 0 before the first.
    #[cfg(test)]
    pub(crate) fn snapshot(&self) -> u64 {
        self.snapshot
    }

    /// The URL of the page the last snapshot was taken of.
    pub(crate) fn url(&self) -> &str {
        &self.url
    }

    /// Replaces the table with a new snapshot of `url`.
    ///
    /// `render` is given the number of the new snapshot and returns the text
    /// that is kept for the `sbrs://page/snapshot` resource; the same text is
    /// returned.
    pub(crate) fn replace(
        &mut self,
        url: String,
        entries: Vec<RefEntry>,
        render: impl FnOnce(u64) -> String,
    ) -> String {
        self.snapshot += 1;
        self.url = url;
        self.entries = entries;
        let text = render(self.snapshot);
        self.snapshot_text = Some(text.clone());
        text
    }

    /// Adds one element to the table, after the ones it has, and returns its
    /// ref. Tools that list elements without a snapshot use it, so their refs
    /// work until the next `page_snapshot`.
    pub(crate) fn push(
        &mut self,
        url: &str,
        selector: String,
        role: String,
        name: String,
    ) -> String {
        if self.snapshot == 0 {
            self.snapshot = 1;
            url.clone_into(&mut self.url);
        }
        let id = format!("e{}", self.entries.len() + 1);
        self.entries.push(RefEntry {
            id: id.clone(),
            selector,
            role,
            name,
        });
        id
    }

    /// The text of the last `page_snapshot`.
    pub(crate) fn snapshot_text(&self) -> Option<&str> {
        self.snapshot_text.as_deref()
    }

    /// Keeps the text `read_page_text` returned last.
    pub(crate) fn set_page_text(&mut self, text: String) {
        self.page_text = Some(text);
    }

    /// The text of the last `read_page_text`.
    pub(crate) fn page_text(&self) -> Option<&str> {
        self.page_text.as_deref()
    }

    /// The entry for `id`.
    ///
    /// # Errors
    ///
    /// Returns what to tell the model when no such ref exists: which refs the
    /// last snapshot has, or that there was none.
    pub(crate) fn get(&self, id: &str) -> Result<&RefEntry, String> {
        if let Some(entry) = self.entries.iter().find(|entry| entry.id == id) {
            return Ok(entry);
        }
        Err(match (self.entries.first(), self.entries.last()) {
            (Some(first), Some(last)) => format!(
                "unknown ref {id}; the last snapshot ({}) has {}..{}",
                self.snapshot, first.id, last.id
            ),
            _ if self.snapshot == 0 => {
                format!("unknown ref {id}; no page_snapshot has been taken yet, call it first")
            }
            _ => format!(
                "unknown ref {id}; the last snapshot ({}) has no refs",
                self.snapshot
            ),
        })
    }
}

/// Whether an argument called `name` holds a selector, and so may be a ref.
///
/// These are the selector-holding names of the existing tools: `selector`,
/// `scope`, and every `*_selector` (`dropdown_selector`, `parent_selector`,
/// `drag_selector`, ...).
pub(crate) fn is_selector_arg(name: &str) -> bool {
    name == "selector" || name == "scope" || name.ends_with("_selector")
}

/// The id in an argument written `ref=e5`, or `None` when it is a selector.
pub(crate) fn parse_ref(value: &str) -> Option<&str> {
    value.trim().strip_prefix(PREFIX).map(str::trim)
}

/// What the page said about the element a ref points to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RefProbe {
    /// How many elements the stored selector matches now.
    pub(crate) count: usize,
    /// The role of the first match.
    pub(crate) role: String,
    /// The accessible name of the first match.
    pub(crate) name: String,
    /// The page's URL now.
    pub(crate) url: String,
    /// Set when this engine cannot select the element at all.
    pub(crate) unsupported: Option<String>,
}

/// The future a [`RefVerify`] returns.
pub(crate) type RefFuture = Pin<Box<dyn Future<Output = Result<RefProbe, ToolError>> + Send>>;

/// Looks in the page for the element of a ref. Set on a host with
/// `Host::with_refs`.
pub(crate) type RefVerify<S> = fn(Ctx<S>, RefEntry) -> RefFuture;

/// `button "Save"`, or `button` when the element has no name.
fn describe(role: &str, name: &str) -> String {
    if name.is_empty() {
        role.to_owned()
    } else {
        format!("{role} \"{name}\"")
    }
}

/// Why `entry` cannot be used, given what the page says now; `None` when the
/// element is still there with the same role and name.
///
/// `snapshot_url` is the page the snapshot was taken of.
pub(crate) fn problem(entry: &RefEntry, probe: &RefProbe, snapshot_url: &str) -> Option<String> {
    let id = &entry.id;
    if let Some(reason) = &probe.unsupported {
        return Some(format!("ref {id} cannot be used: {reason}"));
    }
    let was = describe(&entry.role, &entry.name);
    if probe.count == 0 {
        let moved = if probe.url == snapshot_url || snapshot_url.is_empty() {
            String::new()
        } else {
            format!(
                "; the page is now {}, the snapshot was of {snapshot_url}",
                probe.url
            )
        };
        return Some(format!(
            "ref {id} is stale: no element matches any more (it was {was}){moved}; call \
             page_snapshot again"
        ));
    }
    if probe.role != entry.role || probe.name != entry.name {
        return Some(format!(
            "ref {id} is stale: the element is now {}, it was {was}; call page_snapshot again",
            describe(&probe.role, &probe.name)
        ));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, role: &str, name: &str) -> RefEntry {
        RefEntry {
            id: id.to_owned(),
            selector: format!("#{id}"),
            role: role.to_owned(),
            name: name.to_owned(),
        }
    }

    fn probe(count: usize, role: &str, name: &str) -> RefProbe {
        RefProbe {
            count,
            role: role.to_owned(),
            name: name.to_owned(),
            url: "https://a.test/".to_owned(),
            unsupported: None,
        }
    }

    fn table() -> RefTable {
        let mut table = RefTable::default();
        table.replace(
            "https://a.test/".to_owned(),
            vec![
                entry("e1", "form", "Login"),
                entry("e2", "textbox", "Email"),
            ],
            |n| format!("snapshot {n}"),
        );
        table
    }

    #[test]
    fn a_new_snapshot_replaces_the_refs_and_counts_up() {
        let mut table = table();
        assert_eq!(table.snapshot(), 1);
        assert_eq!(table.get("e2").unwrap().name, "Email");

        let text = table.replace(
            "https://b.test/".to_owned(),
            vec![entry("e1", "link", "Home")],
            |n| format!("snapshot {n}"),
        );

        assert_eq!(text, "snapshot 2");
        assert_eq!(table.snapshot_text(), Some("snapshot 2"));
        assert_eq!(table.url(), "https://b.test/");
        assert!(table.get("e2").is_err(), "the old refs are gone");
        assert_eq!(table.get("e1").unwrap().role, "link");
    }

    #[test]
    fn an_unknown_ref_says_which_refs_exist() {
        let table = table();
        assert_eq!(
            table.get("e9").unwrap_err(),
            "unknown ref e9; the last snapshot (1) has e1..e2"
        );
        assert_eq!(
            RefTable::default().get("e1").unwrap_err(),
            "unknown ref e1; no page_snapshot has been taken yet, call it first"
        );
    }

    #[test]
    fn clearing_forgets_the_snapshot_and_the_texts() {
        let mut table = table();
        table.set_page_text("text".to_owned());
        table.clear();
        assert_eq!(table.snapshot(), 0);
        assert!(table.snapshot_text().is_none());
        assert!(table.page_text().is_none());
        assert!(table.get("e1").is_err());
    }

    #[test]
    fn only_selector_arguments_can_hold_a_ref() {
        for name in [
            "selector",
            "scope",
            "dropdown_selector",
            "parent_selector",
            "drag_selector",
            "drop_selector",
            "hover_selector",
            "click_selector",
            "secondary_selector",
        ] {
            assert!(is_selector_arg(name), "{name}");
        }
        for name in ["text", "url", "script", "selector_kind", "value"] {
            assert!(!is_selector_arg(name), "{name}");
        }
    }

    #[test]
    fn a_ref_argument_is_told_from_a_selector() {
        assert_eq!(parse_ref("ref=e5"), Some("e5"));
        assert_eq!(parse_ref(" ref= e12 "), Some("e12"));
        assert_eq!(parse_ref("#ref=e5"), None);
        assert_eq!(parse_ref("button.primary"), None);
    }

    #[test]
    fn a_ref_whose_element_is_unchanged_is_fine() {
        let entry = entry("e2", "textbox", "Email");
        assert_eq!(
            problem(&entry, &probe(1, "textbox", "Email"), "https://a.test/"),
            None
        );
    }

    #[test]
    fn a_ref_whose_element_changed_names_both() {
        let entry = entry("e5", "textbox", "Email");
        assert_eq!(
            problem(&entry, &probe(1, "button", "Save"), "https://a.test/").unwrap(),
            "ref e5 is stale: the element is now button \"Save\", it was textbox \"Email\"; call \
             page_snapshot again"
        );
    }

    #[test]
    fn a_ref_whose_element_is_gone_says_so_and_notes_a_moved_page() {
        let entry = entry("e5", "button", "Save");
        let mut gone = probe(0, "", "");
        assert_eq!(
            problem(&entry, &gone, "https://a.test/").unwrap(),
            "ref e5 is stale: no element matches any more (it was button \"Save\"); call \
             page_snapshot again"
        );
        gone.url = "https://a.test/next".to_owned();
        let message = problem(&entry, &gone, "https://a.test/").unwrap();
        assert!(
            message.contains(
                "the page is now https://a.test/next, the snapshot was of https://a.test/"
            ),
            "{message}"
        );
    }

    #[test]
    fn an_unsupported_ref_gives_the_engines_reason() {
        let entry = entry("e7", "button", "Go");
        let mut probe = probe(0, "", "");
        probe.unsupported = Some("it is inside a shadow root".to_owned());
        assert_eq!(
            problem(&entry, &probe, "").unwrap(),
            "ref e7 cannot be used: it is inside a shadow root"
        );
    }
}
