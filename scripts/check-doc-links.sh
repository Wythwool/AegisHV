#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

python3 - <<'PY'
import re
import sys
from pathlib import Path

root = Path.cwd()
link_re = re.compile(r'(?<!!)\[[^\]]+\]\(([^)]+)\)')
errors = []

markdown = [root / "README.md", root / "BACKLOG.md"]
markdown.extend(sorted((root / "docs").rglob("*.md")))

for path in markdown:
    if not path.exists():
        continue
    text = path.read_text(encoding="utf-8")
    for lineno, line in enumerate(text.splitlines(), 1):
        for match in link_re.finditer(line):
            target = match.group(1).strip()
            if not target or target.startswith(("#", "http://", "https://", "mailto:")):
                continue
            target = target.split("#", 1)[0]
            if not target:
                continue
            candidate = (path.parent / target).resolve()
            try:
                candidate.relative_to(root)
            except ValueError:
                errors.append(f"{path.relative_to(root)}:{lineno}: link leaves repo: {target}")
                continue
            if not candidate.exists():
                errors.append(f"{path.relative_to(root)}:{lineno}: missing link target: {target}")

if errors:
    for error in errors:
        print(error, file=sys.stderr)
    sys.exit(1)

print("checked markdown links")
PY
