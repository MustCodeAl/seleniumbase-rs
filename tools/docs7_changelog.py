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
REPO = "https://github.com/MustCodeAl/seleniumbase-rs"

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
    """A link target that works from the changelog page, or None to drop the link.

    Every entry sits inside an <Update> component, where Docs7 does not rewrite
    `x.md` links (they would open the raw file), so a docs page becomes its site
    route: docs/tutorials/x.md#a gives /tutorials/x#a.
    """
    if re.match(r"https?://", target) or target.startswith("#"):
        return target
    if target.startswith("docs/"):
        page, _, anchor = target[len("docs/"):].partition("#")
        if page.endswith(".md"):
            page = page[: -len(".md")]
        return f"/{page}" + (f"#{anchor}" if anchor else "")
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


def tag_of(version):
    """The git tag of a released version: 0.5.1 and v0.5.1 both give v0.5.1."""
    return version if version.startswith("v") else f"v{version}"


def downloads(tag):
    """One line of links to a release: its page and the two source archives."""
    return (
        f"**Downloads:** [Release notes]({REPO}/releases/tag/{tag}) · "
        f"[Source (.zip)]({REPO}/archive/refs/tags/{tag}.zip) · "
        f"[Source (.tar.gz)]({REPO}/archive/refs/tags/{tag}.tar.gz)"
    )


def latest(tag):
    """Cards for the newest release and the command that installs it."""
    return [
        f"## Latest release: {tag}",
        "",
        "<Columns cols={3}>",
        "",
        f'<Card title="Release notes" icon="github" color="#1a85d4" href="{REPO}/releases/tag/{tag}">',
        f"  {tag} on GitHub.",
        "</Card>",
        "",
        f'<Card title="Source (.zip)" icon="file-zipper" color="#199f43" href="{REPO}/archive/refs/tags/{tag}.zip">',
        "  The source of this release as a zip archive.",
        "</Card>",
        "",
        f'<Card title="Source (.tar.gz)" icon="box-archive" color="#d47628" href="{REPO}/archive/refs/tags/{tag}.tar.gz">',
        "  The source of this release as a tarball.",
        "</Card>",
        "",
        "</Columns>",
        "",
        "The releases publish no prebuilt binaries; install the command line from the tag:",
        "",
        "```bash Terminal",
        f"cargo install --git {REPO} --tag {tag} seleniumbase-rs-cli",
        "```",
        "",
        "## All releases",
        "",
    ]


def render(releases):
    lines = [
        "---",
        'title: "Changelog"',
        "---",
        "",
        "Generated from `CHANGELOG.md` by `tools/docs7_changelog.py`. The versions follow",
        "Semantic Versioning.",
        "",
    ]
    released = [version for version, date, _ in releases if date]
    if released:
        lines += latest(tag_of(released[0]))
    for version, date, body in releases:
        label = date or "Unreleased"
        description = version if date else "Next release"
        if date and not version.startswith("v"):
            description = f"v{version}"
        tag_list = ", ".join(f'"{t}"' for t in tags(body))
        lines += [f'<Update label="{label}" description="{description}" tags={{[{tag_list}]}}>', ""]
        if date:
            lines += [downloads(tag_of(version)), ""]
        lines += clean(body)
        lines += ["", "</Update>", ""]
    return "\n".join(lines)


def main():
    releases = sections(SOURCE.read_text(encoding="utf-8"))
    OUT.write_text(render(releases), encoding="utf-8", newline="\n")
    print(f"wrote {OUT} ({len(releases)} releases)")


if __name__ == "__main__":
    main()
