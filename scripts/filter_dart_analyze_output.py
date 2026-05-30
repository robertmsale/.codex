#!/usr/bin/env python3
from __future__ import annotations

import json
import re
import sys
import tempfile
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


def filter_json_payload(value: Any) -> tuple[Any, list[Any]]:
    if isinstance(value, list):
        kept = [item for item in value if not diagnostic_is_info(item)]
        infos = [item for item in value if diagnostic_is_info(item)]
        return kept, infos
    if isinstance(value, dict):
        infos: list[Any] = []
        for key in ("diagnostics", "issues", "errors"):
            items = value.get(key)
            if isinstance(items, list):
                value = dict(value)
                filtered_items, removed_items = filter_json_payload(items)
                value[key] = filtered_items
                infos.extend(removed_items)
        return value, infos
    return value, []


def filter_text_lines(raw: str) -> tuple[str, list[str]]:
    kept: list[str] = []
    infos: list[str] = []
    for line in raw.splitlines():
        if INFO_LINE_RE.match(line):
            infos.append(line)
        else:
            kept.append(line)
    return "\n".join(kept).rstrip(), infos


def write_infos(infos: list[Any], as_json: bool) -> str:
    tmp_file = tempfile.NamedTemporaryFile(
        prefix="codex_dart_analyze_infos_",
        suffix=".txt",
        delete=False,
        mode="w",
        encoding="utf-8",
    )
    with tmp_file:
        for item in infos:
            if as_json:
                tmp_file.write(f"{json.dumps(item, separators=(',', ':'))}\n")
            else:
                tmp_file.write(f"{item}\n")
    return tmp_file.name


def main() -> int:
    raw = sys.stdin.read()
    if not raw:
        return 0

    stripped = raw.strip()
    info_file_path: str = ""
    try:
        parsed = json.loads(stripped)
    except json.JSONDecodeError:
        filtered, infos = filter_text_lines(raw)
        if infos:
            info_file_path = write_infos(infos, as_json=False)
        if filtered:
            print(filtered)
        if info_file_path:
            print(f"infos here: {info_file_path}", file=sys.stderr)
        return 0

    filtered_payload, infos = filter_json_payload(parsed)
    if infos:
        info_file_path = write_infos(infos, as_json=True)

    print(json.dumps(filtered_payload, separators=(",", ":")))
    if info_file_path:
        print(f"infos here: {info_file_path}", file=sys.stderr)

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
