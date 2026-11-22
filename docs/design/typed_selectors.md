---
title: "Design: typed selectors (F9)"
sidebarTitle: "Typed selectors"
---


# Typed selectors (F9)

Design for lane `typed-selectors`: the selector half of finding F9 of
`docs/design/api_guidelines_audit.md` (line 724; the lane list entry is item 19, line 1246), plus the
two smaller halves of F9 (booleans, paths; item 13, line 1219) and where they fit. Written at
`origin/main` 32047a6f. Line numbers are of that commit.

## Decisions

1. **No new selector type.** `Selector` (owned) is the one selector type and `SelectorRef<'a>` its
   borrowed view, the `PathBuf`/`Path` pair, both in `src/utils/selectors.rs`. They already exist,
   already auto-detect a plain string, and the Pure CDP engine, `BrowserSession` and `Element` already
   take `impl Into<Selector>`. The lane moves the rest of the crate onto the type that is there.
2. **Parameters.** Every public method that looks an element up or stores a selector takes
   `impl Into<Selector>`; crate-internal helpers that only read take `&Selector`. A string is
   therefore read as a selector once per call, at the public boundary (today up to three times).
   `sb.click("#id")` keeps compiling (POLA).
3. **Conversions.** `From<&str>`, `From<String>`, `From<&String>` and `From<SelectorRef<'_>>` stay
   infallible (auto-detection, the Python-familiar form). New: `FromStr` (the strict form: `From` plus
   `check`, for text that crosses an external boundary: MCP arguments, command line, saved files) and
   `Deserialize` (mirror of the existing `Serialize`). No `TryFrom<&str>`: the infallible `From` makes
   the blanket `TryFrom` exist, so a fallible one cannot coexist.
4. **Error.** `SeleniumBaseError::InvalidSelector(String)` (`src/error.rs:44`), already used for an
   empty selector and a selector that ends on `::shadow`. No new variant. One function,
   `Selector::check`, holds the rule; today it is written twice (`SelectorRef::to_by`,
   `src/utils/selectors.rs:270-279`, and `by_of`, `src/browser/shadow_find.rs:152-158`).
5. **One list of kinds.** The variants of `Selector` are the closed list (CSS, XPath, link text,
   partial link text); the only code that reads prefixes is `SelectorRef::auto` (`:210`). Every `match` on
   a kind becomes exhaustive (no `_` arm), so a new kind fails to compile where it is not handled.
   `#[non_exhaustive]` on `Selector`, so a kind can be added without a major version.
6. **The `:contains()` rewrite moves from construction to lookup.** Today `Selector::from(&str)`
   turns `p:contains("x")` into XPath at once (`:637-644`), and `SelectorRef::to_by` does the same again
   (`:264-269`): two copies. After the lane a `Selector` keeps the text as written, and one
   method, `lookup_form`, makes the form the engines send. This keeps `Display`, the recorder and
   generated test code on the text the user wrote.
7. **Display is the human form, not a round trip.** `Selector::css("//x")` displays as `//x`, which
   the guess reads as XPath. The lossless form is the serde tag `{"kind", "value"}`
   (`:308-309`). Recordings keep strings (see Wire formats).
8. **Generic bloat.** Each converted method is the conversion plus a call to a non-generic body.
   The threshold is measured in phase 2, not guessed (see Phases).
9. **Paths and booleans** ride in the same lane, in the files already being visited, except the four
   `uc: bool` sites in the recorder files, which wait for `lane/recorder-cmd`.

## 1. Today

### How a selector flows

* `src/utils/selectors.rs`: `SelectorRef<'a>` (`:37-49`: `LinkText`, `PartialLinkText`, `Css`, `XPath`,
  `Id`), the prefix tables (`:72`, `:75`), `SelectorRef::auto` (`:210`, order: partial link text, link
  text, shadow, XPath, CSS), `SelectorRef::to_by` (`:254`, `pub(crate)`, builds the `thirtyfour::By`),
  `Selector` (`:310-320`, serde-tagged, four kinds; `Id` has no variant of its own: `From<SelectorRef>`
  turns it into `Css("[id=...]")`, `:628`), the constructors `css`, `xpath`, `link_text`,
  `partial_link_text` (`:324-341`) and the XPath builders `containing_text`, `text`, `label`, `role`
  (`:362-542`), `as_selector` (`:387`), the `From` impls (`:621-657`) and `Display` (`:659`:
  `link=` and `partial_link=` prefixes, CSS and XPath as written).
* `src/browser/shadow_find.rs`: `by_of` (guard arms at `:152-158`) and `checked` (`:169`) turn a
  `Selector` into a `By` and refuse an empty one or one that cannot be cut at `::shadow`, before
  any wait starts.
* WebDriver engine, typed already: `BrowserSession` takes `impl Into<Selector>`
  (`src/browser/session/elements.rs`: 20 methods; `waits.rs`: 5; `navigation.rs:87`), as does
  `Element` (`src/browser/element.rs:628,648,1278,1295`).
* Pure CDP engine, typed already: `Page::locator` (`src/sb_cdp/page.rs:133`), `Locator::locator`
  (`src/sb_cdp/locator.rs:218`), `Page::download_by_click` (`src/sb_cdp/download.rs:367`). Everything
  else goes through `page.locator(sel).click()`, so the engine has no per-action selector methods.
  One string left: `Browser::select_page(&self, selector: &str)` (`src/sb_cdp/browser.rs:1054`).
* The Python-familiar layer, still strings: `BaseCase::click(&mut self, css: &str)`
  (`src/api/base_case.rs:994`). The body uses the string four times: `shadow_find::checked(css)`
  (`:997`), `await_ready(css, ...)` (`src/api/base_case_impls/base_case_impl_parity.rs:146`, detects
  again), `demo_highlight(css)` (`src/api/base_case.rs:244`, detects again) and
  `record("click", Some(css), None)` (`:999`; `fn record` is `:2402`, it hands `Option<&str>` to
  `Recorder::record`, `src/api/recorder.rs:161`). Then `self.session.click(selector)` (`:1002`) takes the
  typed value. The capability traits repeat the string: `ElementApi` (`src/api/traits.rs:62-77`) and
  `AssertionApi` (`:81-95`), and their impls for `BaseCase` (`:140`) and `Page` (`:232`).
* MCP: the wire carries a string. 155 lines under `src/mcp` (without `api_index.json`) name the
  `"selector"` argument; a tool reads it with `args.str("selector")?` and passes the `&str` to
  `page.locator(selector)` (`src/mcp/cdp/clicks.rs:62,72`), so the conversion is implicit and an empty
  string fails at first use.

### Counts (the review step; public means plain `pub`, found with an `ast-grep` rule)

Public functions with a parameter `css: &str` or `selector: &str`: **225**.

| Module | Public fns | Notes |
|---|---|---|
| `src/api/base_case.rs` | 87 | |
| `src/api/base_case_impls/` | 97 | `extra` 30, `cdp_page` 12, `common` 11, `dom` 10, `misc` 9, `gui` 7, `shadow` 7, `mouse` 4, `parity` 4, `highlight` 1, `media` 1, `pdf_html` 1 |
| `src/api/cdp_page.rs`, `deferred.rs`, `html.rs`, `cdp_driver.rs` | 16, 10, 4, 3 (33) | `cdp_driver.rs:198` `click(&self, selector: &str)` |
| `src/api` total | 217 | |
| `src/api/traits.rs` | 21 lines | trait declarations plus the impls for `BaseCase` and `Page`; not in the 225 (trait items are not `pub fn`) |
| `src/utils/selectors.rs`, `src/utils/shadow.rs` | 3 + 3 | the text classifiers (`is_xpath_selector`, `is_shadow_selector`, ...): they read raw text and stay `&str` |
| `src/cli/commands.rs` | 1 | library half of the CLI; look at it in phase 4 |
| `src/sb_cdp/browser.rs` | 1 | `select_page` |
| `src/browser`, rest of `src/sb_cdp` | 0 | 35 parameters already `impl Into<Selector>` |

So about 218 methods convert. The literal counts of the audit (270 `css: &str` and 101
`selector: &str` lines in `src`) also count private helpers and test code. The 225 is a lower bound:
the rule matched the two parameter names only; `xpath: &str` (`click_xpath`, `base_case.rs:2857`;
`convert_xpath_to_css`, `:3149`) and other names come on top, and phase 2 lists them with the same rule
widened.

## 2. The type

`Selector` and `SelectorRef<'a>` stay where they are. Changes to them:

```rust
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Selector { Css(String), XPath(String), LinkText(String), PartialLinkText(String) }

impl Selector {
    pub fn css(..) / xpath(..) / link_text(..) / partial_link_text(..)   // exist; add #[must_use]
    pub fn as_str(&self) -> &str;                          // the text without a prefix
    pub fn check(&self) -> Result<(), SeleniumBaseError>;  // InvalidSelector: blank, or ends on ::shadow
    pub fn lookup_form(&self) -> Cow<'_, Selector>;     // :contains() -> XPath; else borrowed
}
impl FromStr for Selector { type Err = SeleniumBaseError; }   // From<&str> + check
```

* Where it lives: `utils::selectors` (one path; F12 decides the crate-root re-export, and the
  contract test pins whichever path stays).
* The `By`-like constructors exist (`Selector::css`, `xpath`, `link_text`, `partial_link_text`) and are the
  way to skip the guess for a string that would be misread; no `SelectorRef::css` twin is added.
* `SelectorRef::Id` stays borrowed-only; `Selector` gets no `Id` variant (an id is a `[id="..."]`
  CSS selector already, `:628`). Not widened.
* `check` and `lookup_form` replace the two copies named in decision 4 and 6; `SelectorRef::to_by` and
  `shadow_find::by_of` call them. The existing test `a_selector_buf_agrees_with_selector_auto`
  (`:1469`) becomes the guard that the two views still agree.
* `Display` is unchanged except that `:contains()` text stays as written. The one visible
  normalisation is the prefix spelling: `text=Home` and `link_text=Home` both display as `link=Home`.
* Auto-detection is untouched: the table in the module doc (`:7-12`) is the contract.

## 3. Signatures

Before and after (parameter renamed `css` to `selector`: the name is wrong for an XPath):

```rust
// src/api/traits.rs:64 (the same for double_click, type_text, text, attribute, assert_*)
async fn click(&mut self, css: &str) -> crate::Result<()>;
async fn click(&mut self, selector: impl Into<Selector> + Send) -> crate::Result<()>;

// src/api/base_case.rs:994: the public method is the conversion and one call
pub async fn click(&mut self, css: &str) -> Result<(), SeleniumBaseError> { /* 14 lines */ }
pub async fn click(&mut self, selector: impl Into<Selector>) -> Result<(), SeleniumBaseError> {
    self.click_selected(selector.into()).await
}
async fn click_selected(&mut self, selector: Selector) -> Result<(), SeleniumBaseError> { /* body, once */ }

// src/api/cdp_driver.rs:198 and src/sb_cdp/browser.rs:1054
pub async fn click(&self, selector: &str)            -> pub async fn click(&self, selector: impl Into<Selector>)
pub async fn select_page(&self, selector: &str)      -> pub async fn select_page(&self, selector: impl Into<Selector>)

// internal readers take the value, not the string (src/api/base_case_impls/base_case_impl_parity.rs:146, base_case.rs:244, :2402)
async fn await_ready(&self, css: &str, ready: Ready)   -> async fn await_ready(&self, selector: &Selector, ready: Ready)
async fn demo_highlight(&self, css: &str)              -> async fn demo_highlight(&self, selector: &Selector)
fn record(&self, name: &str, target: Option<&str>, ..) -> fn record(&self, name: &str, target: Option<&Selector>, ..)
```

On both engines: the Pure CDP engine (Rust-native) is already on this signature everywhere except
`select_page`; the work is in the Python-familiar layer (`BaseCase`, `CdpDriver`, `deferred`, `html`,
`cdp_page`, the traits). `assert_text(expected, selector)` keeps its text-first order (`traits.rs:91`).

**Generic bloat.** A generic method is compiled in the calling crate, once per argument type per
method used. A test crate that passes `&str` pays one copy per method it calls, and one that mixes
`&str`, `String` and `Selector` pays up to three. The shell above holds none of the body; only the
conversion is generic. Google's guide ("code bloat concerns") and the audit (lane 19) ask for exactly
this. The split is per method and needs the inner name, so one `ast-grep` pattern cannot write it for
all; phase 2 measures first (`cargo bloat --release --crates` on a probe test that calls twenty methods
with three argument types, and an `ast-grep` scan that counts bodies over eight lines) and splits only
the bodies over the threshold; the rest convert in place with `let selector = selector.into();`.
A generic method with an `impl Into<..> + Send` parameter is not dyn-compatible, and `dyn ElementApi`
occurs nowhere (lane 16 searched the repository and the sibling repositories), so the traits keep the
generic parameter. Lane 16 (item 16: native `async fn` in the traits) has landed: each trait method is
`fn name(..) -> impl Future<Output = ..> + Send`, and the `impl` blocks write `async fn`.

Call sites that stop compiling: `&&str` (an `&selector` where `selector: &str`), and any code that
stores `BaseCase::click` as `fn(&mut BaseCase, &str)`. Both are found by `cargo check --all-targets` in
each phase; a `From<&&str>` is not added (it would hide the double borrow).

## 4. The other two parts of F9

### Booleans to two-variant enums (nine sites; counted by the audit, lines checked)

Builder toggles such as `headless(mut self, headless: bool)` (`src/sb_cdp/launch/builder.rs:134`) are
fine and not listed; about 165 public functions have a `name: bool` parameter, almost all of that kind.

| Site | Today | Becomes |
|---|---|---|
| `src/api/recorder.rs:312`, `:364` (and `:276`), `src/api/recorder/generate.rs:152` | `uc: bool` | `DriverMode` (exists, `src/browser/config.rs`) |
| `src/api/presentation.rs:446` (and `:493`, `:507`) | `to_html(show_notes: bool, interval, end_marker: bool)` | two enums, or one options struct with named fields |
| `src/api/chart.rs:526` | `to_html(include_libs: bool)` | a two-variant enum (libraries inline or left out) |
| `src/browser/driver_access.rs:69` | `make_runnable(path: &Path, resign: bool)` | `Resign::{Yes, No}` or a name that reads at the call site |
| `src/cli/scripts/sb_convert.rs:72` | `convert(input: &Path, force: bool)` | `Overwrite::{Replace, Refuse}` |
| `src/api/base_case_impls/base_case_impl_addon.rs:110` | `install_addon(path: &str, temporary: bool)` | `AddonLifetime::{Temporary, Permanent}` |
| `src/sb_cdp/locator.rs:1303` | `click_with_offset(x, y, center: bool)` | `Origin::{Center, TopLeft}` |

The enum names are proposals; the implementer checks for an existing enum first (one name, one
meaning). In this lane: all of them except the four `uc` sites, which are in the files of
`lane/recorder-cmd`; those go in a follow-up commit once it has landed (`DriverMode` is what that
lane's typed command should hold anyway).

**Done (2026-11-21, lane `api-bools-paths`):** the nine sites are enums (`AddonLifetime`,
`ChartLibs`, `SpeakerNotes` with `EndMarker`, `CodeSignature`, `Overwrite`, `OffsetFrom`, and the
existing `DriverMode` for the four `uc` sites and for `RecordingOptions::uc`, which is now `mode`).
Only `DriverMode::Uc` writes Python's `--uc` output.

**Done (2026-11-21, lane `bools-2`, second batch):** `TextMatching::{Contains, Exact}` for the `exact`
of `Selector::text`, `label` and `role`; `OldFiles::{Clear, Archive}` for the `archive` of
`reset_logs`, `reset_downloads`, `log_folder_setup` and `archive_logs_if_set`; `BrowserWindow` and the
existing `Platform` for `virtual_display::wanted` and `Platform` for `sb_install::path_target_dir`;
and `GeneratedFile` is a struct. Left as `bool` on purpose: the setters of builders (`Suite::basic`,
`gha`, `bdd`, `TestFile::basic`, the run builder's `release`, `list` and `nocapture`,
`GridProcess::verbose`, `VisualBaseline::update_baselines`, `sb_install`'s `copy_to_path`,
`Scaffold::replacing`), which name the option at the call site, and `mcp::Args::bool_or(name,
default)`, whose bool is the default of a named argument. Internal (private or `pub(crate)`) functions
such as `swapped_text_hint(.., exact)` and the importer's `Step` fields were not changed.


### Paths to `impl AsRef<Path>` (29 sites; the audit's `file_path`/`path` count needed this review)

`filename: &str`, `path: &str` or `file_path: &str` parameters, found by the same rule, minus the
URL paths (`requests(path)`, `base_case_impl_common.rs:821`; `with_path`, `selenium_grid.rs:122`) and
the `xpath` and command-output strings:

| File | Lines |
|---|---|
| `src/api/base_case_impls/base_case_impl_downloads.rs` | 87, 92, 103, 134, 144 |
| `src/api/base_case_impls/base_case_impl_extra.rs` | 1286, 1301, 1316, 1909, 1949 |
| `src/api/base_case_impls/base_case_impl_pdf_html.rs` | 31, 57, 65, 73 |
| `src/api/base_case_impls/base_case_impl_media.rs` | 11, 23, 33, 64 |
| `src/api/base_case_impls/base_case_impl_dom.rs` | 63, 72, 80, 422 |
| `base_case_impl_misc.rs:376`, `base_case_impl_addon.rs:110`, `base_case.rs:2306` | 1 each |
| `src/api/presentation.rs:493,507`, `src/api/chart.rs:590`, `src/cli/scripts/sb_print.rs:19` | 4 |

Some of these name a file inside a known folder (the logs or `downloaded_files` folder), not a
path; the parameter is still `impl AsRef<Path>` (C-GENERIC, M-IMPL-ASREF), and the doc says a bare name
resolves under that folder, as today. Return types stay `PathBuf`. All 29 are in this lane, as a
separate commit in the pass over each file, because the files are the ones the selector phases visit
(one visit per file, two commits).

**Done (2026-11-21, lane `api-bools-paths`):** the 29 file-name parameters take `impl AsRef<Path>`;
`artifacts::bare_file_name` and `confined_path` take a string or a path, so the check for a plain name
is written once. The `&Path` parameters beside them (`make_runnable`, `convert`, the `folder` of
`save_in`) stay `&Path`.


## 5. Wire formats stay unchanged

* **MCP tool arguments:** the schema (`selector_prop()`, `src/mcp/cdp/clicks.rs:17`) and the
  descriptions do not change; the argument is a string. One new helper on `Args` (`src/mcp/tool.rs`)
  reads it with `FromStr` and maps `InvalidSelector` to `ToolError::invalid("selector", ..)`, so an empty
  selector is refused at the argument, not at first use. `src/mcp/dev/api_index.json` is generated from
  the public methods and changes (signatures): regenerate it with
  `SB_UPDATE_API_INDEX=1 cargo test -p seleniumbase-rs --features mcp-server api_index`.
* **Page script:** the `{"kind", "value"}` payload is pinned by the test
  `a_selector_buf_serializes_with_its_kind_for_the_page` (`:1491`) and does not change; the
  `lookup_form` rewrite happens before it is serialised.
* **Saved recordings (JSON, YAML) and `BaseCase::record`:** strings, as now. `BaseCase::record` writes
  `selector.to_string()`; with decision 6 a `:contains()` CSS stays as written, and the only visible
  difference is the prefix spelling (`text=` to `link=`), covered by a golden test.
* **The recorder (`lane/recorder-cmd`):** that lane lands a typed `RecordedCommand` with
  `selector: String` fields. This lane edits no recorder file. Adoption later is one commit: the field
  becomes `Selector` with `#[serde(with = ..)]` writing the `Display` text and reading it with
  `From<&str>`, so recordings stay human-editable and byte-identical. That is lossless for what the
  recorder produces (auto-detected strings); a command built with `Selector::css("//x")` would not
  round-trip through text, and if that matters the field takes the tagged form instead.
  `Recorder::record(.., target: Option<&str>)` (`src/api/recorder.rs:161`) stays until then.
* **Generated test code:** `tests/golden` and the `sbrs mkfile` templates are unchanged; they call
  methods with string literals, which still compile.

## 6. Migration plan

Test first in every phase: the new tests are written and checked to fail (or to not compile) before the
change. Each phase is about 30 tool calls, ends green (`cargo check --all-targets`, then the narrow
tests of the files it changed), and can merge on its own, because a `&str` argument still compiles. The
Breaking line of the CHANGELOG is written in phase 2 and extended after.

| Phase | Owns | Tests added (first) |
|---|---|---|
| **1. The type** | `src/utils/selectors.rs`, `src/browser/shadow_find.rs`, `src/sb_cdp/locator.rs` | in the `tests` module of `selectors.rs`: `from_str_refuses_a_blank_selector`, `from_str_refuses_a_selector_that_ends_on_shadow`, `from_str_agrees_with_from_str_slice`, `a_contains_selector_keeps_its_text_until_lookup` (`lookup_form` equals today's XPath), `serde_round_trip_keeps_the_kind`; a `Locator` test on the mock transport that `Display` shows the CSS as written and the page receives XPath; `Selector` and `SelectorRef` added to `tests/contract_send_sync.rs`. Docs: module doc of `selectors.rs`, `docs/api/crate-root.md:62`. |
| **2. Helpers, `base_case.rs`, traits** | `await_ready`, `demo_highlight`, `BaseCase::record`; `src/api/base_case.rs` (87, two commits by line range); `src/api/traits.rs` (21); the measurement of the generic threshold | `a_selector_may_be_a_str_a_string_or_a_selector_buf` (the same `By` from each, in `src/api/base_case_impls/base_case_impl_element_tests.rs`, on the mock driver its neighbours use); `a_string_is_read_as_a_selector_once_per_call` (extends the test at `src/browser/session/elements.rs:396` to `BaseCase`); a recorder golden: `click("text=Home")` and `click("p:contains('x')")` record as written. CHANGELOG `### Breaking` line (`CHANGELOG.md:110`). |
| **3. `base_case_impls/`** | the 97 methods in 12 files (largest: `extra` 30, `cdp_page` 12, `common` 11, `dom` 10); the `filename` paths of `downloads`, `extra`, `pdf_html`, `media`, `dom`, `misc`, `addon` (own commits) | one smoke test per file through the mock (a `&str`, a `String`, a `Selector` reach the same call); `a_path_may_be_a_str_a_string_or_a_path` for the path commits. |
| **4. The rest of the layer** | `src/api/cdp_page.rs` (16), `deferred.rs` (10), `html.rs` (4), `cdp_driver.rs` (3), `src/sb_cdp/browser.rs:1054`, `src/cli/commands.rs`, the MCP `Args` helper in `src/mcp/tool.rs` and its tools; the widened scan for other parameter names | MCP: `an_empty_selector_argument_is_refused_at_the_argument` per server (`cdp`, `driver`, `sb`); `select_page` with a `Selector`. `api_index.json` regenerated (the `api_index` test is the check). |
| **5. Booleans** | the sites of section 4 except the four `uc` ones; the `GeneratedFile` tuple alias (`src/cli/scaffold.rs:345`) rides here if the lead keeps it in F9 | a test per enum that the two variants do what the two bools did (`make_runnable`, `convert`, `to_html` of the chart and the presentation, `install_addon`, `click_with_offset`). `uc` follow-up after `lane/recorder-cmd`. |
| **6. Docs, contracts, gate** | `docs/api/waits-assertions.md`, `docs/python-migration.md`, `docs/glossary.md`, `docs/help/common_problems.md`, `docs/ecosystem/tauri-api/elements.md`, `docs/plugins/extractor.md`, `docs/concepts/mobile-and-accessibility.md:420`, `docs/DEVELOPER_GUIDE.md:1383`, `docs/api/cdp-overview.md`; `tests/contract_*.rs` (`mise run contracts`: `contract_companion`, `contract_tauri`, `contract_stealth`, `contract_plugins`, `contract_script`, `contract_send_sync`); `examples/`; `docs/best-practices.md` and `AGENTS.md` if they teach `&str` selectors | `tests/docs_current.rs` and the `own-imports` doctest must stay green (the generated code imports only `seleniumbase_rs`). The lead runs the full gate, `mise run semver` and `cargo bloat --release --crates` before and after. |

`docs/changelog.md` is generated from `CHANGELOG.md`; it is not edited by hand. `parity/api.toml` does
not change (no new Python API).

**CHANGELOG (`### Breaking`, `CHANGELOG.md:110`), one entry per kind of change:**

* `Breaking: every selector parameter of BaseCase, CdpDriver and the ElementApi and AssertionApi traits is
  impl Into<Selector> (it was css: &str).` Strings still work; `&&str` does not; the parameter is called
  `selector`. `Selector` gains `FromStr`, `Deserialize`, `as_str`, `check` and `lookup_form`, and is
  `#[non_exhaustive]`; a `:contains()` selector now keeps its text until the lookup.
* `Breaking: file and path parameters take impl AsRef<Path>` (the 29 sites).
* `Breaking: boolean parameters become enums` (the nine sites).

No deprecation shims (breaking changes are fine before 1.0).

**Order against other lanes.** Lane 16 (traits, native async) first, so phase 2 edits its result;
`lane/recorder-cmd` first for the `uc` sites; both rewrite files this lane touches (`traits.rs`,
`recorder*.rs`), so each phase rebases at its start.

## 7. Open questions for the owner

**Decided 2026-11-19:** the owner accepted the recommended answer to all five questions below.

1. **Move the `:contains()` rewrite from construction to lookup (decision 6)?** It is the only
   non-mechanical change and alters when `Selector::from` rewrites. Recommended: yes; otherwise the
   recorder and error messages show XPath where the user wrote CSS, or the shell must keep the original
   string next to the converted one.
2. **Rename the parameter `css` to `selector` across the Python-familiar layer?** It is wrong for an XPath
   and matches Python's `selector`; it only affects rustdoc and named-argument-style reading, not callers.
   Recommended: yes.
3. **Booleans and paths in this lane, or a sibling lane?** Recommended: paths in this lane, in the same
   file pass as the selectors (29 sites in files being edited anyway), booleans in phase 5, and the four
   `uc` sites after `lane/recorder-cmd` lands.
4. **`#[non_exhaustive]` on `Selector`?** It lets a kind (a test id, an accessibility role) be added
   without a major version, at the price that downstream code matching it needs a `_` arm; the stealth
   crate imports `sb_cdp`, so check it does not match exhaustively. Recommended: yes.
5. **How far to split shell and body?** Recommended: measure in phase 2 and split only bodies over about
   eight lines, instead of all 218 methods; the owner may prefer the uniform rule (every method splits)
   for review simplicity at the price of one extra private function per method.
