#!/usr/bin/env python3
"""Extract the public API of upstream SeleniumBase into a stable JSON file.

Usage:
    python3 -I tools/parity/extract_api.py \
        --seleniumbase PATH/TO/SeleniumBase \
        --mcp PATH/TO/seleniumbase-mcp \
        --out parity/upstream-api.json

Standard library only. The output is sorted and byte-stable, so a diff of the
file between two upstream releases is the list of what changed.
"""
import argparse
import ast
import json
import pathlib
import re
import subprocess
import sys


def parse(path):
    return ast.parse(path.read_text(encoding="utf-8"), filename=str(path))


def params(func):
    args = func.args
    names = [a.arg for a in args.posonlyargs + args.args + args.kwonlyargs]
    if args.vararg:
        names.append("*" + args.vararg.arg)
    if args.kwarg:
        names.append("**" + args.kwarg.arg)
    return [n for n in names if n not in ("self", "cls")]


def class_methods(path, class_name):
    """Public methods of one class, sorted by name."""
    for node in parse(path).body:
        if isinstance(node, ast.ClassDef) and node.name == class_name:
            found = {
                item.name: params(item)
                for item in node.body
                if isinstance(item, (ast.FunctionDef, ast.AsyncFunctionDef))
                and not item.name.startswith("_")
            }
            return [{"name": n, "params": found[n]} for n in sorted(found)]
    raise SystemExit(f"class {class_name} not found in {path}")


def function_params(path, function_name):
    for node in parse(path).body:
        if isinstance(node, ast.FunctionDef) and node.name == function_name:
            return sorted(params(node))
    raise SystemExit(f"function {function_name} not found in {path}")


def cli_commands(path):
    """Commands the `seleniumbase` / `sbase` console script dispatches on."""
    text = path.read_text(encoding="utf-8")
    names = set(re.findall(r'command\s*==\s*"([a-z][a-z0-9-]*)"', text))
    for group in re.findall(r"command\s+in\s+[\(\[]([^\)\]]*)[\)\]]", text):
        names.update(re.findall(r'"([a-z][a-z0-9-]*)"', group))
    return sorted(names)


def pytest_options(path):
    """Option strings registered with `parser.addoption(...)` or a group."""
    options = set()
    for node in ast.walk(parse(path)):
        if (
            isinstance(node, ast.Call)
            and isinstance(node.func, ast.Attribute)
            and node.func.attr == "addoption"
        ):
            for arg in node.args:
                if isinstance(arg, ast.Constant) and isinstance(arg.value, str):
                    if arg.value.startswith("--"):
                        options.add(arg.value)
    return sorted(options)


def mcp_tools(path):
    """Functions registered with `@mcp.tool(...)`, with their parameters."""
    tools = {}
    for node in parse(path).body:
        if not isinstance(node, ast.FunctionDef):
            continue
        for decorator in node.decorator_list:
            target = decorator.func if isinstance(decorator, ast.Call) else decorator
            if (
                isinstance(target, ast.Attribute)
                and target.attr == "tool"
                and isinstance(target.value, ast.Name)
                and target.value.id == "mcp"
            ):
                tools[node.name] = params(node)
    return [{"name": n, "params": tools[n]} for n in sorted(tools)]


def git_head(path):
    out = subprocess.run(
        ["git", "-C", str(path), "rev-parse", "HEAD"],
        capture_output=True, text=True, check=False,
    )
    return out.stdout.strip() or None


def version(path):
    text = (path / "seleniumbase" / "__version__.py").read_text(encoding="utf-8")
    match = re.search(r'__version__\s*=\s*"([^"]+)"', text)
    return match.group(1) if match else None


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--seleniumbase", required=True, type=pathlib.Path)
    ap.add_argument("--mcp", required=True, type=pathlib.Path)
    ap.add_argument("--out", required=True, type=pathlib.Path)
    a = ap.parse_args()
    sb, mcp = a.seleniumbase, a.mcp

    api = {
        "seleniumbase": {"version": version(sb), "commit": git_head(sb)},
        "seleniumbase_mcp": {"commit": git_head(mcp)},
        "basecase": class_methods(sb / "seleniumbase/fixtures/base_case.py", "BaseCase"),
        "cdp": class_methods(sb / "seleniumbase/core/sb_cdp.py", "CDPMethods"),
        "driver": class_methods(sb / "seleniumbase/core/sb_driver.py", "DriverMethods"),
        "cli": cli_commands(sb / "seleniumbase/console_scripts/run.py"),
        "options": {
            "pytest": pytest_options(sb / "seleniumbase/plugins/pytest_plugin.py"),
            "sb_kwargs": function_params(sb / "seleniumbase/plugins/sb_manager.py", "SB"),
            "driver_kwargs": function_params(
                sb / "seleniumbase/plugins/driver_manager.py", "Driver"
            ),
        },
        "mcp": {
            "cdp": mcp_tools(mcp / "cdp_server.py"),
            "driver": mcp_tools(mcp / "driver_server.py"),
            "sb": mcp_tools(mcp / "sb_server.py"),
        },
    }
    a.out.parent.mkdir(parents=True, exist_ok=True)
    a.out.write_text(json.dumps(api, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    counts = {
        "basecase": len(api["basecase"]), "cdp": len(api["cdp"]),
        "driver": len(api["driver"]), "cli": len(api["cli"]),
        "pytest options": len(api["options"]["pytest"]),
        "mcp tools": sum(len(v) for v in api["mcp"].values()),
    }
    print(", ".join(f"{k}={v}" for k, v in counts.items()), file=sys.stderr)


if __name__ == "__main__":
    main()
