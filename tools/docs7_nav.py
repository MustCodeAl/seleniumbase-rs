"""Write docs/docs.json, the Docs7 site configuration, from docs/SUMMARY.md.

Docs7 (https://context7.com/docs/docs7/quickstart) publishes the Markdown pages
in docs/ as a site. Its navigation mirrors the mdBook table of contents: an
entry with sub-entries becomes a group whose first page is the entry itself.
Run it after changing SUMMARY.md:

    python3 -I tools/docs7_nav.py
"""

import json
import re
from pathlib import Path

DOCS = Path(__file__).resolve().parent.parent / "docs"
ENTRY = re.compile(r"^(?P<indent>\s*)- \[(?P<title>[^\]]+)\]\((?P<path>[^)]+)\.md\)\s*$")


def entries():
    """(depth, title, page) for each line of SUMMARY.md that links a page."""
    for line in (DOCS / "SUMMARY.md").read_text(encoding="utf-8").splitlines():
        match = ENTRY.match(line)
        if match:
            depth = len(match["indent"].expandtabs(2)) // 2
            yield depth, match["title"], match["path"]


def navigation():
    """The pages, with each entry that has sub-entries made a group."""
    items = list(entries())
    pages = []
    for index, (depth, title, page) in enumerate(items):
        if depth > 0:
            continue
        children = []
        for child_depth, _, child_page in items[index + 1:]:
            if child_depth == 0:
                break
            children.append(child_page)
        pages.append({"group": title, "pages": [page, *children]} if children else page)
    return pages


def main():
    config = {
        "$schema": "https://context7.com/schema/docs7.json",
        "name": "SeleniumBase for Rust",
        "colors": {"primary": "#CE422B"},
        "navigation": {"groups": [{"group": "Documentation", "pages": navigation()}]},
    }
    out = DOCS / "docs.json"
    out.write_text(json.dumps(config, indent=2) + "\n", encoding="utf-8", newline="\n")
    print(f"wrote {out}")


if __name__ == "__main__":
    main()
