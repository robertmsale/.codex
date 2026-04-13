#!/usr/bin/env python3
from __future__ import annotations

import argparse
import base64
import json
import os
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[4]
CODEX_SERVICES_SRC = ROOT / "backend" / "python" / "codex-services" / "src"
if str(CODEX_SERVICES_SRC) not in sys.path:
    sys.path.insert(0, str(CODEX_SERVICES_SRC))

from codex_services_http.flutter_drive_service import FlutterDriveService
from codex_services_http import flutter_drive_http_routes as routes


SCREENSHOT_ROOT = Path("/tmp/flutter-driver-screenshots")


def normalize_device_id(value: str) -> str:
    if len(value) == 36:
        return value.upper()
    return value


def normalize_screenshot_out_path(device_id: str, requested_path: str) -> str:
    image_name = Path(requested_path).name
    if image_name in {"", ".", ".."}:
        raise SystemExit("Invalid screenshot image name.")
    return str(SCREENSHOT_ROOT / device_id / image_name)


def parse_json_value(raw: str | None):
    if not raw:
        return None
    return json.loads(raw)


def parse_driver_hierarchy(raw: str):
    raw = (raw or "").strip()
    if not raw:
        return None
    start = raw.find("[")
    if start == -1:
        start = raw.find("{")
    if start == -1:
        return None
    return json.loads(raw[start:])


def compact_idb_element(element: dict) -> str:
    label = (element.get("AXLabel") or "").strip()
    value = element.get("AXValue")
    unique_id = (element.get("AXUniqueId") or "").strip()
    role = (element.get("role_description") or element.get("type") or "").strip()
    frame = element.get("frame") or {}
    try:
        left = int(round(float(frame.get("x", 0))))
        top = int(round(float(frame.get("y", 0))))
        width = int(round(float(frame.get("width", 0))))
        height = int(round(float(frame.get("height", 0))))
    except Exception:
        left = top = width = height = 0
    right = left + width
    bottom = top + height
    primary = label or (value.strip() if isinstance(value, str) and value.strip() else "") or unique_id or role or "<node>"
    parts = [primary]
    if unique_id:
        parts.append(f"id=`{unique_id}`")
    if isinstance(value, str) and value.strip() and value.strip() != primary:
        parts.append(f"value={json.dumps(value)}")
    if role and role.casefold() not in primary.casefold():
        parts.append(f"role={json.dumps(role)}")
    if element.get("enabled") is False:
        parts.append("[disabled]")
    parts.append(f"[{left},{top}][{right},{bottom}]")
    return "- " + " ".join(parts)


def compact_idb_hierarchy_lines(raw: str):
    parsed = parse_driver_hierarchy(raw)
    if not isinstance(parsed, list):
        return raw, []
    lines = []
    for element in parsed:
        if not isinstance(element, dict):
            continue
        if (element.get("AXLabel") or "").strip() == "keyboard-frame":
            continue
        lines.append(compact_idb_element(element))
    return parsed, lines


def render(payload: dict, mode: str, out_path: str | None) -> int:
    if mode == "json":
        print(json.dumps(payload, separators=(",", ":")))
        return 0 if payload.get("ok", True) else 1
    if not payload.get("ok", True):
        message = payload.get("message") or "Request failed."
        if payload.get("stderr"):
            message = f"{message}\n{payload['stderr']}"
        elif payload.get("stdout"):
            message = f"{message}\n{payload['stdout']}"
        print(message, file=sys.stderr)
        return 1
    if mode == "apps":
        for app in payload.get("apps") or []:
            print(f"{app.get('name','-')}\t{app.get('appId','-')}")
        return 0
    if mode == "hierarchy":
        raw = payload.get("hierarchy", "")
        parsed, lines = compact_idb_hierarchy_lines(raw)
        if not isinstance(parsed, list):
            print(raw)
            return 0
        print("\n".join(lines))
        return 0
    if mode == "screenshot":
        data = payload.get("data_base64")
        if not data:
            print("Screenshot response did not include image data.", file=sys.stderr)
            return 1
        target = out_path or payload.get("path")
        if not target:
            print("Screenshot response did not include an output path.", file=sys.stderr)
            return 1
        path = Path(target)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(base64.b64decode(data))
        print(str(path))
        return 0
    if mode == "flow":
        for artifact in payload.get("artifacts") or []:
            print(artifact)
        return 0
    if mode == "op":
        tapped = payload.get("tapped_description")
        post_hierarchy = payload.get("post_hierarchy")
        if isinstance(tapped, str) and tapped:
            print(f"Tapped: {tapped}")
        if isinstance(post_hierarchy, str) and post_hierarchy:
            _parsed, lines = compact_idb_hierarchy_lines(post_hierarchy)
            if lines:
                print("\n".join(lines))
                return 0
    print(payload.get("message", ""))
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="flutter-drive")
    parser.add_argument("--json", action="store_true")
    subparsers = parser.add_subparsers(dest="subcommand", required=True)

    apps = subparsers.add_parser("apps")
    apps.add_argument("--device-id", required=True)

    hierarchy = subparsers.add_parser("hierarchy")
    hierarchy.add_argument("--device-id", required=True)
    widget_tree = subparsers.add_parser("widget-tree")
    widget_tree.add_argument("--device-id", required=True)

    screenshot = subparsers.add_parser("screenshot")
    screenshot.add_argument("--device-id", required=True)
    screenshot.add_argument("--out", required=True)

    command = subparsers.add_parser("command")
    command.add_argument("command_name")
    command.add_argument("--device-id", required=True)
    command.add_argument("--input")
    command.add_argument("--label")
    command.add_argument("--out")

    driver = subparsers.add_parser("driver")
    driver.add_argument("command_name")
    driver.add_argument("--device-id", required=True)
    driver.add_argument("--input")
    driver.add_argument("--label")
    driver.add_argument("--out")

    flow = subparsers.add_parser("flow")
    flow.add_argument("--device-id", required=True)
    flow.add_argument("--input", required=True)
    flow.add_argument("--label")
    return parser


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()
    service = FlutterDriveService(
        broker_base_url=os.environ.get("FLUTTER_SIM_BROKER_BASE_URL", "http://127.0.0.1:8767")
    )
    mode = "op"
    out_path = None

    if args.subcommand == "apps":
        payload = routes.maestro_apps(service=service, device_id=normalize_device_id(args.device_id))
        mode = "apps"
    elif args.subcommand in {"hierarchy", "widget-tree"}:
        payload = routes.maestro_hierarchy(service=service, device_id=normalize_device_id(args.device_id))
        mode = "hierarchy"
    elif args.subcommand == "screenshot":
        device_id = normalize_device_id(args.device_id)
        out_path = normalize_screenshot_out_path(device_id, args.out)
        payload = routes.maestro_command(
            service=service,
            device_id=device_id,
            command="takeScreenshot",
            input_payload=None,
            label="screenshot",
            out_path=out_path,
        )
        mode = "screenshot"
    elif args.subcommand in {"command", "driver"}:
        device_id = normalize_device_id(args.device_id)
        command_name = args.command_name
        out_path = args.out
        if command_name in {"takeScreenshot", "screenshot"}:
            if not args.out:
                print("--out is required for screenshot commands", file=sys.stderr)
                return 64
            command_name = "takeScreenshot"
            out_path = normalize_screenshot_out_path(device_id, args.out)
            mode = "screenshot"
        payload = routes.maestro_command(
            service=service,
            device_id=device_id,
            command=command_name,
            input_payload=parse_json_value(args.input),
            label=args.label,
            out_path=out_path,
        )
        if mode != "screenshot":
            mode = "op"
    elif args.subcommand == "flow":
        payload = routes.maestro_flow(
            service=service,
            device_id=normalize_device_id(args.device_id),
            commands=parse_json_value(args.input),
            label=args.label,
        )
        mode = "flow"
    else:
        parser.error("unsupported subcommand")
        return 64

    return render(payload, "json" if args.json else mode, out_path)


if __name__ == "__main__":
    raise SystemExit(main())
