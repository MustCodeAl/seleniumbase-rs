#!/usr/bin/env python3
"""Add a proposed classification for every new upstream name to parity/api.toml.

Usage:
    python3 -I tools/parity/seed_manifest.py            # from the repository root

For each name in parity/upstream-api.json that parity/api.toml does not yet
list, this looks for a Rust counterpart and writes an entry:

  implemented     the Rust item has the same name
  composed        a known Rust composition replaces the Python method
  not-applicable  a known reason (recorded in the tables below)
  planned         nothing found; someone has to decide

Existing entries are never changed, so hand edits survive. Review the new
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
CDP_TYPES = ["Locator", "Page", "Cookies", "Storage", "Window", "Emulation", "Browser"]

CLI_COMPOSED = {
    "help": ("clap", "Provided by the argument parser."),
    "version": ("clap", "Provided by the argument parser."),
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
        if n in CLI_COMPOSED:
            rust, note = CLI_COMPOSED[n]
            put("cli", n, status="composed", rust=rust, note=note)
        elif re.search(rf'\b{camel}\b|"{re.escape(n)}"', cli_text):
            put("cli", n, status="implemented", rust=f"sbase {n}")
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

    merged, added = {}, 0
    for surface in sorted(set(existing) | set(proposed)):
        merged[surface] = dict(existing.get(surface, {}))
        for name, entry in proposed.get(surface, {}).items():
            if name not in merged[surface]:
                merged[surface][name] = entry
                added += 1
    write(merged, manifest, HEADER)
    planned = sum(1 for s in merged.values() for e in s.values() if e["status"] == "planned")
    print(f"added {added} entries; {planned} are still planned", file=sys.stderr)


if __name__ == "__main__":
    main()
