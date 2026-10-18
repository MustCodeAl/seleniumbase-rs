---
title: "Case Plans Generator"
sidebarTitle: "Case plans"
---

export const LayoutTree = ({ children }) => (
  <div className="not-prose layout-tree" style={{ margin: "1.25rem 0", fontFamily: "var(--font-sans)", fontSize: "14px", color: "var(--text-secondary)" }}>
    <style>{`
      .layout-tree summary { list-style: none; }
      .layout-tree summary::-webkit-details-marker { display: none; }
      .layout-tree .lt-row:hover { background-color: var(--surface-subtle); }
      .layout-tree .lt-fold { color: #d5a910; }
      .dark .layout-tree .lt-fold { color: #ffd452; }
      .layout-tree .lt-fold-closed { display: none; }
      .layout-tree details:not([open]) summary .lt-fold-open { display: none; }
      .layout-tree details:not([open]) summary .lt-fold-closed { display: flex; }
      .layout-tree .lt-hl { color: var(--primary); background-color: color-mix(in srgb, var(--primary) 8%, transparent); }
      .dark .layout-tree .lt-hl { color: var(--primary-light); }
      .layout-tree .lt-ic-gray { color: #84848a; }
      .dark .layout-tree .lt-ic-gray { color: #adadb1; }
      .layout-tree .lt-ic-blue { color: #1a85d4; }
      .dark .layout-tree .lt-ic-blue { color: #69b1ff; }
      .layout-tree .lt-ic-cyan { color: #1ca1c7; }
      .dark .layout-tree .lt-ic-cyan { color: #68cdf2; }
      .layout-tree .lt-ic-green { color: #199f43; }
      .dark .layout-tree .lt-ic-green { color: #5ecc71; }
      .layout-tree .lt-ic-indigo { color: #693acf; }
      .dark .layout-tree .lt-ic-indigo { color: #9d6afb; }
      .layout-tree .lt-ic-orange { color: #d47628; }
      .dark .layout-tree .lt-ic-orange { color: #ffa359; }
      .layout-tree .lt-ic-pink { color: #d32a61; }
      .dark .layout-tree .lt-ic-pink { color: #ff678d; }
      .layout-tree .lt-ic-purple { color: #a631be; }
      .dark .layout-tree .lt-ic-purple { color: #d568ea; }
      .layout-tree .lt-ic-red { color: #d52c36; }
      .dark .layout-tree .lt-ic-red { color: #ff6762; }
      .layout-tree .lt-ic-yellow { color: #d5a910; }
      .dark .layout-tree .lt-ic-yellow { color: #ffd452; }
    `}</style>
    {children}
  </div>
)

export const LayoutRow = ({ icon, name, note, highlight = false, button = false }) => (
  <div className={highlight ? "lt-row lt-hl" : "lt-row"} style={{ display: "flex", alignItems: "flex-start", gap: "6px", minHeight: "30px", padding: "5px 8px", margin: "0 2px", borderRadius: "6px", lineHeight: "20px", cursor: button ? "pointer" : "default" }}>
    <span style={{ width: "16px", height: "20px", flexShrink: 0, display: "flex", alignItems: "center", justifyContent: "center", color: highlight ? "inherit" : "var(--text-muted)" }}>{icon}</span>
    <span style={{ whiteSpace: "nowrap" }}>{name}</span>
    {note ? <span style={{ color: "var(--text-muted)", marginLeft: "6px" }}>{note}</span> : null}
  </div>
)

export const LayoutFolderIcon = ({ kind }) => (
  <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
    {kind === "empty" ? (
      <path fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinejoin="round" d="M2 4.2A1.2 1.2 0 0 1 3.2 3h3.1c.32 0 .62.13.85.35L8.4 4.6h4.4A1.2 1.2 0 0 1 14 5.8v6a1.2 1.2 0 0 1-1.2 1.2H3.2A1.2 1.2 0 0 1 2 11.8z" />
    ) : kind === "open" ? (
      <g fill="currentColor">
        <path opacity=".5" d="M1.5 4A1.5 1.5 0 0 1 3 2.5h3.38a1.5 1.5 0 0 1 1.06.44L8.5 4H13a1.5 1.5 0 0 1 1.5 1.5V7H4.2a1.5 1.5 0 0 0-1.4.95L1.5 11.3z" />
        <path d="M4.2 8h10.6a.7.7 0 0 1 .65.95l-1.6 4.1a1.5 1.5 0 0 1-1.4.95H2.2a.7.7 0 0 1-.65-.95l1.25-3.2A1.5 1.5 0 0 1 4.2 8z" />
      </g>
    ) : (
      <g fill="currentColor">
        <path d="M1.5 4A1.5 1.5 0 0 1 3 2.5h3.38a1.5 1.5 0 0 1 1.06.44L8.5 4H1.5z" />
        <path opacity=".6" d="M1.5 4.5V12A1.5 1.5 0 0 0 3 13.5h10a1.5 1.5 0 0 0 1.5-1.5V6A1.5 1.5 0 0 0 13 4.5z" />
      </g>
    )}
  </svg>
)

export const LayoutFileIcon = () => (
  <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
    <path fill="currentColor" opacity=".5" d="M8 1v3a3 3 0 0 0 3 3h3v5.5a2.5 2.5 0 0 1-2.5 2.5h-7A2.5 2.5 0 0 1 2 12.5v-9A2.5 2.5 0 0 1 4.5 1z" />
    <path fill="currentColor" d="M9.5 1a.5.5 0 0 1 .354.146l4 4A.5.5 0 0 1 14 5.5V6h-3a2 2 0 0 1-2-2V1z" />
  </svg>
)

export const LayoutDir = ({ name, note, open = false, highlight = false, children }) =>
  children ? (
    <details open={open}>
      <summary>
        <LayoutRow icon={<span className={highlight ? "" : "lt-fold"} style={{ display: "flex" }}><span className="lt-fold-open" style={{ display: "flex" }}><LayoutFolderIcon kind="open" /></span><span className="lt-fold-closed"><LayoutFolderIcon kind="closed" /></span></span>} name={name} note={note} highlight={highlight} button />
      </summary>
      <div style={{ marginLeft: "17.5px", borderLeft: "1px solid var(--border-subtle)", paddingLeft: "6px" }}>
        {children}
      </div>
    </details>
  ) : (
    <LayoutRow icon={<span className={highlight ? "" : "lt-fold"} style={{ display: "flex" }}><LayoutFolderIcon kind="empty" /></span>} name={name} note={note} highlight={highlight} />
  )

export const layoutIconColor = (name) => {
  const n = String(name).toLowerCase()
  const ext = n.includes(".") ? n.split(".").pop() : ""
  if (n === "dockerfile" || n.startsWith("dockerfile.") || n.startsWith("docker-compose")) return "blue"
  if (["rs", "json", "jsonc", "html", "htm", "zip", "gz", "tgz", "crx", "xpi", "swift", "svg"].includes(ext)) return "orange"
  if (["ts", "tsx", "mts", "cts", "py"].includes(ext)) return "blue"
  if (["js", "mjs", "cjs", "jsx"].includes(ext)) return "yellow"
  if (["md", "mdx", "sh", "bash", "zsh", "fish", "ps1"].includes(ext)) return "green"
  if (["css", "scss"].includes(ext)) return "indigo"
  if (["png", "jpg", "jpeg", "gif", "webp", "ico"].includes(ext)) return "pink"
  if (ext === "go") return "cyan"
  return "gray"
}

export const LayoutFile = ({ name, note, highlight = false }) => (
  <LayoutRow icon={<span className={highlight ? "" : "lt-ic-" + layoutIconColor(name)} style={{ display: "flex" }}><LayoutFileIcon /></span>} name={name} note={note} highlight={highlight} />
)

`sbrs caseplans` (also `sbrs case-plans`) is a terminal app for writing a
case plan for each of your tests: a short markdown table of the steps the test
performs and the result each one expects. It is the Python Case Plans Generator
without the <Tooltip tip="The graphical window toolkit that ships with Python.">Tkinter</Tooltip> window.

<Info>

**What you will learn**

- How to build and launch the app.
- Which tests it lists and where it keeps their plans.
- How to create, edit and save a plan, and how to write the summary of all
  plans.

</Info>

<Visibility for="agents">

Summary for AI exports: `sbrs caseplans` (feature `tui`) lists the test functions of `tests/` and `examples/` (or a folder or file you name) and keeps one markdown plan per test at `case_plans/<name>.md` beside its file. `sbrs caseplans --starter` writes a `CASE_PLANS.md` checklist and needs no feature. The `s` key writes the summary `case_summary.md`.

</Visibility>

## Build and launch

<Badge color="gray">Feature: tui</Badge>

The app is part of the opt-in `tui` feature, with `sbrs commander`:

<CodeGroup>

```bash Run from source
cargo run --features tui --bin sbrs -- caseplans
```

```bash Install the binary with it
cargo install --git https://github.com/MustCodeAl/seleniumbase-rs seleniumbase-rs-cli --features tui
```

</CodeGroup>

<Note>

An `sbrs` built without the feature still has the subcommand, but it only
prints the following message and exits with status 2, as `commander` does.

</Note>

```text
this sbrs was built without the terminal UI; reinstall it with `cargo install --git https://github.com/MustCodeAl/seleniumbase-rs seleniumbase-rs-cli --features tui`
```

Run it from the project root. The folder to look in is the optional argument:

```bash
sbrs caseplans                 # the tests/ and examples/ folders
sbrs caseplans tests/ui        # the *.rs files of one folder
sbrs caseplans tests/login.rs  # one test file
```

A folder that has a `tests` or an `examples` folder is searched there, as
`commander` does; any other folder is searched itself. Only the `*.rs` files
directly inside a folder count, and they are listed by name.

`sbrs caseplans --starter` skips the app and writes a checklist, `CASE_PLANS.md`,
in the current folder. It never replaces a file that is there, and it works in
every build.

## What is listed

Every test function, one row each: `tests/login.rs::valid_login` is the test
`valid_login` of `tests/login.rs`. A function counts when a `test` attribute
(`#[test]`, `#[tokio::test]`, `#[rstest]`) comes before it. This is a read of
the text, not a parse, so a `#[test]` inside a string looks like a test. A file
with no test function is listed as the file alone.

Each row shows `[x]` when you have marked it and `[plan]` when its plan exists.

## Where plans go

The plan of a test is `case_plans/<name>.md` in the folder of its file, where
the name is the last part of the row with `::` turned into `.`:

<LayoutTree>
  <LayoutDir name="tests" open>
    <LayoutFile name="login.rs" note="holds the test valid_login" />
    <LayoutFile name="about.rs" note="a file with no test function, listed as the file alone" />
    <LayoutDir name="case_plans" open note="plans of the files directly in tests/">
      <LayoutFile name="login.valid_login.md" highlight note="the plan of tests/login.rs::valid_login" />
      <LayoutFile name="about.rs.md" note="the plan of tests/about.rs" />
    </LayoutDir>
    <LayoutDir name="ui" open>
      <LayoutFile name="cart.rs" note="holds the test empty" />
      <LayoutDir name="case_plans" open note="plans of the files in tests/ui/">
        <LayoutFile name="cart.empty.md" note="the plan of tests/ui/cart.rs::empty" />
      </LayoutDir>
    </LayoutDir>
  </LayoutDir>
</LayoutTree>

<Info>

Cargo ignores a `case_plans` folder inside `tests/`, so the plans do not become
test targets.

</Info>

A new plan is the Python template:

````markdown New plan template
``login.rs::valid_login``
---
| # | Step Description | Expected Result |
| - | ---------------- | --------------- |
| 1 | Perform Action 1 | Verify Action 1 |
| 2 | Perform Action 2 | Verify Action 2 |
````

## Keys

On the list:

| Key | Action |
|-----|--------|
| `↓` / `↑` or `j` / `k` | Move (wraps around at either end) |
| `PageDown` / `PageUp`, `Home` / `End` | Move a page, or to the first or last row |
| `Space` | Mark or unmark the row |
| `a` / `n` | Mark every row shown / clear all marks |
| `c` | Create a boilerplate plan for each marked test that has none (for the row under the cursor when nothing is marked). A plan that exists is never replaced. |
| `Enter` or `e` | Open the plan of the row in the editor |
| `s` | Write the summary to `case_summary.md` |
| `/` or `f` | Filter the rows (case-insensitive); `Enter` keeps the filter, `Esc` clears it |
| `r` | List the tests again; marks are kept |
| `q` or `Ctrl-C` | Quit |

In the editor:

| Key | Action |
|-----|--------|
| Characters, `Enter`, `Backspace`, `Delete` | Edit |
| Arrow keys, `Home`, `End`, `PageUp`, `PageDown` | Move the cursor |
| `Ctrl-S` | Save the plan |
| `Esc` | Back to the list |
| `Ctrl-C` | Quit |

When the plan has changes that are not saved, `Esc` and `Ctrl-C` ask first: `s`
saves and then closes (or quits), `d` throws the changes away, and `Esc` goes on
editing. A plan is saved with LF line ends and a closing newline.

## The summary

`s` writes `case_summary.md` in the current folder, for the listed tests that
have a plan, and says how many of each kind there are:

- a plan whose table was filled in,
- a plan that is still the template,
- a plan with no table yet.

The file counts them in a table and folds each plan into a `<details>` block, as
the Python summary does, so it reads well on a code host. It is replaced each
time.

## Differences from Python

- It runs in the terminal; Python opens a Tkinter window.
- Python lists the tests pytest collects (`test_file.py::Class::test_method`);
  this lists Rust test functions found in the files.
- Python only creates boilerplate plans. Here you can also open a plan, edit it
  and save it without leaving the app.
- Python writes plans and the summary with CRLF line ends; this writes LF.
