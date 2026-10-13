"""mdBook preprocessor that drops the YAML front matter Docs7 needs.

Docs7 reads a page's title from front matter (`---` / `title: ...` / `---`);
mdBook would print it as a rule and a heading. book.toml runs this before the
HTML renderer.
"""

import json
import re
import sys

FRONT_MATTER = re.compile(r"\A---\r?\n.*?\r?\n---\r?\n(?:\r?\n)?", re.DOTALL)


def strip(items):
    for item in items:
        chapter = item.get("Chapter") if isinstance(item, dict) else None
        if chapter:
            chapter["content"] = FRONT_MATTER.sub("", chapter["content"], count=1)
            strip(chapter.get("sub_items", []))


def main():
    if len(sys.argv) > 1 and sys.argv[1] == "supports":
        sys.exit(0)
    _context, book = json.load(sys.stdin)
    strip(book.get("items") or book.get("sections") or [])
    json.dump(book, sys.stdout)


if __name__ == "__main__":
    main()
