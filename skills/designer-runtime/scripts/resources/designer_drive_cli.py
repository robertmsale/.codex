#!/usr/bin/env python3
from __future__ import annotations

import argparse
import base64
import json
import logging
import os
import subprocess
import sys
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[5]
CODEX_SERVICES_SRC = ROOT / "backend" / "python" / "codex-services" / "src"
if str(CODEX_SERVICES_SRC) not in sys.path:
    sys.path.insert(0, str(CODEX_SERVICES_SRC))

from codex_services_http.bridge import BridgeError
from codex_services_http.flutter_drive_http_routes import BACKSPACE_KEYCODE
from codex_services_http.flutter_drive_http_routes import _idb_cmd_a_delete
from codex_services_http.flutter_drive_http_routes import _idb_key
from codex_services_http.flutter_drive_http_routes import _idb_text
from codex_services_http.flutter_sim import ESCAPE_KEYCODE
from codex_services_http.flutter_sim import default_app_id_for_platform
from codex_services_http.flutter_sim import idb_executable
from codex_services_http.flutter_sim import launch_env
from codex_services_http.flutter_sim import normalize_device_id as normalize_platform_device_id
from codex_services_http.flutter_sim import parse_idb_list_targets_output


FORWARD_DELETE_KEYCODE = 76
SCREENSHOT_ROOT = Path("/tmp/flutter-driver-screenshots")


def normalize_device_id(value: str) -> str:
    if len(value) == 36:
        return value.upper()
    return value


def parse_json_value(raw: str | None) -> Any:
    if not raw:
        return None
    return json.loads(raw)


def normalize_screenshot_out_path(device_id: str, requested_path: str) -> str:
    image_name = Path(requested_path).name
    if image_name in {"", ".", ".."}:
        raise SystemExit("Invalid screenshot image name.")
    return str(SCREENSHOT_ROOT / device_id / image_name)


def ensure_idb_available() -> str:
    path = idb_executable()
    if not path or not Path(path).exists():
        raise BridgeError("idb CLI is not installed or not available on PATH.")
    return path


def run_idb(*, argv: list[str], cwd: Path) -> subprocess.CompletedProcess[str]:
    idb = ensure_idb_available()
    return subprocess.run(
        [idb, *argv],
        capture_output=True,
        text=True,
        cwd=str(cwd),
        env=launch_env(),
    )


def must_run_idb(*, argv: list[str], cwd: Path) -> str:
    result = run_idb(argv=argv, cwd=cwd)
    if result.returncode != 0:
        raise BridgeError((result.stderr or result.stdout or "idb command failed").strip())
    return result.stdout or ""


def list_devices(*, cwd: Path) -> list[dict[str, str]]:
    output = must_run_idb(argv=["list-targets"], cwd=cwd)
    return parse_idb_list_targets_output(output)


def require_device(*, device_id: str, cwd: Path) -> dict[str, str]:
    normalized = normalize_platform_device_id(device_id, platform="ios")
    for row in list_devices(cwd=cwd):
        if row.get("device_id") == normalized:
            return row
    raise BridgeError(f"Device {normalized} is not available to idb.")


def describe_all(*, device_id: str, cwd: Path) -> list[dict[str, Any]]:
    output = must_run_idb(argv=["ui", "describe-all", "--json", "--udid", device_id], cwd=cwd)
    try:
        payload = json.loads(output or "[]")
    except json.JSONDecodeError as error:
        raise BridgeError(f"idb ui describe-all returned invalid JSON: {error}") from error
    if not isinstance(payload, list):
        raise BridgeError("idb ui describe-all did not return a JSON array.")
    return [item for item in payload if isinstance(item, dict)]


def compact_idb_element(element: dict[str, Any]) -> str:
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


def compact_idb_hierarchy_lines(elements: list[dict[str, Any]]) -> list[str]:
    lines: list[str] = []
    for element in elements:
        if (element.get("AXLabel") or "").strip() == "keyboard-frame":
            continue
        lines.append(compact_idb_element(element))
    return lines


def selector_candidates(selector: Any) -> list[tuple[str, str]]:
    if isinstance(selector, str):
        value = selector.strip()
        if not value:
            raise BridgeError("Selector must not be empty.")
        return [("AXLabel", value), ("AXUniqueId", value), ("AXValue", value)]
    if not isinstance(selector, dict):
        raise BridgeError("Selector must be a string or object.")
    candidates: list[tuple[str, str]] = []
    mapping = {
        "id": "AXUniqueId",
        "text": "AXLabel",
        "label": "AXLabel",
        "value": "AXValue",
    }
    for key, field in mapping.items():
        value = selector.get(key)
        if isinstance(value, str) and value.strip():
            candidates.append((field, value.strip()))
    if not candidates:
        raise BridgeError("Selector object must include id, text, label, or value.")
    return candidates


def find_element(*, elements: list[dict[str, Any]], selector: Any) -> dict[str, Any]:
    candidates = selector_candidates(selector)
    for field, expected in candidates:
        for element in elements:
            actual = element.get(field)
            if isinstance(actual, str) and actual == expected:
                return element
    for field, expected in candidates:
        folded = expected.casefold()
        for element in elements:
            actual = element.get(field)
            if isinstance(actual, str) and actual.casefold() == folded:
                return element
    raise BridgeError(f"Could not find an accessibility element matching {selector!r}.")


def element_center(element: dict[str, Any]) -> tuple[int, int]:
    frame = element.get("frame")
    if not isinstance(frame, dict):
        raise BridgeError("Element does not have frame data.")
    try:
        left = int(round(float(frame["x"])))
        top = int(round(float(frame["y"])))
        width = int(round(float(frame["width"])))
        height = int(round(float(frame["height"])))
    except Exception as error:
        raise BridgeError("Element frame is invalid.") from error
    return int(left + width / 2), int(top + height / 2)


def command_tap(*, device_id: str, cwd: Path, selector: Any, duration: float | None = None) -> dict[str, Any]:
    elements = describe_all(device_id=device_id, cwd=cwd)
    element = find_element(elements=elements, selector=selector)
    tap_x, tap_y = element_center(element)
    argv = ["ui", "tap"]
    if duration is not None:
        argv.extend(["--duration", str(duration)])
    argv.extend(["--udid", device_id, str(tap_x), str(tap_y)])
    must_run_idb(argv=argv, cwd=cwd)
    return {
        "ok": True,
        "message": f"tapOn {selector!r} -> [{tap_x},{tap_y}]",
        "tapped_description": compact_idb_element(element),
        "post_hierarchy": json.dumps(describe_all(device_id=device_id, cwd=cwd), separators=(",", ":")),
    }


def command_input_text(*, device_id: str, cwd: Path, payload: Any) -> dict[str, Any]:
    if not isinstance(payload, str):
        raise BridgeError("inputText requires a string payload.")
    must_run_idb(argv=["ui", "text", "--udid", device_id, payload], cwd=cwd)
    return {"ok": True, "message": f"inputText {payload!r}"}


def command_clear_and_input_text(*, device_id: str, payload: Any) -> dict[str, Any]:
    if not isinstance(payload, str):
        raise BridgeError("clearAndInputText requires a string payload.")
    _idb_cmd_a_delete(device_id=device_id)
    _idb_text(device_id=device_id, text=payload)
    return {"ok": True, "message": f"clearAndInputText {payload!r}"}


def command_erase_text(*, device_id: str, count: Any, keycode: int, name: str) -> dict[str, Any]:
    delete_count = 1 if count is None else int(count)
    if delete_count < 0:
        raise BridgeError(f"{name} payload must be a non-negative integer.")
    for _ in range(delete_count):
        _idb_key(device_id=device_id, keycode=keycode)
    return {"ok": True, "message": f"{name} {delete_count}"}


def command_take_screenshot(*, device_id: str, cwd: Path, out_path: str) -> dict[str, Any]:
    destination = Path(out_path)
    destination.parent.mkdir(parents=True, exist_ok=True)
    must_run_idb(argv=["screenshot", "--udid", device_id, str(destination)], cwd=cwd)
    payload = base64.b64encode(destination.read_bytes()).decode("ascii")
    return {"ok": True, "path": str(destination), "data_base64": payload}


def command_swipe(*, device_id: str, cwd: Path, payload: Any) -> dict[str, Any]:
    if not isinstance(payload, dict):
        raise BridgeError("swipe requires an object payload.")
    try:
        x_start = int(payload["x_start"])
        y_start = int(payload["y_start"])
        x_end = int(payload["x_end"])
        y_end = int(payload["y_end"])
    except Exception as error:
        raise BridgeError("swipe requires integer x_start, y_start, x_end, and y_end.") from error
    argv = ["ui", "swipe", "--udid", device_id, str(x_start), str(y_start), str(x_end), str(y_end)]
    duration = payload.get("duration")
    if duration is not None:
        argv[2:2] = ["--duration", str(float(duration))]
    must_run_idb(argv=argv, cwd=cwd)
    return {"ok": True, "message": f"swipe [{x_start},{y_start}] -> [{x_end},{y_end}]"}


def command_hide_keyboard(*, device_id: str) -> dict[str, Any]:
    _idb_key(device_id=device_id, keycode=ESCAPE_KEYCODE)
    return {"ok": True, "message": "hideKeyboard"}


def perform_command(
    *,
    command_name: str,
    device_id: str,
    cwd: Path,
    input_payload: Any,
    out_path: str | None,
    app_id: str,
) -> dict[str, Any]:
    if command_name in {"takeScreenshot", "screenshot"}:
        if not out_path:
            raise BridgeError("Screenshot commands require --out.")
        return command_take_screenshot(device_id=device_id, cwd=cwd, out_path=out_path)
    if command_name == "tapOn":
        return command_tap(device_id=device_id, cwd=cwd, selector=input_payload)
    if command_name == "longPressOn":
        return command_tap(device_id=device_id, cwd=cwd, selector=input_payload, duration=0.8)
    if command_name == "inputText":
        return command_input_text(device_id=device_id, cwd=cwd, payload=input_payload)
    if command_name == "clearAndInputText":
        return command_clear_and_input_text(device_id=device_id, payload=input_payload)
    if command_name == "eraseText":
        return command_erase_text(device_id=device_id, count=input_payload, keycode=BACKSPACE_KEYCODE, name=command_name)
    if command_name == "forwardEraseText":
        return command_erase_text(device_id=device_id, count=input_payload, keycode=FORWARD_DELETE_KEYCODE, name=command_name)
    if command_name == "hideKeyboard":
        return command_hide_keyboard(device_id=device_id)
    if command_name == "swipe":
        return command_swipe(device_id=device_id, cwd=cwd, payload=input_payload)
    if command_name == "apps":
        return {"ok": True, "apps": [{"name": "Runner", "appId": app_id}]}
    raise BridgeError(f"Unsupported command `{command_name}`.")


def perform_flow(
    *,
    commands: Any,
    device_id: str,
    cwd: Path,
    app_id: str,
) -> dict[str, Any]:
    if not isinstance(commands, list):
        raise BridgeError("flow requires a JSON array.")
    artifacts: list[str] = []
    for index, command in enumerate(commands, start=1):
        if isinstance(command, str):
            command_name = command
            payload = None
        elif isinstance(command, dict):
            command_name = str(command.get("command") or command.get("name") or "").strip()
            payload = command.get("input")
            if not command_name:
                raise BridgeError(f"Flow step {index} is missing a command name.")
        else:
            raise BridgeError(f"Unsupported flow command format: {command!r}")
        result = perform_command(
            command_name=command_name,
            device_id=device_id,
            cwd=cwd,
            input_payload=payload,
            out_path=None,
            app_id=app_id,
        )
        message = result.get("message")
        if isinstance(message, str) and message:
            artifacts.append(message)
    return {"ok": True, "artifacts": artifacts}


def render(payload: dict[str, Any], mode: str, out_path: str | None) -> int:
    if mode == "json":
        print(json.dumps(payload, separators=(",", ":")))
        return 0 if payload.get("ok", True) else 1
    if not payload.get("ok", True):
        print(payload.get("message") or "Request failed.", file=sys.stderr)
        return 1
    if mode == "devices":
        for device in payload.get("devices") or []:
            print("\t".join(filter(None, [str(device.get("name") or "-"), str(device.get("device_id") or "-"), str(device.get("details") or "")])))
        return 0
    if mode == "apps":
        for app in payload.get("apps") or []:
            print(f"{app.get('name', '-')}\t{app.get('appId', '-')}")
        return 0
    if mode == "hierarchy":
        raw = payload.get("hierarchy", "")
        try:
            parsed = json.loads(raw)
        except json.JSONDecodeError:
            print(raw)
            return 0
        if not isinstance(parsed, list):
            print(raw)
            return 0
        print("\n".join(compact_idb_hierarchy_lines(parsed)))
        return 0
    if mode == "screenshot":
        data = payload.get("data_base64")
        target = out_path or payload.get("path")
        if not isinstance(data, str) or not target:
            print("Screenshot response did not include image data or path.", file=sys.stderr)
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
            parsed = json.loads(post_hierarchy)
            print("\n".join(compact_idb_hierarchy_lines(parsed)))
            return 0
    print(payload.get("message", ""))
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="designer-drive")
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--launch-path", default=os.getcwd())
    parser.add_argument("--app-id")
    parser.add_argument("--runtime-label", default="designer")
    parser.add_argument("--artifacts-root")
    subparsers = parser.add_subparsers(dest="subcommand", required=True)

    subparsers.add_parser("devices")

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

    flow = subparsers.add_parser("flow")
    flow.add_argument("--device-id", required=True)
    flow.add_argument("--input", required=True)
    flow.add_argument("--label")
    return parser


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()
    launch_path = Path(args.launch_path).expanduser().resolve(strict=False)
    app_id = args.app_id or default_app_id_for_platform("ios")
    mode = "op"
    out_path = None
    logging.getLogger("flutter-drive-http.idb-hid").setLevel(logging.WARNING)

    try:
        if args.subcommand == "devices":
            payload = {"ok": True, "devices": list_devices(cwd=launch_path)}
            mode = "devices"
        else:
            device_id = normalize_device_id(args.device_id)
            require_device(device_id=device_id, cwd=launch_path)
            if args.subcommand in {"hierarchy", "widget-tree"}:
                payload = {
                    "ok": True,
                    "hierarchy": json.dumps(describe_all(device_id=device_id, cwd=launch_path), separators=(",", ":")),
                }
                mode = "hierarchy"
            elif args.subcommand == "apps":
                payload = {"ok": True, "apps": [{"name": "Runner", "appId": app_id}]}
                mode = "apps"
            elif args.subcommand == "screenshot":
                out_path = normalize_screenshot_out_path(device_id, args.out)
                payload = command_take_screenshot(device_id=device_id, cwd=launch_path, out_path=out_path)
                mode = "screenshot"
            elif args.subcommand == "command":
                command_name = args.command_name
                if command_name in {"takeScreenshot", "screenshot"}:
                    if not args.out:
                        print("--out is required for screenshot commands", file=sys.stderr)
                        return 64
                    out_path = normalize_screenshot_out_path(device_id, args.out)
                    mode = "screenshot"
                payload = perform_command(
                    command_name=command_name,
                    device_id=device_id,
                    cwd=launch_path,
                    input_payload=parse_json_value(args.input),
                    out_path=out_path or args.out,
                    app_id=app_id,
                )
            elif args.subcommand == "flow":
                payload = perform_flow(
                    commands=parse_json_value(args.input),
                    device_id=device_id,
                    cwd=launch_path,
                    app_id=app_id,
                )
                mode = "flow"
            else:
                parser.error("unsupported subcommand")
                return 64
    except (BridgeError, ValueError, json.JSONDecodeError) as error:
        payload = {"ok": False, "message": str(error)}

    return render(payload, "json" if args.json else mode, out_path)


if __name__ == "__main__":
    raise SystemExit(main())
