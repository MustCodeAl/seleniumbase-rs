"""Dev-only: write the expected output of the recorder pipeline from Python.

Run it by hand with the reference SeleniumBase (4.55.2), never in CI:

    python3 -I tests/fixtures/recorder/gen_expected.py

For each `NAME.raw.json` next to this file it feeds the raw actions through
the real `BaseCase.__process_recorded_actions` (the tab, file and browser
helpers are replaced; the rewrite passes and `generate_sbase_code` are
SeleniumBase's own) and writes `NAME.expected.json`:

    processed  the actions after the rewrite passes, as Python holds them
    python     the lines `generate_sbase_code` wrote
    calls      those lines as {name, target, value}, with the argument order
               of the Rust generators (`assert_text(text, selector)` is
               target = selector, value = text)

`tests/recorder_pipeline.rs` compares the Rust pipeline with `processed` and
`calls`. The expected files are committed; a change to the Rust passes never
touches them, and a different SeleniumBase release may change them.

A `NAME.raw.json` holds:

    {"start_ms": 0, "origins": [], "uc": false, "actions": [[kind, a, b, time], ...]}
"""
import ast
import contextlib
import copy
import io
import json
import os
import sys
import tempfile

import seleniumbase
from seleniumbase import config as sb_config
from seleniumbase.core import recorder_helper
from seleniumbase.fixtures.base_case import BaseCase

HERE = os.path.dirname(os.path.abspath(__file__))


class Tab:
    """Stands in for the driver: one window, which the helpers never read."""

    window_handles = ["tab"]


class Harness(BaseCase):
    """A BaseCase whose browser-facing helpers answer from the fixture."""

    def switch_to_window(self, *args, **kwargs):
        pass

    def _BaseCase__get_recorded_actions_on_active_tab(self):
        return copy.deepcopy(self.fixture_tab)

    def _BaseCase__get_filename(self):
        return "generated_test.py"


def split_call(line, uc):
    """`self.type("#q", "x")` as (name, target, value), the Rust way."""
    node = ast.parse(line.strip(), mode="eval").body
    method = node.func.attr
    args = [ast.literal_eval(arg) for arg in node.args]
    name = {"type": "type_text"}.get(method, method)
    if uc and method == "type":
        name = "press_keys"
    target = args[0] if args else None
    value = args[1] if len(args) > 1 else None
    if method in (
        "assert_text",
        "assert_exact_text",
        "assert_text_not_visible",
        "assert_exact_text_not_visible",
        "deferred_assert_text",
        "deferred_assert_exact_text",
    ):
        # Python's selector is optional and defaults to "html".
        target, value = (args[1] if len(args) > 1 else "html"), args[0]
    elif method in ("set_attribute", "set_attributes"):
        target, value = args[0], "%s=%s" % (args[1], args[2])
    elif method == "assert_attribute":
        extra = args[2] if len(args) > 2 else ""
        target, value = args[0], "%s=%s" % (args[1], extra)
    elif method == "click_with_offset":
        target, value = args[0], "%s, %s" % (args[1], args[2])
    elif method == "sleep":
        target, value = None, str(args[0])
    elif method == "save_screenshot" and len(args) == 2:
        name = "save_screenshot_to_folder"
    return {"name": name, "target": target, "value": value}


def run(case):
    captured = {}
    original = recorder_helper.generate_sbase_code

    def spy(srt_actions):
        captured["processed"] = copy.deepcopy(srt_actions)
        lines = original(srt_actions)
        captured["python"] = list(lines)
        return lines

    recorder_helper.generate_sbase_code = spy
    sb_config._recorded_actions = {}
    sb_config._sb_mgr_recorded_actions = {}
    sb_config._behave_recorded_actions = {}
    harness = Harness("runTest")
    harness.driver = Tab()
    harness.fixture_tab = case["actions"]
    harness._BaseCase__actions_to_save = []
    harness._BaseCase__extra_actions = []
    harness._BaseCase__origins_to_save = list(case.get("origins", []))
    harness._BaseCase__js_start_time = case.get("start_ms", 0)
    here = os.getcwd()
    with tempfile.TemporaryDirectory() as scratch:
        os.chdir(scratch)
        try:
            with contextlib.redirect_stdout(io.StringIO()):
                try:
                    harness._BaseCase__process_recorded_actions()
                except Exception:
                    # Only the files written after the code is generated may fail.
                    if "python" not in captured:
                        raise
        finally:
            os.chdir(here)
            recorder_helper.generate_sbase_code = original
    uc = case.get("uc", False)
    lines = captured["python"]
    if uc:
        lines = [line.replace("self.type(", "self.press_keys(") for line in lines]
    return {
        "processed": captured["processed"],
        "python": lines,
        "calls": [split_call(line, False) for line in lines],
    }


def main():
    print("SeleniumBase", seleniumbase.__version__)
    names = sorted(n for n in os.listdir(HERE) if n.endswith(".raw.json"))
    for name in names:
        with open(os.path.join(HERE, name), encoding="utf-8") as handle:
            case = json.load(handle)
        result = run(case)
        out = os.path.join(HERE, name.replace(".raw.json", ".expected.json"))
        with open(out, "w", encoding="utf-8") as handle:
            json.dump(result, handle, indent=1, ensure_ascii=False)
            handle.write("\n")
        print("wrote", os.path.basename(out), len(result["calls"]), "calls")


if __name__ == "__main__":
    sys.exit(main())
