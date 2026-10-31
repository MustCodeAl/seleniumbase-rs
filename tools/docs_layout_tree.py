#!/usr/bin/env python3
"""Keep the layout trees of the docs in one place.

Two things were copied by hand onto many pages and drifted apart:

1. The tree component. Thirteen pages began with the same seven `export const`
   definitions (`LayoutTree`, `LayoutRow`, `LayoutFolderIcon`, `LayoutFileIcon`,
   `LayoutDir`, `layoutIconColor`, `LayoutFile`). The one copy that counts is
   `tools/layout_tree/component.mdx`; each page keeps a copy of it between
   `{/* layout-tree-component:start */}` and `{/* layout-tree-component:end */}`.
   Docs7 has a native `<Tree>`, but it takes no note per row, and the trees of
   these pages rest on their notes, so the pages keep their own component.
2. The tree of the repository (`src/`, `crates/`, `docs/`, ...). `LAYOUT` below
   is its one description. It is checked against the files git tracks, so it
   cannot name a path that is gone or leave out a new folder of `src/` or
   `crates/`. It is written between `{/* layout-tree:start */}` and
   `{/* layout-tree:end */}` on the Docs7 pages (a `<LayoutTree>`), and between
   `<!-- layout-tree:start -->` and `<!-- layout-tree:end -->` in a plain
   Markdown file such as CONTRIBUTING.md (a text tree). Folders marked `open`
   are expanded in both; the text tree shows no row of a folder that is shut.

Usage, from the repository root (`just layout-tree` runs the first line):

    python3 -I tools/docs_layout_tree.py            write every page again
    python3 -I tools/docs_layout_tree.py --check    change nothing; list what differs
    python3 -I tools/docs_layout_tree.py --adopt docs/page.md ...
                                                    wrap a page's unmarked component copy
    python3 -I tools/docs_layout_tree.py --print mdx|md|component

`just mdx` (tools/mdx-check/check.mjs) runs `--print` and compares every page with it.
Only the standard library is used.
"""

import argparse
import os
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
COMPONENT_FILE = ROOT / "tools" / "layout_tree" / "component.mdx"

COMPONENT_MARKERS = ("{/* layout-tree-component:start */}", "{/* layout-tree-component:end */}")
MDX_TREE_MARKERS = ("{/* layout-tree:start */}", "{/* layout-tree:end */}")
MD_TREE_MARKERS = ("<!-- layout-tree:start -->", "<!-- layout-tree:end -->")

SKIP_DIRS = {"node_modules", "book", "target", ".git", ".claude", ".tokensave"}


@dataclass
class Node:
    name: str
    note: str = ""
    dir: bool = False
    open: bool = False
    highlight: bool = False
    # What must be described among the files git tracks directly under a folder:
    # "none" (nothing), "dirs" (every folder) or "all" (every folder and file).
    check: str = "none"
    children: list = field(default_factory=list)


def D(name, note="", *children, open=False, highlight=False, check="none"):
    """A folder; `children` makes it expandable."""
    return Node(name, note, True, open, highlight, check, list(children))


def F(name, note="", highlight=False):
    """A file."""
    return Node(name, note, False, False, highlight)


# The repository as it is. Add a line here when a folder appears in src/, crates/ or the
# root; the check (`just mdx`, or `--check`) names the folder that is missing.
LAYOUT = D(
    "seleniumbase-rs", "the workspace root",
    F("Cargo.toml", "the workspace and the library package: version, rust-version 1.99, features", highlight=True),
    F("Cargo.lock", "the locked dependency versions"),
    F("rust-toolchain.toml", "pins Rust 1.99 with clippy and rustfmt"),
    D("src", "the library (no binary): both engines, the MCP tools, the library half of the command line",
      F("lib.rs", "the crate root and its public re-exports", highlight=True),
      F("error.rs", "SeleniumBaseError: hints, exit codes, selector advice", highlight=True),
      D("api", "BaseCase (base_case.rs, base_case_impls/), the capability traits, recorder, scenario, charts, tours"),
      D("browser", "BrowserConfig, the launcher, the WebDriver session, driver access and downloads"),
      D("sb_cdp", "the pure CDP engine: Browser, Page, Locator, pool, mock"),
      D("evasion", "fingerprints, evasion providers, humanize, binary patching and the CDP reactor"),
      D("mcp", "the MCP servers cdp, driver, sb and dev, and their tools"),
      D("cli", "the library half of sbrs, with no parser: commands.rs, scaffold.rs, scripts/"),
      D("testing", "the rstest sb fixtures; bdd/: Gherkin through rstest-bdd"),
      D("plugins", "reporting and logging plugins"),
      D("plugin_host", "resolves plugin names to a PluginSet (feature plugins); the plugins are in the seleniumbase-rs-plugins repository"),
      D("config", "settings, runtime config, proxy and ad-block lists"),
      D("core", "test result records and the Allure, JUnit and dashboard writers"),
      D("common", "graceful shutdown, encryption, obfuscation, test ids, the debug prompt"),
      D("storage", "ResultStore and ProfileVault (feature turso)"),
      D("utilities", "RetryPolicy, the grid server, Selenium Grid and IDE, the Python importer"),
      D("utils", "selectors, shadow DOM, URLs, geometry, bundled extensions, translations"),
      D("profile_payloads", "parses external browser-profile JSON into a Fingerprint"),
      D("resources", "files the crate manages on disk, such as downloaded browser drivers"),
      D("js_code", "building blocks for the JavaScript the crate sends to a page"),
      F("artifacts.rs", "where the files of a test run go (latest_logs) and how they are named"),
      F("drivers.rs", "the WebDriver types behind BaseCase, under this crate's name"),
      F("json.rs", "JSON values and json!, under this crate's name"),
      F("macros.rs", "convenience macros for tests, such as sb_test!"),
      F("ocr.rs", "reading text from screenshots (feature ocr)"),
      F("tracing_util.rs", "tracing initialization helpers"),
      F("visual.rs", "feature visual: VisualBaseline, screenshot baselines compared by SSIM"),
      open=True, check="all"),
    D("crates", "the workspace members besides the library",
      D("cli", "the package seleniumbase-rs-cli: the sbrs and sbrs-mcp binaries",
        F("Cargo.toml", "default features: pdf, mcp-server, mcp-client, plugins-builtin"),
        D("src", "",
          F("sbrs.rs", "the sbrs command line", highlight=True),
          F("mcp_server.rs", "the sbrs-mcp server", highlight=True),
          F("fig.rs", "the Fig completion spec"),
          F("report.rs", "sbrs report"),
          F("reference.rs", "renders docs/help/cli_reference.md from the usage spec"),
          F("strict.rs", "refuses, with status 2, what the parser is too loose about"),
          D("mcp_install", "sbrs-mcp install, uninstall and clients"),
          open=True),
        D("tests", "every test that runs the real sbrs or sbrs-mcp binary"),
        open=True),
      D("own-imports", "tests that import only seleniumbase_rs: the proof that its re-exports work"),
      open=True, check="all"),
    D("tests", "integration tests of the library",
      D("golden", "compiled into the library"),
      D("support", "shared with crates/cli/tests"),
      F("sb_cdp_mock.rs", "the pure CDP API against a mocked browser"),
      F("sb_cdp_chrome.rs", "the same against a real Chrome, ignored by default"),
      F("browser_smoke.rs", "the WebDriver path against a real Chrome, ignored by default")),
    D("benches", "the benchmarks",
      F("cdp_latency.rs", "the pure CDP engine against a real Chrome"),
      F("cpu.rs", "the library's own work, no browser")),
    D("examples", "runnable examples",
      D("tauri-profile-manager", "has its own manifest and is not part of the workspace")),
    D("docs", "the pages of this site",
      F("docs.json", "the Docs7 navigation and settings"),
      F("SUMMARY.md", "the mdBook table of contents"),
      D("tutorials", "step-by-step guides"),
      D("help", "help pages"),
      D("design", "design notes"),
      D("adr", "architecture decision records")),
    D("parity", "the Python parity manifests",
      F("api.toml", "the status of every Python SeleniumBase API"),
      F("upstream.toml", "the upstream version this crate tracks")),
    D("completions", "the usage spec of sbrs",
      F("sbrs.usage.kdl", "generated from crates/cli/src/sbrs.rs")),
    D("schemas", "JSON Schemas",
      F("fingerprint.schema.json", "the schema of a Fingerprint file")),
    D("contrib", "files that are not part of the crate",
      D("tldr", "draft tldr-pages entries for sbrs, kept to propose upstream")),
    D("scripts", "helper scripts",
      F("check-consumers.sh", "just contracts: checks what the repositories that build on this crate use")),
    D("tools", "scripts for the docs and the parity manifests",
      D("mdx-check", "just mdx: builds every docs page as Docs7 does"),
      D("parity", "extracts, seeds and renders the parity manifest (docs/parity.md)"),
      D("layout_tree", "the tree component that docs_layout_tree.py copies onto the pages"),
      F("docs_layout_tree.py", "just layout-tree: writes the layout trees of the docs pages"),
      F("docs7_changelog.py", "writes docs/changelog.md from CHANGELOG.md"),
      check="dirs"),
    D("docker", "the Linux verification image (Dockerfile.verify, verify-linux.sh) and firefox-check/"),
    D(".config", "configuration of tools",
      F("nextest.toml", "the nextest configuration and its chrome test group")),
    D(".github", "GitHub Actions",
      D("workflows", "ci, docker, completions, release and upstream-watch")),
    D(".warp", "the Warp terminal",
      D("workflows", "the Warp workflows for sbrs")),
    F("Dockerfile", "the sbrs image; its skeleton step lists every target"),
    F("Makefile", "make targets"),
    F("justfile", "just recipes"),
    F("mise.toml", "tools and tasks for mise"),
    F("deny.toml", "the cargo-deny policy"),
    F("clippy.toml", "clippy settings"),
    F("rust-analyzer.toml", "settings for rust-analyzer"),
    F("book.toml", "the mdBook configuration"),
    F("README.md"),
    F("CHANGELOG.md", "what changed in each release"),
    F("ROADMAP.md", "what is done and what is missing"),
    F("CLAUDE.md", "the module map for coding assistants"),
    open=True, check="dirs",
)


def tracked_paths():
    """Every path git tracks or would add, as POSIX strings relative to the root."""
    try:
        out = subprocess.run(
            ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"],
            cwd=ROOT, check=True, capture_output=True,
        ).stdout.decode("utf-8")
        return {p for p in out.split("\0") if p}
    except (OSError, subprocess.CalledProcessError):
        found = set()
        for here, dirs, files in os.walk(ROOT):
            dirs[:] = [d for d in dirs if d not in SKIP_DIRS]
            for name in files:
                found.add((Path(here) / name).relative_to(ROOT).as_posix())
        return found


def entries_under(paths, prefix):
    """The names directly under `prefix`, each True when it is a folder."""
    start = prefix + "/" if prefix else ""
    found = {}
    for p in paths:
        if p.startswith(start):
            name, sep, _ = p[len(start):].partition("/")
            found[name] = found.get(name, False) or bool(sep)
    return found


def validate(node, path, paths, problems):
    """Add a problem for every row without a file and every file without a row."""
    entries = entries_under(paths, path)
    for child in node.children:
        child_path = f"{path}/{child.name}" if path else child.name
        on_disk = entries.get(child.name)
        if on_disk is None:
            problems.append(f"{child_path}{'/' if child.dir else ''} is in LAYOUT but not in the repository")
        elif on_disk != child.dir:
            problems.append(f"{child_path} is a {'folder' if on_disk else 'file'} in the repository, not a {'folder' if child.dir else 'file'}")
        elif child.dir:
            validate(child, child_path, paths, problems)
    if node.check == "none":
        return
    described = {c.name for c in node.children}
    modules = {c.name[:-3] for c in node.children if not c.dir and c.name.endswith(".rs")}
    for name, is_dir in sorted(entries.items()):
        if name in described or (is_dir and name in modules):
            continue
        if not is_dir and node.check == "dirs":
            continue
        problems.append(f"{path + '/' if path else ''}{name}{'/' if is_dir else ''} is in the repository but not in LAYOUT: add a line for it")


def attributes(node):
    parts = [f'name="{node.name}"']
    if node.dir and node.open and node.children:
        parts.append("open")
    if node.highlight:
        parts.append("highlight")
    if node.note:
        parts.append(f'note="{node.note}"')
    return " ".join(parts)


def check_notes(node, problems):
    for text in (node.name, node.note):
        if any(c in text for c in '"&\n'):
            problems.append(f"{node.name}: a name or note may not hold a quote, an ampersand or a newline")
    for child in node.children:
        check_notes(child, problems)


def render_mdx(root):
    """The tree as a `<LayoutTree>`, the component of the Docs7 pages."""
    lines = ["<LayoutTree>"]

    def walk(node):
        if node.dir and node.children:
            lines.append(f"<LayoutDir {attributes(node)}>")
            for child in node.children:
                walk(child)
            lines.append("</LayoutDir>")
        elif node.dir:
            lines.append(f"<LayoutDir {attributes(node)} />")
        else:
            lines.append(f"<LayoutFile {attributes(node)} />")

    walk(root)
    lines.append("</LayoutTree>")
    return "\n".join(lines)


def render_md(root):
    """The tree as text in a code block, for a Markdown file that is not on Docs7."""
    rows = []

    def walk(node, prefix, last, top):
        label = node.name + ("/" if node.dir else "")
        rows.append(((label if top else prefix + ("└── " if last else "├── ") + label), node.note))
        if node.dir and node.open:
            below = "" if top else prefix + ("    " if last else "│   ")
            for i, child in enumerate(node.children):
                walk(child, below, i == len(node.children) - 1, False)

    walk(root, "", True, True)
    width = max(len(left) for left, note in rows if note)
    body = "\n".join(f"{left.ljust(width)}  # {note}" if note else left for left, note in rows)
    return f"```text\n{body}\n```"


def component_text():
    return COMPONENT_FILE.read_text(encoding="utf-8").strip("\n")


def wrap(markers, body):
    return f"{markers[0]}\n\n{body}\n\n{markers[1]}"


def find_block(text, markers):
    """(start, end) of the first block between the markers, or None."""
    start = text.find(markers[0])
    if start < 0:
        return None
    end = text.find(markers[1], start)
    if end < 0:
        raise ValueError(f"{markers[0]} has no {markers[1]}")
    return start, end + len(markers[1])


def line_of(text, index):
    return text.count("\n", 0, index) + 1


def first_difference(a, b):
    """The 1-based line where two texts first differ."""
    for n, (x, y) in enumerate(zip(a.split("\n"), b.split("\n")), start=1):
        if x != y:
            return n
    return min(a.count("\n"), b.count("\n")) + 2


def pages():
    found = sorted(ROOT.glob("*.md"))
    for path in sorted((ROOT / "docs").rglob("*")):
        if path.suffix in (".md", ".mdx") and not SKIP_DIRS.intersection(path.relative_to(ROOT).parts):
            found.append(path)
    return found


def canonical():
    problems = []
    check_notes(LAYOUT, problems)
    validate(LAYOUT, "", tracked_paths(), problems)
    return problems, {"mdx": render_mdx(LAYOUT), "md": render_md(LAYOUT), "component": component_text()}


def sync(path, text, wanted, adopt):
    """The page text with its blocks written again, and what was off about it."""
    notes = []
    rel = path.relative_to(ROOT).as_posix()
    for markers, body, what in (
        (COMPONENT_MARKERS, wanted["component"], "tree component"),
        (MDX_TREE_MARKERS, wanted["mdx"], "layout tree"),
        (MD_TREE_MARKERS, wanted["md"], "layout tree"),
    ):
        block = find_block(text, markers)
        if block is None:
            continue
        start, end = block
        fresh = wrap(markers, body)
        if text[start:end] != fresh:
            notes.append(f"{rel}:{line_of(text, start)}: the {what} differs from the canonical one (line {first_difference(text[start:end], fresh)} of the block)")
            text = text[:start] + fresh + text[end:]
    if find_block(text, COMPONENT_MARKERS) is None and "export const LayoutTree" in text:
        start = text.index("export const LayoutTree")
        end = text.index("\n)\n", text.index("export const LayoutFile =", start)) + 2
        copy = text[start:end]
        if adopt:
            if copy != wanted["component"]:
                notes.append(f"{rel}:{line_of(text, start)}: the tree component differed from the canonical one (line {first_difference(copy, wanted['component'])} of the copy)")
            text = text[:start] + wrap(COMPONENT_MARKERS, wanted["component"]) + text[end:]
            notes.append(f"{rel}:{line_of(text, start)}: the tree component is now between markers")
        elif copy != wanted["component"]:
            notes.append(f"{rel}:{line_of(text, start)}: the tree component differs from the canonical one (line {first_difference(copy, wanted['component'])} of the copy); it has no markers: run --adopt on this page")
    if "<LayoutTree" in text and "export const LayoutTree" not in text:
        notes.append(f"{rel}:{line_of(text, text.index('<LayoutTree'))}: the page uses <LayoutTree> but does not define it; add the tree component")
    return text, notes


def main():
    parser = argparse.ArgumentParser(description="Write the layout trees of the docs pages from one source.")
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--check", action="store_true", help="change nothing; list what differs and exit 1 if anything does")
    mode.add_argument("--adopt", nargs="+", metavar="PAGE", help="wrap the unmarked tree component of these pages")
    mode.add_argument("--print", dest="show", choices=["mdx", "md", "component"], help="print one canonical block")
    args = parser.parse_args()

    problems, wanted = canonical()
    if problems:
        for p in problems:
            print(f"tools/docs_layout_tree.py: {p}", file=sys.stderr)
        return 1
    if args.show:
        sys.stdout.write(wanted[args.show] + "\n")
        return 0

    chosen = [Path(p).resolve() for p in args.adopt] if args.adopt else pages()
    failed = False
    for path in chosen:
        text = path.read_text(encoding="utf-8")
        try:
            fresh, notes = sync(path, text, wanted, adopt=bool(args.adopt))
        except ValueError as e:
            print(f"{path.relative_to(ROOT).as_posix()}: {e}", file=sys.stderr)
            failed = True
            continue
        for note in notes:
            print(note)
        failed = failed or (args.check and bool(notes))
        if not args.check and fresh != text:
            path.write_text(fresh, encoding="utf-8")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
