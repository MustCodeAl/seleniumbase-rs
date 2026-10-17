"""Write docs/changelog.md, a Docs7 changelog page, from CHANGELOG.md.

Each release section of CHANGELOG.md becomes one <Update> block whose tags come
from the headings inside it: Added and Changed give "New", Fixed gives "Fix",
and a Removed heading, or the word BREAKING anywhere in the section, gives
"Breaking". Run it after editing CHANGELOG.md:

    python3 -I tools/docs7_changelog.py

Only the standard library is used.
"""

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "CHANGELOG.md"
OUT = ROOT / "docs" / "changelog.md"

RELEASE = re.compile(r"^## \[?(?P<version>[^\]\s]+)\]?(?: - (?P<date>\d{4}-\d{2}-\d{2}))?\s*$")
LINK_DEFINITION = re.compile(r"^\[[^\]]+\]: \S+\s*$")
FENCE = re.compile(r"^\s*(```|~~~)")
INLINE = re.compile(r"(`[^`\n]*`)")
AUTOLINK = re.compile(r"<(https?://[^>\s]+)>")
LINK = re.compile(r"\]\(([^)\s]+)\)")


def sections(text):
    """(version, date, body lines) for each release, in file order."""
    found = []
    current = None
    for line in text.splitlines():
        match = RELEASE.match(line)
        if match:
            current = (match["version"], match["date"], [])
            found.append(current)
        elif current is not None and not LINK_DEFINITION.match(line):
            current[2].append(line)
    return found


def tags(body):
    """The Update tags for the headings (and BREAKING marks) in a section."""
    text = "\n".join(body)
    headings = set(re.findall(r"^### (\w+)", text, re.MULTILINE))
    found = []
    if headings & {"Added", "Changed"}:
        found.append("New")
    if "Fixed" in headings:
        found.append("Fix")
    if "Removed" in headings or "BREAKING" in text:
        found.append("Breaking")
    return found


def fix_link(target):
    """A link target that works from docs/changelog.md, or None to drop the link."""
    if re.match(r"https?://", target) or target.startswith("#"):
        return target
    if target.startswith("docs/"):
        return target[len("docs/"):]
    return None


def prose(segment):
    """MDX-safe prose: no autolinks, no bare braces or angle brackets."""
    segment = AUTOLINK.sub(r"\1", segment)

    def relink(match):
        fixed = fix_link(match[1])
        return f"]({fixed})" if fixed else "]()"

    segment = LINK.sub(relink, segment)
    # A link whose target could not be kept becomes plain text.
    segment = re.sub(r"\[([^\]]+)\]\(\)", r"\1", segment)
    segment = segment.replace("{", "&#123;").replace("}", "&#125;")
    return segment.replace("<", "&lt;")


def clean(body):
    """Escape prose outside code, indent nothing, and trim blank edges."""
    out = []
    fenced = False
    for line in body:
        if FENCE.match(line):
            fenced = not fenced
            out.append(line)
        elif fenced:
            out.append(line)
        else:
            parts = INLINE.split(line)
            out.append("".join(p if p.startswith("`") else prose(p) for p in parts))
    while out and not out[0].strip():
        out.pop(0)
    while out and not out[-1].strip():
        out.pop()
    return out


def render(releases):
    lines = [
        "---",
        'title: "Changelog"',
        "---",
        "",
        "# Changelog",
        "",
        "Generated from `CHANGELOG.md` by `tools/docs7_changelog.py`. The versions follow",
        "Semantic Versioning.",
        "",
    ]
    for version, date, body in releases:
        label = date or "Unreleased"
        description = version if date else "Next release"
        if date and not version.startswith("v"):
            description = f"v{version}"
        tag_list = ", ".join(f'"{t}"' for t in tags(body))
        lines += [f'<Update label="{label}" description="{description}" tags={{[{tag_list}]}}>', ""]
        lines += clean(body)
        lines += ["", "</Update>", ""]
    return "\n".join(lines)


def main():
    releases = sections(SOURCE.read_text(encoding="utf-8"))
    OUT.write_text(render(releases), encoding="utf-8", newline="\n")
    print(f"wrote {OUT} ({len(releases)} releases)")


if __name__ == "__main__":
    main()
