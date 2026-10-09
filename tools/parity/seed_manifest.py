#!/usr/bin/env python3
"""Add a proposed classification for every new upstream name to parity/api.toml.

Usage:
    python3 -I tools/parity/seed_manifest.py            # from the repository root
    python3 -I tools/parity/seed_manifest.py --reclassify-planned

For each name in parity/upstream-api.json that parity/api.toml does not yet
list, this looks for a Rust counterpart and writes an entry:

  implemented     the Rust item has the same name
  composed        a known Rust composition replaces the Python method
  not-applicable  a known reason (recorded in the tables below)
  planned         nothing found; someone has to decide

Existing entries are never changed, so hand edits survive; with
`--reclassify-planned`, entries still marked `planned` are upgraded when a
counterpart has since appeared. Review the new
`planned` entries, then run `cargo test --test parity`.
"""
import json
import pathlib
import re
import sys
import tomllib

ROOT = pathlib.Path(__file__).resolve().parents[2]
SRC = ROOT / "src"

# Python names that map onto a differently named Rust item.
BASECASE_COMPOSED = {
    "type": ("BaseCase::type_text", "One canonical name: Rust has type_text only."),
    "get_google_auth_password": ("BaseCase::get_mfa_code", "Same TOTP computation."),
}
BASECASE_NOT_APPLICABLE = {
    **{
        name: "JQuery-Confirm dialogs are a page UI; Rust uses native dialogs (show_prompt, show_confirm)."
        for name in ("get_jqc_button_input", "get_jqc_form_inputs", "get_jqc_text_input")
    },
    **{
        name: "unittest/pytest harness plumbing; the Rust runner (api::runner) owns the lifecycle."
        for name in (
            "setUp", "tearDown", "setUpClass", "tearDownClass",
            "main", "run", "skip", "has_exception",
        )
    },
}

# `sb.cdp.<name>(selector, ...)` → the Rust item that does it.
CDP_COMPOSED = {
    "open": "Page::goto", "get": "Page::goto", "reload": "Page::reload",
    "refresh": "Page::reload", "go_back": "Page::back", "go_forward": "Page::forward",
    "get_title": "Page::title", "get_current_url": "Page::url",
    "get_page_source": "Page::content", "get_user_agent": "Page::user_agent",
    "evaluate": "Page::evaluate", "bring_active_window_to_front": "Page::bring_to_front",
    "solve_captcha": "Page::solve_captcha", "save_screenshot": "Page::screenshot",
    "print_to_pdf": "Page::pdf", "save_as_pdf": "Page::pdf",
    "get_text": "Locator::text", "get_attribute": "Locator::attribute",
    "get_html": "Locator::html", "set_value": "Locator::set_value",
    "type": "Locator::type_text", "send_keys": "Locator::type_text",
    "press_keys": "Locator::press", "hover_element": "Locator::hover",
    "hover": "Locator::hover", "drag_and_drop": "Locator::drag_to",
    "scroll_into_view": "Locator::scroll_into_view", "focus": "Locator::focus",
    "is_element_visible": "Locator::is_visible", "is_element_present": "Locator::exists",
    "is_checked": "Locator::is_checked", "is_selected": "Locator::is_selected",
    "is_element_enabled": "Locator::is_enabled", "highlight": "Locator::flash",
    "select_option_by_text": "Locator::select_option",
    "select_option_by_value": "Locator::select_option",
    "select_option_by_index": "Locator::select_option",
    "get_all_cookies": "Cookies::all", "clear_all_cookies": "Cookies::clear",
    "save_cookies": "Cookies::save", "load_cookies": "Cookies::load",
    "set_window_rect": "Window::set_bounds", "get_window_rect": "Window::bounds",
    "maximize": "Window::maximize", "minimize": "Window::minimize",
    "set_timezone": "Emulation::timezone", "set_locale": "Emulation::locale",
    "set_geolocation": "Emulation::geolocation",
    "open_new_tab": "Browser::new_page", "open_new_window": "Browser::new_window",
    "get_tabs": "Browser::pages", "switch_to_newest_tab": "Browser::newest_page",
    "close_active_tab": "Page::close", "grant_permissions": "Browser::grant_permissions",
    "reset_permissions": "Browser::reset_permissions",
}
CDP_COMPOSED.update({
    # Retrying assertions: `page.locator(sel).expect()` / `page.expect()`.
    "assert_element": "LocatorExpect::to_be_visible",
    "assert_element_visible": "LocatorExpect::to_be_visible",
    "assert_element_present": "LocatorExpect::to_exist",
    "assert_element_absent": "LocatorExpect::to_exist",
    "assert_element_not_visible": "LocatorExpect::to_be_hidden",
    "assert_element_attribute": "LocatorExpect::to_have_attribute",
    "assert_text": "LocatorExpect::to_contain_text",
    "assert_exact_text": "LocatorExpect::to_have_text",
    "assert_text_not_visible": "LocatorExpect::to_contain_text",
    "assert_title": "PageExpect::to_have_title",
    "assert_title_contains": "PageExpect::to_contain_title",
    "assert_url": "PageExpect::to_have_url",
    "assert_url_contains": "PageExpect::to_contain_url",
    "assert_any_of_elements_visible": "Page::wait_for_any",
    "assert_any_of_elements_present": "Page::wait_for_any",
    "wait_for_any_of_elements_visible": "Page::wait_for_any",
    "wait_for_any_of_elements_present": "Page::wait_for_any",
    # Waits: `locator.wait_for(State::...)`.
    "wait_for_element": "Locator::wait_for",
    "wait_for_element_visible": "Locator::wait_for",
    "wait_for_element_present": "Locator::wait_for",
    "wait_for_element_absent": "Locator::wait_for",
    "wait_for_element_not_visible": "Locator::wait_for",
    "wait_for_text": "LocatorExpect::to_contain_text",
    "wait_for_text_not_visible": "LocatorExpect::to_contain_text",
    # Clicking and form controls.
    "click_if_visible": "Locator::click",
    "click_nth_element": "Locator::nth",
    "click_nth_visible_element": "Locator::nth",
    "click_visible_elements": "Locator::visible",
    "click_link": "Locator::click",
    "click_and_hold": "Mouse::down",
    "mouse_click": "Mouse::click",
    "hover_and_click": "Locator::hover",
    "nested_click": "Locator::locator",
    "clear_input": "Locator::clear",
    "select": "Locator::select_option",
    "check_if_unchecked": "Locator::check",
    "uncheck_if_checked": "Locator::uncheck",
    "select_if_unselected": "Locator::set_checked",
    "unselect_if_selected": "Locator::set_checked",
    "fast_type": "Locator::fill",
    "fast_keys": "Keyboard::insert_text",
    # Finding and reading elements.
    "find_element": "Page::locator", "find_elements": "Locator::all",
    "find_all": "Locator::all", "find_visible_elements": "Locator::visible",
    "find_element_by_text": "Page::locator", "find_elements_by_text": "Locator::all",
    "get_parent": "Locator::parent", "get_nested_element": "Locator::locator",
    "get_element_attribute": "Locator::attribute", "get_element_attributes": "Locator::info",
    "get_element_html": "Locator::html", "get_element_rect": "Locator::bounding_box",
    "get_element_position": "Locator::bounding_box", "get_element_size": "Locator::bounding_box",
    "is_attribute_present": "Locator::attribute", "is_text_visible": "Locator::text",
    "is_exact_text_visible": "Locator::text", "remove_element": "Locator::remove",
    "remove_elements": "Locator::remove", "remove_from_dom": "Locator::remove",
    "set_attributes": "Locator::set_attribute", "js_scroll_into_view": "Locator::scroll_into_view",
    "highlight_overlay": "Locator::flash",
    # Scrolling and page state.
    "scroll_by_y": "Page::scroll", "scroll_down": "Page::scroll", "scroll_up": "Page::scroll",
    "scroll_to_bottom": "Page::scroll", "scroll_to_top": "Page::scroll", "scroll_to_y": "Page::scroll",
    "get_page_title": "Page::title", "get_active_element": "Page::evaluate",
    "get_active_element_css": "Page::evaluate", "get_origin": "Page::evaluate",
    "goto_if_not_url": "Page::goto", "execute_script": "Page::evaluate",
    "get_document": "Page::execute", "get_flattened_document": "Page::execute",
    "get_navigation_history": "Page::execute",
    "save_screenshot_to_logs": "Page::screenshot", "save_page_source": "Page::content",
    "save_page_source_to_logs": "Page::content", "save_as_html": "Page::content",
    "save_as_html_to_logs": "Page::content", "save_as_pdf_to_logs": "Page::pdf",
    # Cookies, storage, windows, tabs and the browser itself.
    "clear_cookies": "Cookies::clear", "set_all_cookies": "Cookies::set",
    "get_cookie_string": "Cookies::header",
    "get_local_storage_item": "Storage::get", "set_local_storage_item": "Storage::set",
    "get_session_storage_item": "Storage::get", "set_session_storage_item": "Storage::set",
    "get_window": "Window::bounds", "get_window_position": "Window::bounds",
    "get_window_size": "Window::bounds", "get_screen_rect": "Window::bounds",
    "reset_window_size": "Window::restore",
    "switch_to_tab": "Browser::pages", "switch_to_window": "Browser::pages",
    "switch_to_newest_window": "Browser::newest_page", "get_active_tab": "Browser::default_page",
    "stop": "Browser::close", "quit": "Browser::close",
    "get_port": "Browser::debugging_port", "get_rd_port": "Browser::debugging_port",
    "get_rd_host": "Browser::http_url", "get_rd_url": "Browser::http_url",
    "get_endpoint_url": "Browser::http_url", "get_websocket_url": "Browser::websocket_url",
    "grant_all_permissions": "Browser::grant_permissions",
    "set_download_path": "Browser::set_download_dir",
    "click_with_offset": "Locator::click_at",
    "get_gui_element_rect": "Locator::screen_rect",
    "get_gui_element_center": "Locator::screen_rect",
    "get_all_urls": "Locator::urls",
    "medimize": "Window::restore",
    "click_captcha": "Page::solve_captcha",
    "gui_click_captcha": "Page::solve_captcha",
    "click_active_element": "Page::evaluate",
    "get_locale_code": "Page::evaluate",
    "select_all": "Page::evaluate",
    "internalize_links": "Locator::set_attribute",
    "download_file": "Browser::set_download_dir",
    "get_path_of_downloaded_file": "Browser::set_download_dir",
    "assert_downloaded_file": "Browser::set_download_dir",
    "get_mfa_code": "BaseCase::get_mfa_code",
    "enter_mfa_code": "BaseCase::enter_mfa_code",
    "gui_click_x_y": "Gui::click", "gui_click_with_offset": "Gui::click",
    "gui_click_element": "Gui::click", "gui_click_and_hold": "Gui::mouse_down",
    "gui_hover_element": "Gui::move_mouse", "gui_hover_x_y": "Gui::move_mouse",
    "gui_move_to_element": "Gui::move_mouse", "gui_hover_and_click": "Gui::click",
    "gui_drag_and_drop": "Gui::drag", "gui_drag_drop_points": "Gui::drag",
    "gui_press_key": "Gui::press_key", "gui_press_keys": "Gui::press_keys",
    "gui_write": "Gui::write",
})
CDP_TYPES = ["Locator", "Page", "Cookies", "Storage", "Window", "Emulation", "Browser"]

# `sb.cdp.<name>` with no Rust counterpart because Rust already has the idea.
CDP_NOT_APPLICABLE = {
    **{
        name: "Rust has assert!, assert_eq! and friends."
        for name in (
            "assert_equal", "assert_not_equal", "assert_true", "assert_false",
            "assert_in", "assert_not_in",
        )
    },
    "get_event_loop": "The async runtime is the caller's; there is no event loop to expose.",
    "add_handler": "Subscribe to protocol events with Page::events / Browser::events.",
    "js_dumps": "Values cross as serde_json::Value; there is nothing to serialise by hand.",
    "get_beautiful_soup": "HTML parsing lives in api::html (the scraper crate).",
    "tile_windows": "Desktop window tiling is not part of a CDP browser API.",
    "activate_cdp_mode": "Already in CDP mode: sb_cdp is the CDP engine.",
    "sleep": "Use tokio::time::sleep.",
    "append_data_to_logs": "Write files with std::fs; artifacts::ensure_latest_logs_dir gives the logs folder.",
    "save_data_to_logs": "Write files with std::fs; artifacts::ensure_latest_logs_dir gives the logs folder.",
    "save_file_as": "Write files with std::fs.",
}

CLI_COMPOSED = {
    "help": ("clap", "Provided by the argument parser."),
    "version": ("clap", "Provided by the argument parser."),
    "get": ("sbase install", "Python treats `get` and `install` as the same command."),
    "case-plans": ("sbase caseplans", "One canonical name; the hyphenated spelling is an alias upstream."),
    "gui-behave": ("sbase behave-gui", "One canonical name; the alias is not ported."),
    "gui": ("sbase commander", "Python's `gui` is the same test runner as `commander`."),
    "recorder": ("sbase record", "Records browser actions to a test file."),
    "codegen": ("sbase record", "Records browser actions to a test file."),
}
CLI_NOT_APPLICABLE = {
    "methods": "Lists Python methods; use `cargo doc` or docs/parity.md for the Rust API.",
    "options": "Lists pytest options; Rust settings are fields of BrowserConfig and RuntimeConfig.",
    "behave-options": "Lists behave (Python BDD) options; this crate has its own Gherkin runner.",
    "obfuscate": "Obfuscates a Python source file; there is no Python source to obfuscate here.",
    "unobfuscate": "Reverses Python source obfuscation; see `obfuscate`.",
    "extract-objects": "Edits the structure of Python test files.",
    "inject-objects": "Edits the structure of Python test files.",
    "revert-objects": "Edits the structure of Python test files.",
}
CLI_DIVERGENCE = {
    "encrypt": "AES-256-GCM under a PBKDF2 key from SB_ENCRYPTION_KEY, not Python's fixed-key obfuscation; tokens are not interchangeable.",
    "decrypt": "Reads tokens made by `sbase encrypt` only; see `encrypt`.",
}

FN = re.compile(r"^\s*pub(?:\([a-z]+\))?\s+(?:const\s+)?(?:async\s+)?(?:unsafe\s+)?fn\s+(\w+)")
IMPL = re.compile(r"^\s*impl(?:<[^>]*>)?\s+(?:[\w:<>, ']+\s+for\s+)?([A-Za-z_]\w*)")
STRUCT = re.compile(r"^\s*pub\s+struct\s+(\w+)")
FIELD = re.compile(r"^\s*pub\s+(\w+)\s*:")


def scan_rust():
    """(type, fn) pairs and (struct, field) pairs found in the sources."""
    methods, fields = set(), set()
    for path in sorted(SRC.rglob("*.rs")):
        owner = None
        for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
            if m := IMPL.match(line):
                owner = m.group(1)
            elif m := STRUCT.match(line):
                owner = m.group(1)
            elif owner and (m := FN.match(line)):
                methods.add((owner, m.group(1)))
            elif owner and (m := FIELD.match(line)):
                fields.add((owner, m.group(1)))
    return methods, fields


def rust_text(*parts):
    return "\n".join(
        p.read_text(encoding="utf-8", errors="replace")
        for part in parts
        for p in sorted(SRC.glob(part))
    )


def mcp_tool_names():
    names = {}
    for server in ("cdp", "driver", "sb", "stealth"):
        text = (SRC / "mcp" / f"{server}.rs").read_text(encoding="utf-8")
        names[server] = set(re.findall(r'ToolDef::new\(\s*"(\w+)"', text))
    names["sb"] |= names["stealth"]
    return names


def classify(api, methods, fields):
    entries = {}

    def put(surface, name, **kw):
        entries.setdefault(surface, {})[name] = kw

    for item in api["basecase"]:
        n = item["name"]
        if ("BaseCase", n) in methods:
            put("basecase", n, status="implemented", rust=f"BaseCase::{n}")
        elif n in BASECASE_COMPOSED:
            rust, note = BASECASE_COMPOSED[n]
            put("basecase", n, status="composed", rust=rust, note=note)
        elif n in BASECASE_NOT_APPLICABLE:
            put("basecase", n, status="not-applicable", reason=BASECASE_NOT_APPLICABLE[n])
        else:
            put("basecase", n, status="planned", note="No Rust counterpart yet.")

    for item in api["cdp"]:
        n = item["name"]
        if n in CDP_NOT_APPLICABLE:
            put("cdp", n, status="not-applicable", reason=CDP_NOT_APPLICABLE[n])
            continue
        target = CDP_COMPOSED.get(n)
        if target is None:
            target = next((f"{t}::{n}" for t in CDP_TYPES if (t, n) in methods), None)
        if target and tuple(target.split("::")) in methods:
            put("cdp", n, status="composed", rust=target,
                note="sb.cdp takes a selector per call; Rust holds it in a Locator.")
        else:
            put("cdp", n, status="planned", note="No Rust composition identified yet.")

    for item in api["driver"]:
        n = item["name"]
        if ("BaseCase", n) in methods:
            put("driver", n, status="implemented", rust=f"BaseCase::{n}")
        else:
            put("driver", n, status="planned", note="No Rust counterpart yet.")

    cli_text = rust_text("cli/**/*.rs")
    for n in api["cli"]:
        camel = "".join(p.capitalize() for p in n.split("-"))
        if n in CLI_NOT_APPLICABLE:
            put("cli", n, status="not-applicable", reason=CLI_NOT_APPLICABLE[n])
        elif n in CLI_COMPOSED:
            rust, note = CLI_COMPOSED[n]
            put("cli", n, status="composed", rust=rust, note=note)
        elif re.search(rf'\b{camel}\b|"{re.escape(n)}"', cli_text):
            extra = {"divergence": CLI_DIVERGENCE[n]} if n in CLI_DIVERGENCE else {}
            put("cli", n, status="implemented", rust=f"sbase {n}", **extra)
        else:
            put("cli", n, status="planned", note="No Rust command yet.")

    config_types = ("BrowserConfig", "RuntimeConfig")
    for surface, names in (
        ("options_pytest", [o.lstrip("-").replace("-", "_") for o in api["options"]["pytest"]]),
        ("options_sb", api["options"]["sb_kwargs"]),
        ("options_driver", api["options"]["driver_kwargs"]),
    ):
        raw = api["options"]["pytest"] if surface == "options_pytest" else names
        for original, n in zip(raw, names):
            owner = next((t for t in config_types if (t, n) in fields), None)
            if owner:
                put(surface, original, status="implemented", rust=f"{owner}::{n}")
            else:
                put(surface, original, status="planned", note="No Rust setting yet.")

    ours = mcp_tool_names()
    for server in ("cdp", "driver", "sb"):
        for tool in api["mcp"][server]:
            n = tool["name"]
            if n in ours[server]:
                put(f"mcp_{server}", n, status="implemented", rust=f"mcp::{server}")
            else:
                put(f"mcp_{server}", n, status="planned", note="Tool not offered yet.")
    return entries


def toml_value(v):
    return json.dumps(v, ensure_ascii=False)


def write(entries, path, header):
    lines = [header.rstrip(), ""]
    for surface in sorted(entries):
        for name in sorted(entries[surface]):
            e = entries[surface][name]
            lines.append(f"[{surface}.{toml_value(name)}]")
            for key in ("status", "rust", "reason", "note", "divergence"):
                if key in e:
                    lines.append(f"{key} = {toml_value(e[key])}")
            lines.append("")
    path.write_text("\n".join(lines), encoding="utf-8")


HEADER = """\
# How each name of the upstream Python API maps onto this crate.
#
# status: implemented | composed | not-applicable | planned
#   implemented     `rust` is the Rust item (Type::method, Type::field, mcp::<server>,
#                   or `sbase <command>`)
#   composed        the Python method is replaced by a Rust composition; `rust` is its
#                   main item and `note` says how they differ
#   not-applicable  `reason` says why there is nothing to port
#   planned         not done yet; `note` says what is missing
# `divergence` records any difference in behaviour from the Python method.
#
# New upstream names are added by tools/parity/seed_manifest.py; edit entries freely.
"""


def main():
    upstream = ROOT / "parity" / "upstream-api.json"
    manifest = ROOT / "parity" / "api.toml"
    api = json.loads(upstream.read_text(encoding="utf-8"))
    existing = tomllib.loads(manifest.read_text(encoding="utf-8")) if manifest.exists() else {}
    methods, fields = scan_rust()
    proposed = classify(api, methods, fields)

    upgrade = "--reclassify-planned" in sys.argv[1:]
    merged, added = {}, 0
    for surface in sorted(set(existing) | set(proposed)):
        merged[surface] = dict(existing.get(surface, {}))
        for name, entry in proposed.get(surface, {}).items():
            current = merged[surface].get(name)
            if current is None:
                merged[surface][name] = entry
                added += 1
            elif upgrade and current["status"] == "planned" and entry["status"] != "planned":
                merged[surface][name] = entry
                added += 1
    write(merged, manifest, HEADER)
    planned = sum(1 for s in merged.values() for e in s.values() if e["status"] == "planned")
    print(f"added {added} entries; {planned} are still planned", file=sys.stderr)


if __name__ == "__main__":
    main()
