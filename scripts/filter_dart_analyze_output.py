#!/usr/bin/env python3
from __future__ import annotations

import json
import re
import sys
from typing import Any


INFO_LINE_RE = re.compile(r"^\s*info\s+[-•:]", re.IGNORECASE)


def diagnostic_is_info(value: Any) -> bool:
    if not isinstance(value, dict):
        return False
    for key in ("severity", "type", "level"):
        raw = value.get(key)
        if isinstance(raw, str) and raw.casefold() == "info":
            return True
    return False


def filter_json_payload(value: Any) -> Any:
    if isinstance(value, list):
        return [item for item in value if not diagnostic_is_info(item)]
    if isinstance(value, dict):
        for key in ("diagnostics", "issues", "errors"):
            items = value.get(key)
            if isinstance(items, list):
                value = dict(value)
                value[key] = [item for item in items if not diagnostic_is_info(item)]
        return value
    return value


def filter_text_lines(raw: str) -> str:
    kept: list[str] = []
    for line in raw.splitlines():
        if INFO_LINE_RE.match(line):
            continue
        kept.append(line)
    return "\n".join(kept).rstrip()


def main() -> int:
    raw = sys.stdin.read()
    if not raw:
        return 0

    stripped = raw.strip()
    try:
        parsed = json.loads(stripped)
    except json.JSONDecodeError:
        filtered = filter_text_lines(raw)
        if filtered:
            print(filtered)
        return 0

    print(json.dumps(filter_json_payload(parsed), separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
