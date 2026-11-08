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
   Markdown file such as CONTRIBUTING.md (a text tree). A start marker may name
   a folder (`{/* layout-tree:start src/api */}`): that block is the tree of
   that folder, including the rows marked detail, which the repository-wide
   tree leaves out. Folders marked `open` are expanded in both; the text tree
   shows no row of a folder that is shut.

Usage, from the repository root (`mise run layout-tree` runs the first line):

    python3 -I tools/docs_layout_tree.py            write every page again
    python3 -I tools/docs_layout_tree.py --check    change nothing; list what differs
    python3 -I tools/docs_layout_tree.py --adopt docs/page.md ...
                                                    wrap a page's unmarked component copy
    python3 -I tools/docs_layout_tree.py --print mdx|md|component

`mise run mdx` (tools/mdx-check/check.mjs) runs `--print` and compares every page with it.
Only the standard library is used.
"""

import argparse
import os
import re
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
COMPONENT_FILE = ROOT / "tools" / "layout_tree" / "component.mdx"

COMPONENT_MARKERS = ("{/* layout-tree-component:start */}", "{/* layout-tree-component:end */}")
MDX_TREE_MARKERS = ("{/* layout-tree:start */}", "{/* layout-tree:end */}")
MD_TREE_MARKERS = ("<!-- layout-tree:start -->", "<!-- layout-tree:end -->")
MDX_TREE_START = re.compile(r"\{/\* layout-tree:start(?:[ \t]+(\S+))?[ \t]*\*/\}")
MD_TREE_START = re.compile(r"<!-- layout-tree:start(?:[ \t]+(\S+))?[ \t]*-->")

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
    # Shown only when this folder is the root of a scoped tree, not in the
    # repository-wide tree. The check still requires the row.
    detail: bool = False
    children: list = field(default_factory=list)


def D(name, note="", *children, open=False, highlight=False, check="none", detail=False):
    """A folder; `children` makes it expandable."""
    return Node(name, note, True, open, highlight, check, detail, list(children))


def F(name, note="", highlight=False, detail=False):
    """A file."""
    return Node(name, note, False, False, highlight, "none", detail, [])


def files(pairs):
    """Files that belong in a module deep dive and not in the repository-wide tree."""
    return [F(name, note, detail=True) for name, note in pairs]


def _file(name, note):
    return F(name, note, detail=True)


def _folder(name, note, *children, check="all"):
    """A folder that appears in a module deep dive and not in the repository-wide tree."""
    return D(name, note, *children, open=True, check=check, detail=True)


def api_detail():
    """The files of src/api, for the module deep dive."""
    impl = _folder(
        "base_case_impls", "BaseCase helpers, one file per domain",
        _file("base_case_impl_addon.rs", "add-on helpers"),
        _file("base_case_impl_alerts.rs", "alerts"),
        _file("base_case_impl_browser.rs", "browser helpers"),
        _file("base_case_impl_cdp_page.rs", "CDP page helpers"),
        _file("base_case_impl_cdp_tests.rs", "tests of the CDP helpers"),
        _file("base_case_impl_charts.rs", "charts"),
        _file("base_case_impl_common.rs", "helpers shared by the domain files"),
        _file("base_case_impl_cookie_tests.rs", "tests of the cookie helpers"),
        _file("base_case_impl_debug.rs", "debug helpers"),
        _file("base_case_impl_demo_tests.rs", "tests of the demo helpers"),
        _file("base_case_impl_dialog_inspector.rs", "the dialog inspector"),
        _file("base_case_impl_dom.rs", "DOM helpers"),
        _file("base_case_impl_downloads.rs", "downloads"),
        _file("base_case_impl_driver_tests.rs", "tests of the driver helpers"),
        _file("base_case_impl_drivers.rs", "driver helpers"),
        _file("base_case_impl_element_tests.rs", "tests of the element helpers"),
        _file("base_case_impl_esc.rs", "ending a test when ESC is pressed"),
        _file("base_case_impl_extra.rs", "extra helpers"),
        _file("base_case_impl_fingerprint.rs", "fingerprint helpers"),
        _file("base_case_impl_form_tests.rs", "tests of the form helpers"),
        _file("base_case_impl_frame_tests.rs", "tests of the frame helpers"),
        _file("base_case_impl_gui.rs", "GUI helpers"),
        _file("base_case_impl_highlight.rs", "highlighting elements"),
        _file("base_case_impl_highlight_tests.rs", "tests of highlighting"),
        _file("base_case_impl_jslibs.rs", "JavaScript libraries injected into the page"),
        _file("base_case_impl_links.rs", "links"),
        _file("base_case_impl_logs_tests.rs", "tests of the log helpers"),
        _file("base_case_impl_masterqa.rs", "MasterQA helpers"),
        _file("base_case_impl_media.rs", "media helpers"),
        _file("base_case_impl_misc.rs", "miscellaneous helpers"),
        _file("base_case_impl_misc_calls_tests.rs", "tests of the miscellaneous calls"),
        _file("base_case_impl_mouse.rs", "mouse helpers"),
        _file("base_case_impl_mouse_tests.rs", "tests of the mouse helpers"),
        _file("base_case_impl_nav.rs", "navigation helpers"),
        _file("base_case_impl_parity.rs", "parity helpers"),
        _file("base_case_impl_pdf_html.rs", "PDF and HTML helpers"),
        _file("base_case_impl_presentations.rs", "presentations"),
        _file("base_case_impl_rec_cdp.rs", "recorder helpers for CDP"),
        _file("base_case_impl_recorder.rs", "recorder helpers"),
        _file("base_case_impl_shadow.rs", "shadow DOM helpers"),
        _file("base_case_impl_storage.rs", "cookies and storage"),
        _file("base_case_impl_switch_tests.rs", "tests of window and frame switching"),
        _file("base_case_impl_switching.rs", "window and frame switching"),
        _file("base_case_impl_test_data.rs", "test data helpers"),
        _file("base_case_impl_text_tests.rs", "tests of the text helpers"),
        _file("base_case_impl_tours.rs", "tours"),
        _file("base_case_impl_traffic_tests.rs", "tests of the traffic helpers"),
        _file("base_case_impl_window.rs", "window helpers"),
    )
    recorder = _folder(
        "recorder", "the page recorder: state, generated output and its page script",
        _file("breakpoint.rs", "the pause while a person records"),
        _file("calls.rs", "the recorded calls"),
        _file("cdp.rs", "recording over CDP"),
        _file("extras.rs", "extra recorded actions"),
        _file("generate.rs", "turns a recording into a test"),
        _folder("js", "scripts the recorder runs in the page",
                _file("recorder.js", "the recorder script"),
                _file("shift_esc.js", "SHIFT+ESC ends the recording")),
        _file("output.rs", "where a recording is written"),
        _file("process.rs", "processes the recorded actions"),
        _file("raw.rs", "the raw recording"),
        _file("state.rs", "the recorder state"),
    )
    return [
        _file("base_case.rs", "the BaseCase struct and its constructors"),
        impl,
        _file("call.rs", "one call a test makes"),
        _file("cdp_driver.rs", "CdpDriver, the DevTools client used from BaseCase"),
        _file("cdp_page.rs", "CdpPage, one page of that client"),
        _file("chart.rs", "charts"),
        _file("deferred.rs", "assertions that run later"),
        _file("demo.rs", "demo mode: highlight an element and pause"),
        _file("dialog.rs", "dialogs"),
        _file("gui.rs", "GUI automation, feature gui"),
        _file("html.rs", "HTML reports and pages"),
        _file("html_inspector.rs", "the HTML inspector"),
        _file("jqc.rs", "jQuery-style helpers"),
        _folder("master_qa", "MasterQA reports",
                _file("report.rs", "the MasterQA report"),
                _file("wrapper_tests.rs", "tests of the MasterQA wrapper")),
        _file("master_qa.rs", "MasterQA"),
        _file("membership.rs", "membership helpers"),
        _file("messages.rs", "messages shown on the page"),
        _file("misc_calls.rs", "miscellaneous calls"),
        _file("mod.rs", "the api module"),
        _file("page_scripts.rs", "scripts BaseCase runs in the page"),
        _file("pdf.rs", "PDF helpers, feature pdf"),
        _file("presentation.rs", "presentations"),
        recorder,
        _file("recorder.rs", "the recorder on BaseCase"),
        _file("run_settings.rs", "the settings of one test run"),
        _file("runner.rs", "run_browser_test"),
        _file("scenario.rs", "scenarios: several steps in one browser"),
        _file("switching.rs", "switching windows and frames"),
        _file("tour.rs", "tours"),
        _file("traits.rs", "BrowserApi, ElementApi, AssertionApi and ScreenshotApi"),
    ]


def browser_detail():
    """The files of src/browser, for the module deep dive."""
    options = _folder(
        "options", "typed launch settings",
        _file("capabilities.rs", "capabilities added to the session"),
        _file("cdp_events.rs", "which DevTools events are logged"),
        _file("chromium_args.rs", "Chromium switches"),
        _file("device.rs", "mobile device emulation"),
        _file("driver.rs", "driver settings"),
        _file("extensions.rs", "extensions to load"),
        _file("firefox.rs", "Firefox arguments and preferences"),
        _file("locale.rs", "the locale"),
        _file("network.rs", "NetworkThrottle, how the network is slowed down or cut off"),
        _file("mod.rs", "the options module"),
        _file("page_load.rs", "how long a navigation waits"),
        _file("plugins.rs", "plugins turned on for the browser"),
        _file("proxy.rs", "the proxy"),
        _file("remote.rs", "a remote WebDriver server"),
        _file("session.rs", "SessionSettings and GridSettings, what only a WebDriver session reads"),

        _file("switches.rs", "browser switches"),
        _file("virtual_display.rs", "a virtual display"),
        _file("window.rs", "window size and position"),
    )
    return [
        _file("binaries.rs", "finding browser binaries"),
        _file("chrome_for_testing.rs", "Chrome for Testing builds"),
        _file("chromium_snapshots.rs", "Chromium snapshot builds"),
        _file("config.rs", "BrowserConfig, Browser and DriverMode"),
        _file("download_progress.rs", "progress bars for downloads, quiet when not on a terminal"),
        _file("devtools.rs", "the WebDriver engine's DevTools client (CdpClient)"),
        _file("downloader.rs", "downloading a browser or a driver"),
        _file("downloads.rs", "the downloaded_files folder browsers save into"),
        _file("driver_access.rs", "repairs and runs drivers"),
        _file("driver_install.rs", "installs geckodriver and msedgedriver"),
        _file("driver_version.rs", "matches chromedriver to Chrome"),
        _file("element.rs", "Element, the element the WebDriver engine found"),

        _file("launcher.rs", "chromedriver discovery, startup and StealthOptions"),
  
        _file("mod.rs", "the browser module"),
        options,
        _file("password_prompts.rs", "password prompts"),
        _file("plugin_hooks.rs", "calls the plugin set from the WebDriver engine"),
        _file("proxy_auth.rs", "proxy authentication"),
        _file("reuse.rs", "reused sessions and --crumbs"),
        _folder("session", "BrowserSession, the WebDriver wrapper and the CDP session cache",
                        _file("capabilities.rs", "the capabilities each browser is asked for"),
                        _file("cdp.rs", "DevTools commands of the session"),
                        _file("connect.rs", "starting a session, a leased one, or a direct CDP attachment"),
                        _file("elements.rs", "finding and using elements"),
                        _file("emulation.rs", "the ad block and the phone emulation"),
                        _file("fingerprint.rs", "the fingerprint of the session"),
                        _file("launch.rs", "the driver, the browser program and the virtual screen"),
                        _file("lifecycle.rs", "reconnect, quit and the cleanup on drop"),
                        _file("mod.rs", "the BrowserSession struct and its accessors"),
                        _file("navigation.rs", "opening pages and moving through history"),
                        _file("page.rs", "the page: title, source, scripts"),
                        _file("plugins.rs", "browser plugins (feature plugins)"),
                        _file("waits.rs", "waiting for elements and text")),
        _file("shadow_find.rs", "finding elements in a shadow root"),
        _file("virtual_display.rs", "opens a virtual display on Linux"),
    ]


def evasion_detail():
    """The files of src/evasion, for the module deep dive."""
    fingerprint = _folder(
        "fingerprint", "parts of fingerprint.rs",
        _file("compare.rs", "compares two fingerprints"),
        _file("file.rs", "reads and writes a fingerprint file"),
        _file("schema.rs", "the fingerprint JSON schema"),
    )
    js = _folder(
        "js", "page scripts the evasion layer ships",
        _file("active_css_js.rs", "the active CSS script"),
        _file("best_selector.js", "picks a selector for an element"),
        _file("capture.js", "reads a page fingerprint"),
        _file("capture_js.rs", "the capture script as Rust"),
        _file("mod.rs", "the js module"),
        _file("recorder_js.rs", "the recorder script as Rust"),
    )
    providers = _folder(
        "providers", "the evasion registry and the built-in providers",
        _file("builtin.rs", "the built-in providers"),
        _file("mod.rs", "EvasionRegistry and default_registry"),
    )
    return [
        _file("behavior.rs", "human-like input"),
        _file("capture.rs", "CapturedFingerprint: reading a real browser fingerprint"),
        _file("cdp.rs", "the DevTools wrapper used from WebDriver"),
        _file("dprocess.rs", "detached chromedriver and browser processes"),
        _file("evasions.rs", "launch flags for the evasion layer"),
        fingerprint,
        _file("fingerprint.rs", "Fingerprint, one Spoof per setting, ProxyConfig and ClientHints"),
        _file("humanize.rs", "Bezier mouse paths, keystroke timing and the seedable Rng"),
        js,
        _file("mod.rs", "the evasion module"),
        _file("options.rs", "StealthOptions: launch arguments and preferences"),
        _file("patcher.rs", "ChromedriverPatcher and EnginePatch"),
        _file("policy.rs", "StealthPolicy"),
        providers,
        _file("randomize.rs", "the tables behind Fingerprint::randomized; the method is the API"),
        _file("reactor.rs", "the background CDP Fetch interceptor"),
        _file("settings.rs", "value types such as ScreenSize and WebGl"),
        _file("spoof.rs", "Spoof, one value and how it is applied"),
        _file("uc.rs", "undetected-chromedriver mode"),
    ]


def profile_payload_detail():
    return [
        _file("mod.rs", "the profile_payloads module"),
        _file("profile.rs", "ProfileParams: every JSON field, then to_browser_config and browser"),
    ]


def cli_lib_detail():
    """The files of src/cli, the library half of the command line."""
    objectify = _folder(
        "sb_objectify", "turns selectors in a Rust test into page-object constants",
        _file("api.rs", "which calls take a selector"),
        _file("edit.rs", "rewrites the test"),
        _file("lexer.rs", "tokenizes a Rust test"),
        _file("model.rs", "the page objects"),
        _file("scan.rs", "finds the selectors"),
        _file("tests.rs", "tests of objectify"),
    )
    scripts = _folder(
        "scripts", "shared by the binaries, the MCP server and the golden tests",
        _file("all_scripts.rs", "the script registry"),
        _file("behave_gui_tui.rs", "the behave-gui terminal app"),
        _file("behave_options.rs", "how Python behave options map to sbrs test"),
        _file("case_plans.rs", "case plans"),
        _file("caseplans_tui.rs", "the caseplans terminal app"),
        _file("discovery.rs", "finds tests"),
        _file("feature_files.rs", "Gherkin feature files"),
        _file("gherkin_outline.rs", "Gherkin outlines"),
        _file("logo_helper.rs", "the logo"),
        _file("mod.rs", "the scripts module"),
        _file("recorder_tui.rs", "the recorder terminal app"),
        _file("rich_helper.rs", "terminal formatting"),
        _file("run.rs", "runs a selection of tests"),
        _file("run_filter.rs", "the -k and -m filters"),
        _file("run_logs.rs", "logs of a test run"),
        _file("sb_behave_gui.rs", "sbrs behave-gui"),
        _file("sb_caseplans.rs", "sbrs caseplans"),
        _file("sb_commander.rs", "sbrs commander"),
        _file("sb_convert.rs", "sbrs convert"),
        _file("sb_grid.rs", "sbrs grid-hub and grid-node"),
        _file("sb_install.rs", "sbrs install"),
        _file("sb_mkchart.rs", "sbrs mkchart"),
        _file("sb_mkdir.rs", "sbrs mkdir"),
        _file("sb_mkfile.rs", "sbrs mkfile"),
        _file("sb_mkpres.rs", "sbrs mkpres"),
        objectify,
        _file("sb_objectify.rs", "sbrs objectify, extract-objects, inject-objects and revert-objects"),
        _file("sb_print.rs", "sbrs print"),
        _file("sb_recorder.rs", "sbrs recorder"),
        _file("sb_translate.rs", "sbrs translate"),
        _file("tui_kit.rs", "shared pieces of the terminal apps"),
    )
    return [
        _file("commands.rs", "what each sbrs command does, with no parser"),
        _file("devtools.rs", "Chrome with an open DevTools endpoint"),
        _file("fingerprint_cmd.rs", "sbrs fingerprint"),
        _file("mod.rs", "the cli module"),
        _file("plugin_cmd.rs", "sbrs plugin list and info, and the check of plugin names"),
        _file("prompt.rs", "the shell prompt line"),
        _file("scaffold.rs", "sbrs mkdir and mkfile"),
        scripts,
    ]


def utilities_detail():
    pyconv = _folder(
        "pyconv", "converts a Python test into Rust",
        _file("ast.rs", "the Python syntax the converter reads"),
        _file("cdp.rs", "CDP-mode calls"),
        _file("expr.rs", "expressions"),
        _file("generated.rs", "generated pieces of the converter"),
        _file("mod.rs", "the pyconv module"),
        _file("parse.rs", "the parser"),
        _file("stmt.rs", "statements"),
        _file("table.rs", "the method table"),
        _file("tests.rs", "tests of the converter"),
        _file("tr.rs", "the translation"),
        _file("units.rs", "units of a converted test"),
    )
    importer = _folder(
        "importer", "the Python importer",
        _file("emit.rs", "writes the Rust test"),
        _file("lexer.rs", "tokenizes Python"),
        _file("mod.rs", "the importer module"),
        _file("parser.rs", "parses Python"),
        _file("program.rs", "a parsed program"),
        pyconv,
        _file("recorder.rs", "imports a recording"),
        _file("selector.rs", "selectors in the imported test"),
        _file("tests.rs", "tests of the importer"),
        _file("translate.rs", "translates one call"),
    )
    return [
        _file("grid_server.rs", "GridProcess: start, state, stop and ended"),
        importer,
        _file("mod.rs", "the utilities module"),
        _file("python_importer.rs", "imports a Python SeleniumBase test"),
        _file("retry.rs", "RetryPolicy and RateLimiter"),
        _file("selenium_grid.rs", "Selenium Grid helpers"),
        _file("selenium_ide.rs", "the Selenium IDE parser"),
    ]


def cli_package_detail():
    """Rows of crates/cli/src that the repository-wide tree does not expand."""
    args = _folder(
        "args", "argument structs of the sbrs subcommands",
        _file("content.rs", "convert and the content commands"),
        _file("doctor.rs", "sbrs doctor and doctor --fix"),
        _file("generators.rs", "mkdir, mkfile, mkchart, mkpres"),
        _file("install.rs", "sbrs install"),
        _file("language.rs", "the chart and presentation languages"),
        _file("mod.rs", "the args module"),
        _file("objects.rs", "extract-objects, inject-objects, objectify, revert-objects"),
        _file("report.rs", "sbrs report finish and, with the turso feature, report history"),
        _file("tools.rs", "the smaller tool commands"),
    )
    mcp_client = _folder(
        "mcp_client", "sbrs mcp, the MCP client",
        _file("args.rs", "the client command line"),
        _file("mod.rs", "the mcp_client module"),
        _folder("output", "how a tool result is printed",
                _file("tests.rs", "tests of that printing")),
        _file("output.rs", "prints a tool result"),
    )
    return [
        args,
        _file("browser_flags.rs", "the browser options, their SB_ variables and the settings file"),
        _file("completion.rs", "the patched shell completion scripts"),
        _file("crypto.rs", "sbrs encrypt and decrypt"),
        _file("diagnostic.rs", "the miette report printed when a command fails"),
        _file("failure.rs", "a command line that does not parse prints the command page, then the error"),
        _file("fingerprint.rs", "sbrs fingerprint"),
        _file("history.rs", "sbrs report history"),
        _file("info.rs", "sbrs version and the info commands"),
        mcp_client,
        _file("mcp_cmd.rs", "sbrs mcp"),
        _file("proxy.rs", "sbrs proxy"),
        _file("recorder_flags.rs", "the recorder flags"),
        _file("test_args.rs", "sbrs test"),
        _file("test_session.rs", "the test run sbrs test starts and finishes"),
    ]


def cli_install_files():
    """The files of crates/cli/src/mcp_install."""
    return [
        _file("apply.rs", "writes one client's config"),
        _file("args.rs", "the install, uninstall and clients command line"),
        _file("client.rs", "the clients and where each keeps its config"),
        _file("entry.rs", "the two server entries"),
        _file("env.rs", "the machine, faked by tests"),
        _file("external.rs", "the claude and codex programs"),
        _file("fsx.rs", "atomic writes and the backup"),
        _file("jsonedit.rs", "edits JSON as text"),
        _file("jsonfile.rs", "the JSON layouts"),
        _file("mod.rs", "install, uninstall and clients"),
        _file("report.rs", "what one client prints"),
        _file("tests.rs", "tests against a temporary folder"),
        _file("tomlfile.rs", "edits Codex config.toml"),
    ]


# The repository as it is. Add a line here when a folder appears in src/, crates/ or the
# root; the check (`mise run mdx`, or `--check`) names the folder that is missing.
LAYOUT = D(
    "seleniumbase-rs", "the workspace root",
    F("Cargo.toml", "the workspace and the library package: version, rust-version 1.99, features", highlight=True),
    F("Cargo.lock", "the locked dependency versions"),
    F("rust-toolchain.toml", "pins Rust 1.99 with clippy and rustfmt"),
    D("src", "the library (no binary): both engines, the MCP tools, the library half of the command line",
      F("lib.rs", "the crate root and its public re-exports", highlight=True),
      F("error.rs", "SeleniumBaseError: hints, exit codes, selector advice", highlight=True),
      D("api", "BaseCase (base_case.rs, base_case_impls/), the capability traits, recorder, scenario, charts, tours",
        *api_detail(), check="all"),
      D("browser", "BrowserConfig, the launcher, the WebDriver session, driver access and downloads",
        *browser_detail(), check="all"),
      D("sb_cdp", "the pure CDP engine: Browser, Page, Locator, pool, mock"),
      D("evasion", "fingerprints, evasion providers, humanize, binary patching and the CDP reactor",
        *evasion_detail(), check="all"),
      D("mcp", "the MCP servers cdp, driver, sb and dev, and their tools"),
      D("cli", "the library half of sbrs, with no parser: commands.rs, scaffold.rs, scripts/",
        *cli_lib_detail(), check="all"),
      D("testing", "the rstest sb fixtures; bdd/: Gherkin through rstest-bdd"),
      D("plugins", "reporting and logging plugins"),
      D("plugin_host", "resolves plugin names to a PluginSet (feature plugins); the plugins are in the seleniumbase-rs-plugins repository"),
      D("config", "settings, runtime config, proxy and ad-block lists"),
      D("core", "test result records and the Allure, JUnit and dashboard writers"),
      D("common", "graceful shutdown, encryption, obfuscation, test ids, the debug prompt"),
      D("storage", "ResultStore and ProfileVault (feature turso)"),
      D("utilities", "RetryPolicy, the grid server, Selenium Grid and IDE, the Python importer",
        *utilities_detail(), check="all"),
      D("utils", "selectors, shadow DOM, URLs, geometry, bundled extensions, translations"),
      D("profile_payloads", "parses external browser-profile JSON into a Fingerprint",
        *profile_payload_detail(), check="all"),
      D("resources", "files the crate manages on disk, such as downloaded browser drivers"),
      D("js_code", "building blocks for the JavaScript the crate sends to a page"),
      F("artifacts.rs", "where the files of a test run go (latest_logs) and how they are named"),
      F("cookie.rs", "Cookie and SameSite, the one cookie type of both engines"),
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
          F("trace.rs", "sbrs trace show: opens a trace folder's viewer"),
          D("mcp_install", "sbrs-mcp install, uninstall and clients", *cli_install_files(), open=True, check="all"),
          *cli_package_detail(),
          open=True, check="all"),
        D("tests", "every test that runs the real sbrs or sbrs-mcp binary"),
        open=True, check="all"),
      D("own-imports", "tests that import only seleniumbase_rs: the proof that its re-exports work"),
      D("playwright", "the package seleniumbase-rs-playwright: Stealthy Playwright Mode, rustwright attached to a stealthy sb_cdp browser"),

      open=True, check="all"),
    D("tests", "integration tests of the library",
      D("golden", "compiled into the library"),
      D("support", "shared with crates/cli/tests"),
      D("offline", "mock and offline integration tests, one binary"),
      D("chrome", "real-Chrome integration tests, one binary, ignored by default"),
      F("browser_smoke.rs", "the WebDriver path against a real Chrome, ignored by default")),
    D("benches", "the benchmarks",
      F("cdp_latency.rs", "the pure CDP engine against a real Chrome"),
      F("cpu.rs", "the library's own work, no browser")),
    D("examples", "runnable examples"),
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
      F("check-consumers.sh", "mise run contracts: checks what the repositories that build on this crate use")),
    D("tools", "scripts for the docs, the parity manifests, the Linux check and the release archives",
      D("mdx-check", "mise run mdx: builds every docs page as Docs7 does"),
      D("parity", "extracts, seeds and renders the parity manifest (docs/parity.md)"),
      D("layout_tree", "the tree component that docs_layout_tree.py copies onto the pages"),
      D("linux-check", "mise run linux-check: the test suite and the Chrome tests in a Linux container"),
      D("release", "build-binaries.sh: the release archives for Linux and macOS Intel"),
      F("docs_layout_tree.py", "mise run layout-tree: writes the layout trees of the docs pages"),
      F("docs7_changelog.py", "writes docs/changelog.md from CHANGELOG.md"),
      check="dirs"),
    D("docker", "the Linux verification image (Dockerfile.verify, verify-linux.sh) and firefox-check/"),
    D(".config", "configuration of tools",
      F("nextest.toml", "the nextest configuration and its chrome test group")),
    D(".github", "GitHub Actions",
      D("workflows", "ci, docker, completions, release and upstream-watch")),
    F(".gitlab-ci.yml", "GitLab CI: gate, test, supply-chain and docs jobs, the GitLab security scanners, and the manual msrv and chrome jobs"),
    D(".warp", "the Warp terminal",
      D("workflows", "the Warp workflows for sbrs")),
    F("Dockerfile", "the sbrs image; its skeleton step lists every target"),
    F("Makefile", "make targets"),
    F("mise.toml", "the tools and the tasks of the repository (mise tasks ls)"),
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


def attributes(node, visible):
    parts = [f'name="{node.name}"']
    if node.dir and node.open and visible:
        parts.append("open")
    if node.highlight:
        parts.append("highlight")
    if node.note:
        parts.append(f'note="{node.note}"')
    return " ".join(parts)


def visible_children(node, overview):
    """Children drawn for this tree. Detail rows stay out of the repository-wide tree."""
    if overview:
        return [child for child in node.children if not child.detail]
    return list(node.children)


def check_notes(node, problems):
    for text in (node.name, node.note):
        if any(c in text for c in '"&\n'):
            problems.append(f"{node.name}: a name or note may not hold a quote, an ampersand or a newline")
    for child in node.children:
        check_notes(child, problems)


def render_mdx(root, overview=False):
    """The tree as a `<LayoutTree>`, the component of the Docs7 pages."""
    lines = ["<LayoutTree>"]

    def walk(node):
        children = visible_children(node, overview)
        if node.dir and children:
            lines.append(f"<LayoutDir {attributes(node, True)}>")
            for child in children:
                walk(child)
            lines.append("</LayoutDir>")
        elif node.dir:
            lines.append(f"<LayoutDir {attributes(node, False)} />")
        else:
            lines.append(f"<LayoutFile {attributes(node, False)} />")

    walk(root)
    lines.append("</LayoutTree>")
    return "\n".join(lines)


def render_md(root, overview=False):
    """The tree as text in a code block, for a Markdown file that is not on Docs7."""
    rows = []

    def walk(node, prefix, last, top):
        label = node.name + ("/" if node.dir else "")
        rows.append(((label if top else prefix + ("└── " if last else "├── ") + label), node.note))
        children = visible_children(node, overview)
        if node.dir and node.open and children:
            below = "" if top else prefix + ("    " if last else "│   ")
            for i, child in enumerate(children):
                walk(child, below, i == len(children) - 1, False)

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
    return problems, {"mdx": render_mdx(LAYOUT, overview=True), "md": render_md(LAYOUT, overview=True), "component": component_text()}


def find_tree_blocks(text, kind):
    """Every layout-tree block: (start, end, scope). scope is empty for the whole repository."""
    start_re = MDX_TREE_START if kind == "mdx" else MD_TREE_START
    end_marker = (MDX_TREE_MARKERS if kind == "mdx" else MD_TREE_MARKERS)[1]
    blocks = []
    for match in start_re.finditer(text):
        close = text.find(end_marker, match.end())
        if close < 0:
            raise ValueError(f"{match.group(0)} has no {end_marker}")
        blocks.append((match.start(), close + len(end_marker), match.group(1) or ""))
    return blocks


def lookup(scope):
    """The folder `scope` (a path such as `src/api`) inside LAYOUT."""
    node = LAYOUT
    for part in scope.strip("/").split("/"):
        found = next((child for child in node.children if child.dir and child.name == part), None)
        if found is None:
            raise ValueError(f"no folder {scope} in LAYOUT")
        node = found
    return node


def tree_markers(kind, scope):
    if kind == "mdx":
        start = "{/* layout-tree:start */}" if not scope else f"{{/* layout-tree:start {scope} */}}"
        return (start, MDX_TREE_MARKERS[1])
    start = "<!-- layout-tree:start -->" if not scope else f"<!-- layout-tree:start {scope} -->"
    return (start, MD_TREE_MARKERS[1])


def tree_body(scope, kind):
    """The rendered tree. An empty scope is the repository; a path is that folder."""
    node = LAYOUT if not scope else lookup(scope)
    overview = not scope
    if kind == "mdx":
        return render_mdx(node, overview=overview)
    return render_md(node, overview=overview)


def sync(path, text, wanted, adopt):
    """The page text with its blocks written again, and what was off about it."""
    notes = []
    rel = path.relative_to(ROOT).as_posix()
    block = find_block(text, COMPONENT_MARKERS)
    if block is not None:
        start, end = block
        fresh = wrap(COMPONENT_MARKERS, wanted["component"])
        if text[start:end] != fresh:
            notes.append(f"{rel}:{line_of(text, start)}: the tree component differs from the canonical one (line {first_difference(text[start:end], fresh)} of the block)")
            text = text[:start] + fresh + text[end:]
    for kind in ("mdx", "md"):
        for start, end, scope in reversed(find_tree_blocks(text, kind)):
            body = tree_body(scope, kind)
            fresh = wrap(tree_markers(kind, scope), body)
            if text[start:end] != fresh:
                what = f"layout tree of {scope}" if scope else "layout tree"
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


def self_test():
    """The scope marker and the detail rows. Runs on every invocation."""
    sample = (
        "{/* layout-tree:start */}\n\nOLD\n\n{/* layout-tree:end */}\n"
        "{/* layout-tree:start src/browser */}\n\nOLD\n\n{/* layout-tree:end */}\n"
    )
    blocks = find_tree_blocks(sample, "mdx")
    assert [scope for _, _, scope in blocks] == ["", "src/browser"], blocks
    browser = lookup("src/browser")
    rendered = render_mdx(browser, overview=False)
    assert 'name="reuse.rs"' in rendered, "src/browser should list reuse.rs"
    assert 'name="playwright.rs"' not in rendered, "src/browser has no playwright.rs"
    overview = render_mdx(LAYOUT, overview=True)
    assert 'name="reuse.rs"' not in overview, "detail rows stay out of the repository tree"
    assert 'name="base_case.rs"' not in overview
    fresh, notes = sync(ROOT / "docs" / "DEVELOPER_GUIDE.md", sample, {"component": ""}, adopt=False)
    assert "src/browser" in fresh and 'name="launcher.rs"' in fresh, notes
    assert notes, "a stale scoped block must be reported"


def main():
    parser = argparse.ArgumentParser(description="Write the layout trees of the docs pages from one source.")
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--check", action="store_true", help="change nothing; list what differs and exit 1 if anything does")
    mode.add_argument("--adopt", nargs="+", metavar="PAGE", help="wrap the unmarked tree component of these pages")
    mode.add_argument("--print", dest="show", choices=["mdx", "md", "component"], help="print one canonical block")
    parser.add_argument("--scope", default="", help="with --print mdx or md, render this folder instead of the whole repository")
    args = parser.parse_args()

    try:
        self_test()
    except AssertionError as error:
        print(f"tools/docs_layout_tree.py: self-test failed: {error}", file=sys.stderr)
        return 1

    problems, wanted = canonical()
    if problems:
        for p in problems:
            print(f"tools/docs_layout_tree.py: {p}", file=sys.stderr)
        return 1
    if args.scope and args.show not in ("mdx", "md"):
        print("tools/docs_layout_tree.py: --scope is only for --print mdx or --print md", file=sys.stderr)
        return 1
    if args.show:
        if args.scope:
            try:
                sys.stdout.write(tree_body(args.scope, args.show) + "\n")
            except ValueError as error:
                print(f"tools/docs_layout_tree.py: {error}", file=sys.stderr)
                return 1
        else:
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
