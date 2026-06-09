from __future__ import annotations

import argparse
import asyncio
import base64
import json
import logging
import os
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

from idb.common.hid import _key_down_event
from idb.common.hid import _key_up_event
from idb.common.hid import key_press_to_events
from idb.common.types import DomainSocketAddress
from idb.common.types import HIDDelay
from idb.grpc.client import Client

from idb_accessibility import AccessibilityError
from idb_accessibility import best_element_containing_point
from idb_accessibility import compact_idb_element
from idb_accessibility import element_center
from idb_accessibility import ensure_nonzero_element_frame
from idb_accessibility import find_idb_element
from idb_accessibility import normalized_swipe_duration
from idb_accessibility import orientation_metadata_from_elements
from idb_accessibility import render_raw_hierarchy
from idb_accessibility import resolve_tap_point
from idb_accessibility import root_frame_size
from idb_accessibility import screen_dimensions_from_describe_output
from idb_accessibility import tap_coordinates_for_accessibility_point
from idb_accessibility import transform_accessibility_point
from screenshot_crop import ScreenshotCropError
from screenshot_crop import crop_image_by_selector
from screenshot_crop import normalize_fresh_screenshot_orientation
from screenshot_crop import parse_selector


LEFT_COMMAND_KEYCODE = 227
A_KEYCODE = 4
BACKSPACE_KEYCODE = 42
ESCAPE_KEYCODE = 41
FORWARD_DELETE_KEYCODE = 76
DEFAULT_IOS_APP_ID = os.environ.get("EZRA_IOS_APP_ID", "com.ezraworks.aierpios")
SCREENSHOT_ROOT = Path("/tmp/flutter-driver-screenshots")


class DriverError(RuntimeError):
    pass


def default_app_id_for_platform(platform: str) -> str:
    if platform != "ios":
        raise DriverError(f"Unsupported platform {platform!r}.")
    return DEFAULT_IOS_APP_ID


def idb_executable() -> str:
    configured = os.environ.get("IDB_BIN", "").strip()
    if configured:
        return configured
    discovered = shutil.which("idb")
    if discovered:
        return discovered
    return str(Path.home() / ".local" / "bin" / "idb")


def xcrun_executable() -> str | None:
    configured = os.environ.get("XCRUN_BIN", "").strip()
    if configured:
        return configured
    return shutil.which("xcrun")


def launch_env() -> dict[str, str]:
    env = os.environ.copy()
    current_path = env.get("PATH", "")
    segments = [segment for segment in current_path.split(os.pathsep) if segment]
    rbenv_shims = str(Path.home() / ".rbenv" / "shims")
    if rbenv_shims in segments:
        segments = [segment for segment in segments if segment != rbenv_shims]
    if "/opt/homebrew/bin" in segments:
        segments = [segment for segment in segments if segment != "/opt/homebrew/bin"]
    env["PATH"] = os.pathsep.join([rbenv_shims, "/opt/homebrew/bin", *segments])
    return env


def parse_idb_list_targets_output(text: str) -> list[dict[str, str]]:
    devices: list[dict[str, str]] = []
    for raw_line in text.replace("\r", "").splitlines():
        line = raw_line.strip()
        if not line or "|" not in line:
            continue
        parts = [part.strip() for part in line.split("|")]
        if len(parts) < 7:
            continue
        name, device_id, state, target_type, os_version, architecture, companion = parts[:7]
        if state != "Booted" or target_type != "simulator" or not os_version.startswith("iOS "):
            continue
        devices.append(
            {
                "name": name,
                "device_id": normalize_platform_device_id(device_id, platform="ios"),
                "platform": "ios",
                "details": f"{os_version} ({architecture})",
                "os_version": os_version,
                "architecture": architecture,
                "companion": companion if companion != "No Companion Connected" else "",
            }
        )
    return devices


def parse_simctl_devices_output(text: str) -> list[dict[str, str]]:
    try:
        payload = json.loads(text or "{}")
    except json.JSONDecodeError as error:
        raise DriverError(f"simctl list devices returned invalid JSON: {error}") from error
    devices_by_runtime = payload.get("devices")
    if not isinstance(devices_by_runtime, dict):
        raise DriverError("simctl list devices did not return a devices object.")
    devices: list[dict[str, str]] = []
    for runtime_name, runtime_devices in devices_by_runtime.items():
        if not isinstance(runtime_name, str) or "iOS" not in runtime_name:
            continue
        if not isinstance(runtime_devices, list):
            continue
        for item in runtime_devices:
            if not isinstance(item, dict):
                continue
            if item.get("state") != "Booted" or not item.get("isAvailable", True):
                continue
            device_id = str(item.get("udid") or "").strip()
            name = str(item.get("name") or "").strip()
            if not device_id or not name:
                continue
            runtime_label = runtime_name.split(".")[-1].replace("-", " ")
            devices.append(
                {
                    "name": name,
                    "device_id": normalize_platform_device_id(device_id, platform="ios"),
                    "platform": "ios",
                    "details": runtime_label,
                    "os_version": runtime_label,
                    "architecture": "",
                    "companion": "",
                }
            )
    return devices


def normalize_platform_device_id(device_id: str, *, platform: str | None = None) -> str:
    normalized = device_id.strip()
    if platform == "ios" or re.fullmatch(r"[0-9a-fA-F-]{36}", normalized):
        return normalized.upper()
    return normalized


def normalize_device_id(value: str) -> str:
    if len(value) == 36:
        return value.upper()
    return value


def parse_json_value(raw: str | None) -> Any:
    if not raw:
        return None
    return json.loads(raw)


def normalize_screenshot_out_path(device_id: str, requested_path: str) -> str:
    path = Path(requested_path).expanduser()
    if path.name in {"", ".", ".."}:
        raise SystemExit("Invalid screenshot image name.")
    if path.is_absolute():
        return str(path)
    return str((Path.cwd() / path).resolve(strict=False))


def ensure_idb_available() -> str:
    path = idb_executable()
    if not path or not Path(path).exists():
        raise DriverError("idb CLI is not installed or not available on PATH.")
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
        raise DriverError((result.stderr or result.stdout or "idb command failed").strip())
    return result.stdout or ""


def run_simctl_screenshot(*, device_id: str, destination: Path, cwd: Path) -> None:
    xcrun = xcrun_executable()
    if not xcrun:
        raise DriverError("xcrun is not installed or not available on PATH.")
    result = subprocess.run(
        [xcrun, "simctl", "io", device_id, "screenshot", str(destination)],
        capture_output=True,
        text=True,
        cwd=str(cwd),
        env=launch_env(),
    )
    if result.returncode != 0:
        raise DriverError((result.stderr or result.stdout or "simctl screenshot failed").strip())


def transient_simulator_service_error(text: str) -> bool:
    markers = (
        "CoreSimulatorService connection became invalid",
        "CoreSimulatorService connection refused",
        "simdiskimaged crashed or is not responding",
        "Unable to locate device set",
        "unable to discover Simulator runtimes",
    )
    return any(marker in text for marker in markers)


def list_devices(*, cwd: Path) -> list[dict[str, str]]:
    attempts = int(os.environ.get("DESIGNER_DRIVE_DEVICE_LIST_ATTEMPTS", "8"))
    delay = float(os.environ.get("DESIGNER_DRIVE_DEVICE_LIST_RETRY_DELAY", "1.0"))
    last_error = ""
    for attempt in range(max(1, attempts)):
        result = run_idb(argv=["list-targets"], cwd=cwd)
        if result.returncode == 0:
            return parse_idb_list_targets_output(result.stdout or "")
        xcrun = xcrun_executable()
        if not xcrun:
            raise DriverError((result.stderr or result.stdout or "idb list-targets failed and xcrun is not available").strip())
        simctl = subprocess.run(
            [xcrun, "simctl", "list", "devices", "booted", "--json"],
            capture_output=True,
            text=True,
            cwd=str(cwd),
            env=launch_env(),
        )
        if simctl.returncode == 0:
            return parse_simctl_devices_output(simctl.stdout or "{}")
        idb_error = (result.stderr or result.stdout or "idb list-targets failed").strip()
        simctl_error = (simctl.stderr or simctl.stdout or "simctl list devices failed").strip()
        last_error = f"{idb_error}; fallback simctl failed: {simctl_error}"
        if attempt + 1 < attempts and transient_simulator_service_error(last_error):
            time.sleep(delay)
            continue
        break
    raise DriverError(last_error or "idb list-targets failed")


def require_device(*, device_id: str, cwd: Path) -> dict[str, str]:
    normalized = normalize_platform_device_id(device_id, platform="ios")
    for row in list_devices(cwd=cwd):
        if row.get("device_id") == normalized:
            return row
    raise DriverError(f"Device {normalized} is not available to idb.")


def describe_all(*, device_id: str, cwd: Path) -> list[dict[str, Any]]:
    output = must_run_idb(argv=["ui", "describe-all", "--json", "--udid", device_id], cwd=cwd)
    try:
        payload = json.loads(output or "[]")
    except json.JSONDecodeError as error:
        raise DriverError(f"idb ui describe-all returned invalid JSON: {error}") from error
    if not isinstance(payload, list):
        raise DriverError("idb ui describe-all did not return a JSON array.")
    return [item for item in payload if isinstance(item, dict)]


def _companion_path(device_id: str) -> Path:
    return Path("/tmp/idb") / f"{device_id}_companion.sock"


async def _idb_key_async(*, device_id: str, keycode: int) -> None:
    companion_path = _companion_path(device_id)
    if not companion_path.exists():
        raise DriverError(f"idb companion socket not found for device {device_id}: {companion_path}")
    logger = logging.getLogger("direct-idb-driver.idb-hid")
    async with Client.build(address=DomainSocketAddress(path=str(companion_path)), logger=logger) as client:
        await client.set_hardware_keyboard(True)
        await client.key(keycode)


def idb_key(*, device_id: str, keycode: int) -> None:
    asyncio.run(_idb_key_async(device_id=device_id, keycode=keycode))


async def _idb_text_async(*, device_id: str, text: str) -> None:
    companion_path = _companion_path(device_id)
    if not companion_path.exists():
        raise DriverError(f"idb companion socket not found for device {device_id}: {companion_path}")
    logger = logging.getLogger("direct-idb-driver.idb-hid")
    async with Client.build(address=DomainSocketAddress(path=str(companion_path)), logger=logger) as client:
        await client.text(text)


def idb_text(*, device_id: str, text: str) -> None:
    asyncio.run(_idb_text_async(device_id=device_id, text=text))


async def _idb_cmd_a_delete_async(*, device_id: str) -> None:
    companion_path = _companion_path(device_id)
    if not companion_path.exists():
        raise DriverError(f"idb companion socket not found for device {device_id}: {companion_path}")
    logger = logging.getLogger("direct-idb-driver.idb-hid")
    async with Client.build(address=DomainSocketAddress(path=str(companion_path)), logger=logger) as client:
        await client.set_hardware_keyboard(True)
        await client.send_events(
            [
                _key_down_event(LEFT_COMMAND_KEYCODE),
                _key_down_event(A_KEYCODE),
                _key_up_event(A_KEYCODE),
                _key_up_event(LEFT_COMMAND_KEYCODE),
                HIDDelay(duration=0.1),
                *key_press_to_events(BACKSPACE_KEYCODE),
            ]
        )


def idb_cmd_a_delete(*, device_id: str) -> None:
    asyncio.run(_idb_cmd_a_delete_async(device_id=device_id))


def command_tap(*, device_id: str, cwd: Path, selector: Any, duration: float | None = None) -> dict[str, Any]:
    elements = describe_all(device_id=device_id, cwd=cwd)
    try:
        element = find_idb_element(elements=elements, selector=selector)
        ensure_nonzero_element_frame(element=element, selector=selector)
    except AccessibilityError as error:
        raise DriverError(str(error)) from error
    center = element_center(element)
    if center is None:
        raise DriverError(f"Element {selector!r} does not have a usable frame.")
    describe_output = must_run_idb(argv=["describe", "--udid", device_id], cwd=cwd)
    try:
        portrait_width, portrait_height = screen_dimensions_from_describe_output(describe_output)
        root_width, root_height = root_frame_size(elements=elements)
    except AccessibilityError as error:
        raise DriverError(str(error)) from error

    orientation_metadata = orientation_metadata_from_elements(elements)
    if orientation_metadata is not None:
        tap_point = transform_accessibility_point(
            point=center,
            portrait_width=portrait_width,
            portrait_height=portrait_height,
            transform=str(orientation_metadata["transform"]),
        )
        transform = str(orientation_metadata["transform"])
    else:
        transform = None

        def probe_point(candidate: tuple[int, int]) -> dict[str, Any] | None:
            result = run_idb(
                argv=["ui", "describe-point", "--json", "--udid", device_id, str(candidate[0]), str(candidate[1])],
                cwd=cwd,
            )
            if result.returncode != 0 or not (result.stdout or "").strip():
                return None
            try:
                payload = json.loads(result.stdout)
            except json.JSONDecodeError:
                return None
            return payload if isinstance(payload, dict) else None

        try:
            tap_point, _probes, _probed, transform = resolve_tap_point(
                point=center,
                expected_element=element,
                portrait_width=portrait_width,
                portrait_height=portrait_height,
                root_width=root_width,
                root_height=root_height,
                probe_point=probe_point,
            )
        except AccessibilityError as error:
            raise DriverError(str(error)) from error
    argv = ["ui", "tap"]
    if duration is not None:
        argv.extend(["--duration", str(duration)])
    argv.extend(["--udid", device_id, str(tap_point[0]), str(tap_point[1])])
    must_run_idb(argv=argv, cwd=cwd)
    return {
        "ok": True,
        "message": f"tapOn {selector!r} -> [{tap_point[0]},{tap_point[1]}]",
        "transform": transform,
        "tapped_description": compact_idb_element(element),
        "post_hierarchy": json.dumps(describe_all(device_id=device_id, cwd=cwd), separators=(",", ":")),
    }


def command_tap_point(*, device_id: str, cwd: Path, payload: Any) -> dict[str, Any]:
    if not isinstance(payload, dict):
        raise DriverError("tapPoint requires an object payload.")
    try:
        x = int(payload["x"])
        y = int(payload["y"])
    except Exception as error:
        raise DriverError("tapPoint requires integer x and y.") from error
    elements = describe_all(device_id=device_id, cwd=cwd)
    describe_output = must_run_idb(argv=["describe", "--udid", device_id], cwd=cwd)
    try:
        portrait_width, portrait_height = screen_dimensions_from_describe_output(describe_output)
        root_width, root_height = root_frame_size(elements=elements)
        orientation_metadata = orientation_metadata_from_elements(elements)
        if orientation_metadata is not None:
            tap_point = transform_accessibility_point(
                point=(x, y),
                portrait_width=portrait_width,
                portrait_height=portrait_height,
                transform=str(orientation_metadata["transform"]),
            )
        else:
            tap_point = tap_coordinates_for_accessibility_point(
                portrait_width=portrait_width,
                portrait_height=portrait_height,
                root_width=root_width,
                root_height=root_height,
                point=(x, y),
            )
    except AccessibilityError as error:
        raise DriverError(str(error)) from error
    must_run_idb(argv=["ui", "tap", "--udid", device_id, str(tap_point[0]), str(tap_point[1])], cwd=cwd)
    tapped_element = best_element_containing_point(elements=elements, point=(x, y))
    return {
        "ok": True,
        "message": f"tapPoint [{x},{y}] -> [{tap_point[0]},{tap_point[1]}]",
        "tapped_description": compact_idb_element(tapped_element) if tapped_element is not None else None,
        "post_hierarchy": json.dumps(describe_all(device_id=device_id, cwd=cwd), separators=(",", ":")),
    }


def command_input_text(*, device_id: str, cwd: Path, payload: Any) -> dict[str, Any]:
    if not isinstance(payload, str):
        raise DriverError("inputText requires a string payload.")
    must_run_idb(argv=["ui", "text", "--udid", device_id, payload], cwd=cwd)
    return {"ok": True, "message": f"inputText {payload!r}"}


def command_clear_and_input_text(*, device_id: str, payload: Any) -> dict[str, Any]:
    if not isinstance(payload, str):
        raise DriverError("clearAndInputText requires a string payload.")
    idb_cmd_a_delete(device_id=device_id)
    idb_text(device_id=device_id, text=payload)
    return {"ok": True, "message": f"clearAndInputText {payload!r}"}


def command_erase_text(*, device_id: str, count: Any, keycode: int, name: str) -> dict[str, Any]:
    delete_count = 1 if count is None else int(count)
    if delete_count < 0:
        raise DriverError(f"{name} payload must be a non-negative integer.")
    for _ in range(delete_count):
        idb_key(device_id=device_id, keycode=keycode)
    return {"ok": True, "message": f"{name} {delete_count}"}


def command_take_screenshot(*, device_id: str, cwd: Path, out_path: str, selector: Any | None = None) -> dict[str, Any]:
    destination = Path(out_path)
    destination.parent.mkdir(parents=True, exist_ok=True)
    result = run_idb(argv=["screenshot", "--udid", device_id, str(destination)], cwd=cwd)
    if result.returncode != 0:
        failure_text = (result.stderr or result.stdout or "").strip()
        if "No Image available to encode" in failure_text:
            run_simctl_screenshot(device_id=device_id, destination=destination, cwd=cwd)
        else:
            raise DriverError(failure_text or "idb screenshot failed")
    if not destination.exists():
        raise DriverError("Screenshot command completed without producing an image file.")
    orientation_metadata: dict[str, Any] | None = None
    try:
        orientation_metadata = normalize_fresh_screenshot_orientation(image_path=destination, device_id=device_id)
    except ScreenshotCropError as error:
        print(f"screenshot orientation normalization failed: {error}", file=sys.stderr)
    crop_metadata: dict[str, Any] | None = None
    warning: str | None = None
    if selector is not None:
        try:
            crop_metadata = crop_image_by_selector(
                image_path=destination,
                out_path=destination,
                device_id=device_id,
                selector=selector,
            )
        except ScreenshotCropError as error:
            warning = f"selector not found: {error}"
            print(warning, file=sys.stderr)
    payload = base64.b64encode(destination.read_bytes()).decode("ascii")
    response: dict[str, Any] = {"ok": True, "path": str(destination), "data_base64": payload}
    if orientation_metadata is not None:
        response["orientation"] = orientation_metadata
    if selector is not None:
        response["selector"] = selector
    if crop_metadata is not None:
        response["crop"] = crop_metadata
    if warning is not None:
        response["warning"] = warning
    return response


def command_swipe(*, device_id: str, cwd: Path, payload: Any) -> dict[str, Any]:
    if not isinstance(payload, dict):
        raise DriverError("swipe requires an object payload.")
    try:
        x_start = int(payload["x_start"])
        y_start = int(payload["y_start"])
        x_end = int(payload["x_end"])
        y_end = int(payload["y_end"])
    except Exception as error:
        raise DriverError("swipe requires integer x_start, y_start, x_end, and y_end.") from error
    duration = payload.get("duration")
    elements = describe_all(device_id=device_id, cwd=cwd)
    describe_output = must_run_idb(argv=["describe", "--udid", device_id], cwd=cwd)
    try:
        portrait_width, portrait_height = screen_dimensions_from_describe_output(describe_output)
        root_width, root_height = root_frame_size(elements=elements)
        orientation_metadata = orientation_metadata_from_elements(elements)
        if orientation_metadata is not None:
            transform = str(orientation_metadata["transform"])
            swipe_start = transform_accessibility_point(
                point=(x_start, y_start),
                portrait_width=portrait_width,
                portrait_height=portrait_height,
                transform=transform,
            )
            swipe_end = transform_accessibility_point(
                point=(x_end, y_end),
                portrait_width=portrait_width,
                portrait_height=portrait_height,
                transform=transform,
            )
        else:
            swipe_start = tap_coordinates_for_accessibility_point(
                portrait_width=portrait_width,
                portrait_height=portrait_height,
                root_width=root_width,
                root_height=root_height,
                point=(x_start, y_start),
            )
            swipe_end = tap_coordinates_for_accessibility_point(
                portrait_width=portrait_width,
                portrait_height=portrait_height,
                root_width=root_width,
                root_height=root_height,
                point=(x_end, y_end),
            )
        normalized_duration = normalized_swipe_duration(duration) if duration is not None else None
    except AccessibilityError as error:
        raise DriverError(str(error)) from error
    argv = ["ui", "swipe"]
    if normalized_duration is not None:
        argv.extend(["--duration", str(normalized_duration)])
    argv.extend(["--udid", device_id, str(swipe_start[0]), str(swipe_start[1]), str(swipe_end[0]), str(swipe_end[1])])
    must_run_idb(argv=argv, cwd=cwd)
    return {"ok": True, "message": f"swipe [{x_start},{y_start}] -> [{x_end},{y_end}]"}


def command_hide_keyboard(*, device_id: str) -> dict[str, Any]:
    idb_key(device_id=device_id, keycode=ESCAPE_KEYCODE)
    return {"ok": True, "message": "hideKeyboard"}


def command_clear_field(*, device_id: str, cwd: Path, payload: Any) -> dict[str, Any]:
    if not isinstance(payload, dict) or not payload:
        raise DriverError("clearField requires a selector object.")
    result = command_tap(device_id=device_id, cwd=cwd, selector=dict(payload))
    idb_cmd_a_delete(device_id=device_id)
    return {
        "ok": True,
        "message": f"clearField {payload!r}",
        "tapped_description": result.get("tapped_description"),
        "post_hierarchy": json.dumps(describe_all(device_id=device_id, cwd=cwd), separators=(",", ":")),
    }


def command_launch_app(*, device_id: str, cwd: Path, app_id: str, payload: Any) -> dict[str, Any]:
    resolved_app_id = payload.strip() if isinstance(payload, str) and payload.strip() else app_id
    xcrun = xcrun_executable()
    if not xcrun:
        raise DriverError("xcrun is required for launchApp.")
    result = subprocess.run(
        [xcrun, "simctl", "launch", device_id, resolved_app_id],
        cwd=cwd,
        env=launch_env(),
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=30,
        check=False,
    )
    if result.returncode != 0:
        raise DriverError((result.stderr or result.stdout or "simctl launch failed").strip())
    return {"ok": True, "message": f"launchApp {resolved_app_id}", "stdout": result.stdout.strip()}


def command_terminate_app(*, device_id: str, cwd: Path, app_id: str, payload: Any) -> dict[str, Any]:
    resolved_app_id = payload.strip() if isinstance(payload, str) and payload.strip() else app_id
    xcrun = xcrun_executable()
    if not xcrun:
        raise DriverError("xcrun is required for terminateApp.")
    result = subprocess.run(
        [xcrun, "simctl", "terminate", device_id, resolved_app_id],
        cwd=cwd,
        env=launch_env(),
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=30,
        check=False,
    )
    result_text = (result.stderr or result.stdout or "").lower()
    if result.returncode != 0 and "not running" not in result_text and "found nothing to terminate" not in result_text:
        raise DriverError((result.stderr or result.stdout or "simctl terminate failed").strip())
    return {"ok": True, "message": f"terminateApp {resolved_app_id}", "stdout": result.stdout.strip()}


def command_reset_app(*, device_id: str, cwd: Path, app_id: str, payload: Any) -> dict[str, Any]:
    resolved_app_id = payload.strip() if isinstance(payload, str) and payload.strip() else app_id
    xcrun = xcrun_executable()
    if not xcrun:
        raise DriverError("xcrun is required for resetApp.")
    terminate = subprocess.run(
        [xcrun, "simctl", "terminate", device_id, resolved_app_id],
        cwd=cwd,
        env=launch_env(),
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=30,
        check=False,
    )
    terminate_text = (terminate.stderr or terminate.stdout or "").lower()
    if terminate.returncode != 0 and "not running" not in terminate_text and "found nothing to terminate" not in terminate_text:
        raise DriverError((terminate.stderr or terminate.stdout or "simctl terminate failed").strip())
    uninstall = subprocess.run(
        [xcrun, "simctl", "uninstall", device_id, resolved_app_id],
        cwd=cwd,
        env=launch_env(),
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=60,
        check=False,
    )
    uninstall_text = (uninstall.stderr or uninstall.stdout or "").strip()
    already_absent = "no such app" in uninstall_text.lower() or "not installed" in uninstall_text.lower()
    if uninstall.returncode != 0 and not already_absent:
        raise DriverError(uninstall_text or "simctl uninstall failed")
    return {"ok": True, "message": f"resetApp {resolved_app_id}", "stdout": uninstall.stdout.strip()}


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
            raise DriverError("Screenshot commands require --out.")
        selector = None
        if input_payload is not None:
            if not isinstance(input_payload, (dict, str)):
                raise DriverError("Screenshot selector input must be a selector string or selector object.")
            selector = input_payload
        return command_take_screenshot(device_id=device_id, cwd=cwd, out_path=out_path, selector=selector)
    if command_name == "tapOn":
        return command_tap(device_id=device_id, cwd=cwd, selector=input_payload)
    if command_name == "longPressOn":
        return command_tap(device_id=device_id, cwd=cwd, selector=input_payload, duration=0.8)
    if command_name == "tapPoint":
        return command_tap_point(device_id=device_id, cwd=cwd, payload=input_payload)
    if command_name == "inputText":
        return command_input_text(device_id=device_id, cwd=cwd, payload=input_payload)
    if command_name == "clearField":
        return command_clear_field(device_id=device_id, cwd=cwd, payload=input_payload)
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
    if command_name == "launchApp":
        return command_launch_app(device_id=device_id, cwd=cwd, app_id=app_id, payload=input_payload)
    if command_name == "terminateApp":
        return command_terminate_app(device_id=device_id, cwd=cwd, app_id=app_id, payload=input_payload)
    if command_name == "resetApp":
        return command_reset_app(device_id=device_id, cwd=cwd, app_id=app_id, payload=input_payload)
    if command_name == "apps":
        return {"ok": True, "apps": [{"name": "Runner", "appId": app_id}]}
    raise DriverError(f"Unsupported command `{command_name}`.")


def perform_flow(
    *,
    commands: Any,
    device_id: str,
    cwd: Path,
    app_id: str,
) -> dict[str, Any]:
    if not isinstance(commands, list):
        raise DriverError("flow requires a JSON array.")
    artifacts: list[str] = []
    for index, command in enumerate(commands, start=1):
        if isinstance(command, str):
            command_name = command
            payload = None
        elif isinstance(command, dict):
            command_name = str(command.get("command") or command.get("name") or "").strip()
            payload = command.get("input")
            if not command_name:
                raise DriverError(f"Flow step {index} is missing a command name.")
        else:
            raise DriverError(f"Unsupported flow command format: {command!r}")
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
        print(render_raw_hierarchy(raw))
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
            print(render_raw_hierarchy(post_hierarchy))
            return 0
    print(payload.get("message", ""))
    return 0


def add_common_subcommands(parser: argparse.ArgumentParser, *, include_devices: bool, include_driver_alias: bool) -> None:
    subparsers = parser.add_subparsers(dest="subcommand", required=True)
    if include_devices:
        subparsers.add_parser("devices")

    apps = subparsers.add_parser("apps")
    apps.add_argument("--device-id", required=True)

    hierarchy = subparsers.add_parser("hierarchy")
    hierarchy.add_argument("--device-id", required=True)
    widget_tree = subparsers.add_parser("widget-tree")
    widget_tree.add_argument("--device-id", required=True)

    screenshot = subparsers.add_parser(
        "screenshot",
        description=(
            "Capture a screenshot to --out. The saved image is normalized to the live UI orientation first; "
            "apps that export automation.orientationBeacon provide the preferred orientation source. "
            "If --selector selector JSON matches a live accessibility element, the saved image is cropped to that frame. "
            "If it does not match, stderr reports 'selector not found' and the full screenshot is kept."
        ),
    )
    screenshot.add_argument("--device-id", required=True)
    screenshot.add_argument("--out", required=True)
    screenshot.add_argument(
        "--selector",
        help="Optional accessibility selector JSON string. Example: '{\"id\":\"node-style-minimal-flat\"}'. If it matches, the saved screenshot is cropped to that element frame. If not, stderr notes 'selector not found' and the full screenshot is kept.",
    )

    command = subparsers.add_parser(
        "command",
        description=(
            "Run a direct driver command. "
            "For taps, swipes, and takeScreenshot/screenshot, apps that export automation.orientationBeacon "
            "provide the preferred orientation source. "
            "For takeScreenshot/screenshot, the saved image is normalized to the live UI orientation first. "
            "For takeScreenshot/screenshot, pass an optional selector via --input as selector JSON or a selector JSON object "
            "to crop the saved image in one step."
        ),
    )
    command.add_argument("command_name")
    command.add_argument("--device-id", required=True)
    command.add_argument("--input", help="Optional JSON/string payload. For takeScreenshot, this can be selector JSON used for inline cropping.")
    command.add_argument("--label")
    command.add_argument("--out")

    if include_driver_alias:
        driver = subparsers.add_parser(
            "driver",
            description=(
                "Alias of command. "
                "For taps, swipes, and takeScreenshot/screenshot, apps that export automation.orientationBeacon "
                "provide the preferred orientation source. "
                "For takeScreenshot/screenshot, the saved image is normalized to the live UI orientation first. "
                "For takeScreenshot/screenshot, pass an optional selector via --input as selector JSON or a selector JSON object "
                "to crop the saved image in one step."
            ),
        )
        driver.add_argument("command_name")
        driver.add_argument("--device-id", required=True)
        driver.add_argument("--input", help="Optional JSON/string payload. For takeScreenshot, this can be selector JSON used for inline cropping.")
        driver.add_argument("--label")
        driver.add_argument("--out")

    flow = subparsers.add_parser("flow")
    flow.add_argument("--device-id", required=True)
    flow.add_argument("--input", required=True)
    flow.add_argument("--label")


def run_cli(
    *,
    args: argparse.Namespace,
    launch_path: Path,
    app_id: str,
    allow_devices: bool,
) -> int:
    mode = "op"
    out_path = None
    logging.getLogger("direct-idb-driver.idb-hid").setLevel(logging.WARNING)

    try:
        if args.subcommand == "devices":
            if not allow_devices:
                raise DriverError("devices is not supported by this wrapper.")
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
                selector = None
                if getattr(args, "selector", None):
                    selector = parse_selector(args.selector)
                payload = command_take_screenshot(device_id=device_id, cwd=launch_path, out_path=out_path, selector=selector)
                mode = "screenshot"
            elif args.subcommand in {"command", "driver"}:
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
                raise DriverError(f"Unsupported subcommand {args.subcommand!r}.")
    except (DriverError, ValueError, json.JSONDecodeError) as error:
        payload = {"ok": False, "message": str(error)}

    return render(payload, "json" if getattr(args, "json", False) else mode, out_path)
